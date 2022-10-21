use std::convert::Infallible;
use std::net::{SocketAddr, Ipv4Addr};
use std::time::SystemTime;
use std::sync::Arc;
use log::{trace, debug};
use hyper::server::conn::AddrIncoming;
use hyper::{Body, Request, Response, Method, Server, StatusCode};
use hyper::body::Bytes;
use hyper::service::{make_service_fn, service_fn};
use http::{Uri};
use http::uri::{Scheme, Authority};
use http::header::{HeaderName, HeaderValue};
use anyhow::{anyhow, Result, Error};
use json::{JsonValue, object};
use aws_types::SdkConfig;
use aws_types::credentials::{ProvideCredentials, Credentials};
use aws_sigv4::http_request::{SigningSettings, SignableRequest, SignableBody};
use aws_sigv4::SigningParams;
use lazy_static::lazy_static;
use regex::Regex;

use crate::http_client::HttpProxyClient;
use crate::keypair::KeyPair;
use crate::nsm::Nsm;

const X_AMZ_TARGET: HeaderName = HeaderName::from_static("x-amz-target");

static X_AMZ_JSON: HeaderValue = HeaderValue::from_static("application/x-amz-json-1.1");

const X_AMZ_CREDENTIAL: &str = "X-Amz-Credential";

const ATTESTING_ACTIONS: [&str; 3] = [ "TrentService.Decrypt", "TrentService.GenerateDataKey", "TrentService.GenerateRandom" ];

const KMS_SERVICE_NAME: &str = "kms";

// Used to parse out the required fields out of the Authorization header or query parameters.
// TODO: make it work using string references to avoid numerous copies.
struct CredentialScope {
    region: String,
    service: String,
}

impl CredentialScope {
    fn from_request(head: &http::request::Parts) -> Result<Self> {
        lazy_static! {
            // e.g.: AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, ...
            static ref HEADER_RE: Regex = Regex::new(r"AWS4\-HMAC\-SHA256 Credential=.*?/.*?/(.*?)/(.*?)/aws4_request,").unwrap();
            static ref QUERY_RE: Regex = Regex::new(r".*?/.*?/(.*?)/(.*?)/aws4_request").unwrap();
        }

        use std::ops::Deref;

        // Look for the signature either in the Authorization HTTP header or in a query string
        let (cred, re) = match head.headers.get(http::header::AUTHORIZATION) {
            Some(authz) => (authz.to_str()?.to_string(), HEADER_RE.deref()),
            None => {
                let cred = amz_credential_query(&head.uri)
                    .ok_or(anyhow!("No AWS SigV4 found in the request"))?;
                (cred, QUERY_RE.deref())
            }
        };

        debug!("CredentialScope: {cred}");

        let groups = re.captures(&cred)
            .ok_or(anyhow!("{} header has an invalid format", http::header::AUTHORIZATION))?;

        Ok(Self{
            region: groups.get(1).unwrap().as_str().to_string(),
            service: groups.get(2).unwrap().as_str().to_string(),
        })
    }

    fn validate(&self) -> Result<()> {
        if self.service != KMS_SERVICE_NAME {
            return Err(anyhow!("Received request signed for a non-KMS ({}) service", self.service));
        }

        Ok(())
    }
}

struct KmsRequestIncoming {
    head: http::request::Parts,
    body: hyper::body::Bytes,
}

impl KmsRequestIncoming {
    async fn recv(req: Request<Body>) -> std::result::Result<Self, hyper::Error> {
        let (head, body) = req.into_parts();
        let body = hyper::body::to_bytes(body).await?;

        Ok(Self{ head, body })
    }

    fn method(&self) -> &http::Method {
        &self.head.method
    }

    fn path(&self) -> &str {
        self.head.uri.path()
    }

    fn target(&self) -> Option<&HeaderValue> {
        self.head.headers.get(X_AMZ_TARGET)
    }

    fn content_type(&self) -> &HeaderValue {
        &X_AMZ_JSON
    }

    fn body_as_json(&self) -> Result<JsonValue> {
        Ok(json::parse(std::str::from_utf8(&self.body)?)?)
    }

