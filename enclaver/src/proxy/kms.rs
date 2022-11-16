use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings};
use aws_sigv4::SigningParams;
use aws_types::credentials::Credentials;
use http::header::{HeaderName, HeaderValue};
use http::uri::{Authority, Scheme};
use http::Uri;
use hyper::body::Bytes;
use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use json::{object, JsonValue};
use lazy_static::lazy_static;
use log::{debug, trace};
use regex::Regex;
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;

use crate::keypair::KeyPair;
use crate::nsm::Nsm;

const X_AMZ_TARGET: HeaderName = HeaderName::from_static("x-amz-target");

static X_AMZ_JSON: HeaderValue = HeaderValue::from_static("application/x-amz-json-1.1");

const X_AMZ_CREDENTIAL: &str = "X-Amz-Credential";

const ATTESTING_ACTIONS: [&str; 3] = [
    "TrentService.Decrypt",
    "TrentService.GenerateDataKey",
    "TrentService.GenerateRandom",
];

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

        let groups = re.captures(&cred).ok_or(anyhow!(
            "{} header has an invalid format",
            http::header::AUTHORIZATION
        ))?;

        Ok(Self {
            region: groups.get(1).unwrap().as_str().to_string(),
            service: groups.get(2).unwrap().as_str().to_string(),
        })
    }

    fn validate(&self) -> Result<()> {
        if self.service != KMS_SERVICE_NAME {
            return Err(anyhow!(
                "Received request signed for a non-KMS ({}) service",
                self.service
            ));
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

        Ok(Self { head, body })
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
                        .any(|a| a.eq_ignore_ascii_case(action));
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

        Ok(Self { inner })
    }

    fn from_incoming(req_in: KmsRequestIncoming, authority: Authority) -> Result<Self> {
        let action = req_in.target().ok_or(anyhow!("KMS Action is missing"))?;

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

        Ok(Self { inner })
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
            SignableBody::Bytes(&self.inner.body()),
        );

        // Sign and then apply the signature to the request
        let signed = aws_sigv4::http_request::sign(signable_request, &signing_params)
            .map_err(|e| Error::msg(e))?;

        let (signing_instructions, _signature) = signed.into_parts();
        signing_instructions.apply_to_request(&mut self.inner);

        // Convert Request<Bytes> to Request<Body>
        let (head, bytes_body) = self.inner.into_parts();

        let req = Request::from_parts(head, Body::from(bytes_body));

        trace!(
            "Signed request auth: {}",
            req.headers()
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap()
        );
        Ok(req)
    }
}

pub trait KmsEndpointProvider {
    fn endpoint(&self, region: &str) -> String;
}

pub struct KmsProxyConfig {
    pub client: Box<dyn HttpClient + Send + Sync>,
    pub credentials: Credentials,
    pub keypair: Arc<KeyPair>,
    pub attester: Box<dyn AttestationProvider + Send + Sync>,
    pub endpoints: Arc<dyn KmsEndpointProvider + Send + Sync>,
}

impl KmsProxyConfig {
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

