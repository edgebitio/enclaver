use anyhow::{anyhow, Result};
use serde_bytes::ByteBuf;

pub use aws_nitro_enclaves_nsm_api::api::{Request, Response};

pub struct Nsm {
    fd: i32,
}

impl Nsm {
    pub fn new() -> Self {
        Self {
            fd: aws_nitro_enclaves_nsm_api::driver::nsm_init(),
        }
    }

    pub fn get_random(&self) -> Result<Vec<u8>> {
        match self.process_request(Request::GetRandom {})? {
            Response::GetRandom { random } => Ok(random),

            _ => Err(anyhow!("unexpected response for GetRandom")),
        }
    }

    pub fn attestation(&self, public_key: Option<Vec<u8>>) -> Result<Vec<u8>> {
        let req = Request::Attestation {
            nonce: None,
            user_data: None,
            public_key: public_key.map(ByteBuf::from),
        };

        match self.process_request(req)? {
            Response::Attestation { document } => Ok(document),
            _ => Err(anyhow!("unexpected response for Attestation")),
        }
    }

    fn process_request(&self, req: Request) -> Result<Response> {
        match aws_nitro_enclaves_nsm_api::driver::nsm_process_request(self.fd, req) {
            Response::Error(err) => Err(anyhow!("nsm request failed with: {:?}", err)),
            resp @ _ => Ok(resp),
        }
    }
}

impl Drop for Nsm {
    fn drop(&mut self) {
        aws_nitro_enclaves_nsm_api::driver::nsm_exit(self.fd);
    }
}