    fn is_attesting_action(&self) -> bool {
        if self.head.method == Method::POST {
            if self.head.uri.path() == "/" {
                if let Some(target) = self.target() {
                    let action = target.to_str().unwrap();
                    return ATTESTING_ACTIONS
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(action))
                }
            }
        }

        false
    }

    fn credential_scope<'a>(&'a self) -> Result<CredentialScope> {
        CredentialScope::from_request(&self.head)
    }
}

struct KmsRequestOutgoing {
    inner: Request<Bytes>,
}

impl KmsRequestOutgoing {
    fn new(authority: Authority, action: &HeaderValue, body: JsonValue) -> Result<Self> {
        let body_bytes = Bytes::copy_from_slice(json::stringify(body).as_bytes());

        let uri = Uri::builder()
            .scheme(Scheme::HTTPS)
            .authority(authority)
            .path_and_query("/")
            .build()?;

        let inner = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(X_AMZ_TARGET, action)
            .header(hyper::header::CONTENT_TYPE, &X_AMZ_JSON)
            .body(body_bytes)?;

        Ok(Self{ inner })
    }

    fn from_incoming(req_in: KmsRequestIncoming, authority: Authority) -> Result<Self> {
        let action = req_in.target()
            .ok_or(anyhow!("KMS Action is missing"))?;

        let uri = Uri::builder()
            .scheme(Scheme::HTTPS)
            .authority(authority)
            .path_and_query(req_in.path())
            .build()?;

        let inner = Request::builder()
            .method(req_in.method())
            .uri(uri)
            .header(X_AMZ_TARGET, action)
            .header(hyper::header::CONTENT_TYPE, req_in.content_type())
            .body(req_in.body)?;

        Ok(Self{ inner })
    }

    fn sign(mut self, credentials: &Credentials, region: &str) -> Result<Request<Body>> {
        let signing_settings = SigningSettings::default();
        let mut signing_builder = SigningParams::builder()
            .access_key(credentials.access_key_id())
            .secret_key(credentials.secret_access_key())
            .region(region)
            .service_name(KMS_SERVICE_NAME)
            .time(SystemTime::now())
            .settings(signing_settings);

        if let Some(ref token) = credentials.session_token() {
            signing_builder = signing_builder.security_token(token);
        }

        let signing_params = signing_builder.build()?;

        let signable_request = SignableRequest::new(
            &self.inner.method(),
            &self.inner.uri(),
            &self.inner.headers(),
            SignableBody::Bytes(&self.inner.body()));

        // Sign and then apply the signature to the request
        let signed = aws_sigv4::http_request::sign(signable_request, &signing_params)
            .map_err(|e| Error::msg(e))?;

        let (signing_instructions, _signature) = signed.into_parts();
        signing_instructions.apply_to_request(&mut self.inner);

        // Convert Request<Bytes> to Request<Body>
        let (head, bytes_body) = self.inner.into_parts();

        let req = Request::from_parts(head, Body::from(bytes_body));

        trace!("Signed request auth: {}", req.headers().get("authorization").unwrap().to_str().unwrap());
        Ok(req)
    }
}

pub trait KmsEndpointProvider {
    fn endpoint(&self, region: &str) -> String;
}

pub struct KmsProxyConfig {
    client: HttpProxyClient<Body>,
    credentials: Credentials,
    keypair: Arc<KeyPair>,
    attester: Box<dyn AttestationProvider + Send + Sync>,
    endpoints: Arc<dyn KmsEndpointProvider + Send + Sync>,
}

impl KmsProxyConfig {
    pub async fn new(
        config: SdkConfig,
        proxy_uri: Uri,
        keypair: Arc<KeyPair>,
        attester: Box<dyn AttestationProvider + Send + Sync>,
        endpoints: Arc<dyn KmsEndpointProvider + Send + Sync>)-> Result<Self> {

        let credentials = config.credentials_provider()
            .ok_or(anyhow!("credentials provider is missing"))?
            .provide_credentials()
            .await?;

        trace!("Credentials: {}, {}, {}", credentials.access_key_id(), credentials.secret_access_key(), credentials.session_token().unwrap_or(""));

        let client = crate::http_client::new_http_proxy_client::<Body>(proxy_uri);

        Ok(Self{
            client,
            credentials,
            keypair,
            attester,
            endpoints,
        })
    }