        Ok(Self { config, incoming })
    }

    pub async fn serve(self) -> Result<()> {
        let handler = Arc::new(KmsProxyHandler {
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

        Server::builder(self.incoming).serve(make_svc).await?;

        Ok(())
    }
}

struct KmsProxyHandler {
    config: KmsProxyConfig,
}

impl KmsProxyHandler {
    async fn handle(
        &self,
        req: Request<Body>,
    ) -> std::result::Result<Response<Body>, hyper::Error> {
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

        body_obj.insert(
            "Recipient",
            object! {
                "AttestationDocument": json::JsonValue::String(base64::encode(&attestation_doc)),
                "KeyEncryptionAlgorithm": "RSAES_OAEP_SHA_256",
            },
        )?;

        let req_out = KmsRequestOutgoing::new(authority, req_in.target().unwrap(), body_obj)?;

        // Send the request to the actual KMS
        let resp = self.send(req_out, &region).await?;

        // Decode the response
        self.handle_response(resp).await
    }

    fn get_attestation(&self) -> Result<Vec<u8>> {
        self.config
            .attester
            .attestation(self.config.keypair.public_key_as_der()?)
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
            let b64ciphertext = body_obj
                .remove("CiphertextForRecipient")
                .ok_or(anyhow!("Response body is missing 'CiphertextForRecipient'"))?;

            let b64ciphertext = b64ciphertext
                .as_str()
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

// hyper::client::Client implements tower::Service and would make a perfect
// trait but it uses `&mut self` and would require a needless mutex.
#[async_trait]
pub trait HttpClient {
    async fn request(
        &self,
        req: Request<Body>,
    ) -> std::result::Result<Response<Body>, hyper::Error>;
}

#[async_trait]
impl<C> HttpClient for hyper::client::Client<C>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    async fn request(
        &self,
        req: Request<Body>,
    ) -> std::result::Result<Response<Body>, hyper::Error> {
        hyper::client::Client::request(self, req).await
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
    Body::from(Bytes::copy_from_slice(&bytes))
}

fn json_body(json_val: JsonValue) -> Body {
    let body = json::stringify(json_val);
    body_from_slice(&body.into_bytes())
}

fn bytes_response(head: http::response::Parts, body: &[u8]) -> Response<Body> {
    Response::from_parts(head, body_from_slice(&body))
}

fn json_response(head: http::response::Parts, json_val: JsonValue) -> Response<Body> {
    Response::from_parts(head, json_body(json_val))
}

fn internal_srv_err(msg: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(msg))
        .unwrap()
}

fn amz_credential_query(uri: &Uri) -> Option<String> {
    let q = uri.path_and_query()?.query()?;

    for (k, v) in form_urlencoded::parse(q.as_bytes()) {
        if X_AMZ_CREDENTIAL.eq_ignore_ascii_case(&k) {
            return Some(v.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::assert;
    use pkcs8::DecodePrivateKey;
    use rsa::RsaPrivateKey;

    // Attestation document is passed through verbatim so can test with just random bytes
    const ATTESTATION_DOC: &[u8] = &[
        245, 174, 153, 213, 192, 166, 9, 203, 152, 176, 158, 67, 233, 45, 229, 228,
    ];
    const KEY_ID: &str = "e6ed9116-53d7-11ed-8eee-5b6905c751a7";

    lazy_static! {
        static ref KEYS: JsonValue = object! {
            "Keys": [
                {
                    "KeyArn": "arn:aws:kms:us-east-1:072396882261:key/e6ed9116-53d7-11ed-8eee-5b6905c751a7",
                    "KeyId": KEY_ID,
                }
            ]
        };
    }

    struct Mock;

    #[async_trait]
    impl HttpClient for Mock {
        async fn request(
            &self,
            req: Request<Body>,
        ) -> std::result::Result<Response<Body>, hyper::Error> {
            let action = req.headers().get(X_AMZ_TARGET).unwrap().to_str().unwrap();

            let authz = req
                .headers()
                .get(hyper::header::AUTHORIZATION)
                .unwrap()
                .to_str()
                .unwrap();

            // Don't validate the signature, just care that it was put it and that it looks like it
            // came from the AWS signing process
            assert!(authz.starts_with("AWS4-HMAC-SHA256 Credential="));

            match action {
                "TrentService.ListKeys" => self.list_keys(req).await,
                "TrentService.Decrypt" => self.decrypt(req).await,
                _ => panic!("unexpected action"),
            }
        }
    }

    impl Mock {
        async fn list_keys(
            &self,
            _req: Request<Body>,
        ) -> std::result::Result<Response<Body>, hyper::Error> {
            Ok(kms_response(KEYS.clone()))
        }

        async fn decrypt(
            &self,
            req: Request<Body>,
        ) -> std::result::Result<Response<Body>, hyper::Error> {
            let body = body_as_json(req.into_body()).await.unwrap();

            // make sure the attestation document has been attached
            let att_doc = body["Recipient"]["AttestationDocument"].as_str().unwrap();
            assert!(att_doc == base64::encode(ATTESTATION_DOC));

            let resp = kms_response(object! {
                "EncryptionAlgorithm": "SYMMETRIC_DEFAULT",
                "KeyId": KEY_ID,
                "CiphertextForRecipient": crate::proxy::pkcs7::tests::INPUT,
            });

            Ok(resp)
        }
    }

    impl AttestationProvider for Mock {
        fn attestation(&self, _public_key: Vec<u8>) -> Result<Vec<u8>> {
            Ok(ATTESTATION_DOC.to_vec())
        }
    }

    impl KmsEndpointProvider for Mock {
        fn endpoint(&self, _region: &str) -> String {
            "test.local".to_string()
        }
    }

    fn kms_request(action: &str, body: JsonValue) -> Request<Body> {
        let body_bytes = Bytes::copy_from_slice(json::stringify(body).as_bytes());

        Request::builder()
            .method(Method::POST)
            .uri("/")
            .header(X_AMZ_TARGET, action)
            .header(hyper::header::CONTENT_TYPE, &X_AMZ_JSON)
            .header(
                hyper::header::AUTHORIZATION,
                "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/kms/aws4_request, ",
            )
            .body(Body::from(body_bytes))
            .unwrap()
    }

    fn kms_response(body: JsonValue) -> Response<Body> {
        Response::builder()
            .status(hyper::StatusCode::OK)
            .body(json_body(body))
            .unwrap()
    }

    async fn body_as_json(body: Body) -> Result<JsonValue> {
        let bytes = hyper::body::to_bytes(body).await?;
        Ok(json::parse(std::str::from_utf8(&bytes)?)?)
    }

    fn new_test_handler() -> KmsProxyHandler {
        let key_der = base64::decode(crate::proxy::pkcs7::tests::PRIVATE_KEY).unwrap();
        let priv_key = RsaPrivateKey::from_pkcs8_der(&key_der).unwrap();

        let config = KmsProxyConfig {
            client: Box::new(Mock),
            credentials: Credentials::from_keys("TESTKEY", "TESTSECRET", None),
            keypair: Arc::new(KeyPair::from_private(priv_key)),
            attester: Box::new(Mock {}),
            endpoints: Arc::new(Mock {}),
        };

        KmsProxyHandler { config }
    }

    #[test]
    fn test_credential_scope() {
        let req1 = Request::builder()
            .uri("http://kms.us-east-1.amazonaws.com")
            .header(
                http::header::AUTHORIZATION,
                "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/kms/aws4_request, ",
            )
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

    #[tokio::test]
    async fn test_forwarding_action() {
        let handler = new_test_handler();

        let req = kms_request("TrentService.ListKeys", object! {});
        let resp = handler.handle(req).await.unwrap();

        let (head, body) = resp.into_parts();

        assert!(head.status == hyper::StatusCode::OK);

        let keys = body_as_json(body).await.unwrap();
        assert!(keys == *KEYS);
    }

    #[tokio::test]
    async fn test_attesting_action() {
        let handler = new_test_handler();

        let req = kms_request(
            "TrentService.Decrypt",
            object! {
               "CiphertextBlob": base64::encode("~~~ ENCRYPTED Hello, World ~~~"),
            },
        );

        let resp = handler.handle(req).await.unwrap();

        let (head, body) = resp.into_parts();

        if head.status == hyper::StatusCode::OK {
            let body = body_as_json(body).await.unwrap();
            assert!(body["Plaintext"].as_str().unwrap() == base64::encode("Hello, World"));
            assert!(body["KeyId"].as_str().unwrap() == KEY_ID);
        } else {
            let bytes = hyper::body::to_bytes(body).await.unwrap();
            let msg = std::str::from_utf8(&bytes).unwrap();
            assert!("DUMMY" == msg);
        }
    }
}
