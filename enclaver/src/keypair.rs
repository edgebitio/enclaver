use anyhow::Result;
use rsa::pkcs8::{EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};

const RSA_KEY_LEN: usize = 2048;

#[derive(Clone)]
pub struct KeyPair {
    pub private: RsaPrivateKey,
    pub public: RsaPublicKey,
}

impl KeyPair {
    pub fn generate() -> Result<Self> {
        let mut rng = rand::thread_rng();
        let private = RsaPrivateKey::new(&mut rng, RSA_KEY_LEN)?;
        let public = RsaPublicKey::from(&private);

        Ok(KeyPair { private, public })
    }

    pub fn from_private(private: RsaPrivateKey) -> Self {
        let public = private.to_public_key();

        Self { private, public }
    }

    pub fn public_key_as_der(&self) -> Result<Vec<u8>> {
        Ok(self.public.to_public_key_der()?.into_vec())
    }

    pub fn public_key_as_pem(&self) -> Result<String> {
        Ok(self.public.to_public_key_pem(LineEnding::LF)?)
    }
}