    pub async fn from_imds(
        proxy_uri: Uri,
        keypair: Arc<KeyPair>,
        attester: Box<dyn AttestationProvider + Send + Sync>,
        endpoints: Arc<dyn KmsEndpointProvider + Send + Sync>) -> Result<Self> {

        let imds = super::aws_util::imds_client_with_proxy(proxy_uri.clone()).await?;
        let config = super::aws_util::load_config_from_imds(imds).await?;

        KmsProxyConfig::new(config, proxy_uri, keypair, attester, endpoints).await
    }

    pub fn get_authority(&self, region: &str) -> Authority {
        let endpoint = self.endpoints.endpoint(region);
        Authority::from_maybe_shared(endpoint).unwrap()
    }
}

pub struct KmsProxy {
    config: KmsProxyConfig,
    incoming: AddrIncoming,
}

impl KmsProxy {
    pub fn bind(listen_port: u16, config: KmsProxyConfig) -> Result<Self> {
        let listen_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, listen_port));
        let incoming = AddrIncoming::bind(&listen_addr)?;

        Ok(Self{
            config,
            incoming,
        })
    }

    pub async fn serve(self) -> Result<()> {
        let handler = Arc::new(KmsProxyHandler{
            config: self.config,
        });

        // thanks https://www.fpcomplete.com/blog/ownership-puzzle-rust-async-hyper/
        let make_svc = make_service_fn(move |_conn| {
            // service_fn converts our function into a `Service`
           let handler = handler.clone();
           async {
               Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                   let handler = handler.clone();
                   async move { handler.handle(req).await }
               }))
           }
        });

        Server::builder(self.incoming)
            .serve(make_svc).await?;

        Ok(())
    }
}

struct KmsProxyHandler {
    config: KmsProxyConfig,
}

impl KmsProxyHandler {
    async fn handle(&self, req: Request<Body>) -> std::result::Result<Response<Body>, hyper::Error> {
        debug!("Request: {:?}", req);

        let req_in = KmsRequestIncoming::recv(req).await?;

        // TODO: Check the signature!!!

        let resp_res = if req_in.is_attesting_action() {
            self.handle_attesting_action(req_in).await
        } else {
            self.handle_forward(req_in).await
        };

        resp_res.or_else(|err| Ok(internal_srv_err(err.to_string())))
    }

    async fn handle_attesting_action(&self, req_in: KmsRequestIncoming) -> Result<Response<Body>> {
        // Take the original request, insert "Recipient": <RecipientInfo> into the body json,
        // re-sign the request and send it off.
        debug!("Handling attesting action");

        let credential = req_in.credential_scope()?;
        credential.validate()?;

        let region = credential.region;
        let authority = self.config.get_authority(&region);

        let mut body_obj = req_in.body_as_json()?;

        let attestation_doc = self.get_attestation()?;

        body_obj.insert("Recipient", object!{
            "AttestationDocument": json::JsonValue::String(base64::encode(&attestation_doc)),
            "KeyEncryptionAlgorithm": "RSAES_OAEP_SHA_256",
        })?;

        let req_out = KmsRequestOutgoing::new(authority, req_in.target().unwrap(), body_obj)?;

        // Send the request to the actual KMS
        let resp = self.send(req_out, &region).await?;

        // Decode the response
        self.handle_response(resp).await
    }

    fn get_attestation(&self) -> Result<Vec<u8>> {
        self.config.attester.attestation(self.config.keypair.public_key_as_der()?)
    }

