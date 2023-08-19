use anyhow::Result;
use async_trait::async_trait;
use http::{Method, Request, Response};
use hyper::header;
use hyper::{Body, StatusCode};
use pkcs8::{DecodePublicKey, SubjectPublicKeyInfo};
use serde::Deserialize;

use crate::http_util::{self, HttpHandler};
use crate::nsm::{AttestationParams, AttestationProvider};

const MIME_APPLICATION_CBOR: &str = "application/cbor";

pub struct ApiHandler {
    attester: Box<dyn AttestationProvider + Send + Sync>,
}

impl ApiHandler {
    pub fn new(attester: Box<dyn AttestationProvider + Send + Sync>) -> Self {
        Self { attester }
    }

    async fn handle_attestation(
        &self,
        _head: &http::request::Parts,
        body: &[u8],
    ) -> Result<Response<Body>> {
        let attestation_req: AttestationRequest = match serde_json::from_slice(body) {
            Ok(req) => req,
            Err(err) => return Ok(http_util::bad_request(err.to_string())),
        };

        let params = match attestation_req.into_params() {
            Ok(params) => params,
            Err(err) => return Ok(http_util::bad_request(err.to_string())),
        };

        let att_doc = self.attester.attestation(params)?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, MIME_APPLICATION_CBOR)
            .body(Body::from(att_doc))?)
    }
}

#[async_trait]
impl HttpHandler for ApiHandler {
    async fn handle(&self, req: Request<Body>) -> Result<Response<Body>> {
        let (head, body) = req.into_parts();
        let body = hyper::body::to_bytes(body).await?;

        match head.uri.path() {
            "/v1/attestation" => match head.method {
                Method::POST => self.handle_attestation(&head, &body).await,

                _ => Ok(http_util::method_not_allowed()),
            },
            _ => Ok(http_util::not_found()),
        }
    }
}

#[derive(Deserialize)]
struct AttestationRequest {
    nonce: Option<String>,
    public_key: Option<String>,
    user_data: Option<String>,
}

impl AttestationRequest {
    fn into_params(self) -> Result<AttestationParams> {
        Ok(AttestationParams {
            nonce: self.nonce.map(base64::decode).transpose()?,
            public_key: self.public_key.map(|s| pem_decode(&s)).transpose()?,
            user_data: self.user_data.map(base64::decode).transpose()?,
        })
    }
}

struct DerPublicKey {
    bytes: Vec<u8>,
}

impl<'a> TryFrom<SubjectPublicKeyInfo<'a>> for DerPublicKey {
    type Error = pkcs8::spki::Error;

    fn try_from(spki: SubjectPublicKeyInfo<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            bytes: spki.subject_public_key.to_vec(),
        })
    }
}

impl DecodePublicKey for DerPublicKey {}

impl DerPublicKey {
    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn pem_decode(pem: &str) -> Result<Vec<u8>> {
    let der = DerPublicKey::from_public_key_pem(pem)?;
    Ok(der.into_bytes())
}

#[tokio::test]
async fn test_attestation_handler() {
    use crate::nsm::StaticAttestationProvider;
    use assert2::assert;

    let handler = ApiHandler::new(Box::new(StaticAttestationProvider::new(Vec::new())));

    let body = json::object!(
        public_key: "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAyY9b3O0t0zDH3pcxYWW2\nTBjW302L3eL+S4C1rmW6OFIXa6U1ZrBtSvMvI3ievCVHq7AOof6xkbXXqobgbokc\n0514+7stOsq/CqnXGWhWwW+aCIj5FFi+gf4kXbXvUYKhUVFFJm5Rq71r5stt3B1p\njYC0Nm391GjR98gO9Sw8TGYx21Q7KuNFsfMa/dtYboFX38fQFw4eTHvSafErgZNO\nMUmzLPibM+1zXqHbXX1M5hyFMBJE28zNi+TmvopdMxsG/a2yTiM1j6Srw2Y5ZrE6\nO1Rr8MxrAepPbmybNOn0K0YIcf/KZurDuvOIuhsurxFgGTVQhsMZ0iNaXA0usFM+\npQIDAQAB\n-----END PUBLIC KEY-----".to_string(),
    );

    let req = Request::builder()
        .method("POST")
        .uri("/v1/attestation")
        .body(Body::from(json::stringify(body)))
        .unwrap();

    let resp = handler.handle(req).await.unwrap();
    assert!(resp.status() == StatusCode::OK);

    let body = json::object!(
        nonce: base64::encode("the nonce"),
        user_data: base64::encode("my data"),
    );

    let req = Request::builder()
        .method("POST")
        .uri("/v1/attestation")
        .body(Body::from(json::stringify(body)))
        .unwrap();

    let resp = handler.handle(req).await.unwrap();
    assert!(resp.status() == StatusCode::OK);
}