    async fn handle_response(&self, resp: Response<Body>) -> Result<Response<Body>> {
        let (mut head, body) = resp.into_parts();
        head.headers.remove(hyper::header::CONTENT_LENGTH);

        let body = hyper::body::to_bytes(body).await?;

        if head.status != StatusCode::OK {
            trace!("Response body: {:?}", std::str::from_utf8(&body));
            return Ok(bytes_response(head, &body));
        }

        let body_val = json::parse(std::str::from_utf8(&body)?)?;

        if let JsonValue::Object(mut body_obj) = body_val {
            let b64ciphertext = body_obj.remove("CiphertextForRecipient")
                .ok_or(anyhow!("Response body is missing 'CiphertextForRecipient'"))?;

            let b64ciphertext = b64ciphertext.as_str()
                .ok_or(anyhow!("CiphertextForRecipient is not a string"))?;

            let ciphertext = base64::decode(b64ciphertext)?;
            let plaintext = self.decrypt_cms(&ciphertext)?;

            body_obj["Plaintext"] = json::JsonValue::String(base64::encode(&plaintext));
            Ok(json_response(head, JsonValue::Object(body_obj)))
        } else {
            Err(anyhow!("The response body is not a JSON object"))
        }
    }

    async fn handle_forward(&self, req_in: KmsRequestIncoming) -> Result<Response<Body>> {
        let credential = req_in.credential_scope()?;
        credential.validate()?;

        let region = credential.region.to_string();
        let authority = self.config.get_authority(&region);

        let req_out = KmsRequestOutgoing::from_incoming(req_in, authority)?;
        self.send(req_out, &region).await
    }

    async fn send(&self, req: KmsRequestOutgoing, region: &str) -> Result<Response<Body>> {
        let signed = req.sign(&self.config.credentials, region)?;

        debug!("Sending Request: {:?}", signed);
        Ok(self.config.client.request(signed).await?)
    }

    fn decrypt_cms(&self, cms: &[u8]) -> Result<Vec<u8>> {
        let content_info = super::pkcs7::ContentInfo::parse_ber(cms)?;
        Ok(content_info.decrypt_content(&self.config.keypair.private)?)
    }
}

pub trait AttestationProvider {
    fn attestation(&self, public_key: Vec<u8>) -> Result<Vec<u8>>;
}

pub struct NsmAttestationProvider {
    nsm: Arc<Nsm>,
}

impl NsmAttestationProvider {
    pub fn new(nsm: Arc<Nsm>) -> Self {
        Self { nsm }
    }
}

impl AttestationProvider for NsmAttestationProvider {
    fn attestation(&self, public_key: Vec<u8>) -> Result<Vec<u8>> {
        self.nsm.attestation(Some(public_key))
    }
}

fn body_from_slice(bytes: &[u8]) -> Body {
    let bytes = Bytes::copy_from_slice(&bytes);
    let stream = futures_util::stream::once(async move { Result::<_, std::io::Error>::Ok(bytes) } );
    Body::wrap_stream(stream)
}

fn bytes_response(head: http::response::Parts, body: &[u8]) -> Response<Body> {
    Response::from_parts(head, body_from_slice(&body))
}

fn json_response(head: http::response::Parts, json_val: JsonValue) -> Response<Body> {
    let body = json::stringify(json_val);
    bytes_response(head, &body.into_bytes())
}

fn internal_srv_err(msg: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(msg))
        .unwrap()
}

fn amz_credential_query(uri: &Uri) -> Option<String> {
    let q = uri.path_and_query()?
        .query()?;

    for (k, v) in form_urlencoded::parse(q.as_bytes()) {
        if X_AMZ_CREDENTIAL.eq_ignore_ascii_case(&k) {
            return Some(v.to_string());
        }
    }

    None
}

#[test]
fn test_credential_scope() {
    use assert2::assert;

    let req1 = Request::builder()
        .uri("http://kms.us-east-1.amazonaws.com")
        .header(http::header::AUTHORIZATION, "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/kms/aws4_request, ")
        .body(())
        .unwrap();

    let (head1, _) = req1.into_parts();

    let cred1 = CredentialScope::from_request(&head1).unwrap();
    assert!(cred1.region == "us-east-1");
    assert!(cred1.service == "kms");

    let req2 = Request::builder()
        .uri("http://kms.us-east-1.amazonaws.com?X-Amz-Credential=AKIDEXAMPLE%2F20150830%2Fus-east-1%2Fkms%2Faws4_request")
        .body(())
        .unwrap();

    let (head2, _) = req2.into_parts();

    let cred2 = CredentialScope::from_request(&head2).unwrap();
    assert!(cred2.region == "us-east-1");
    assert!(cred2.service == "kms");
}
