use alloc::{format, string::ToString, vec::Vec};
use borsh::io::{Error, ErrorKind, Write};
use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::Verifier;

#[derive(Debug, Clone, PartialEq)]
pub struct ED25519PublicKey(pub [u8; ed25519_dalek::PUBLIC_KEY_LENGTH]);

#[derive(Debug, Clone)]
pub struct Secp256K1PublicKey([u8; 64]);

#[derive(Debug, Clone, PartialEq)]
pub enum PublicKey {
    /// 256 bit elliptic curve based public-key.
    ED25519(ED25519PublicKey),
}

#[derive(Debug, Clone)]
pub enum KeyType {
    ED25519 = 0,
}

/// Signature container supporting different curves.
#[derive(Debug, Clone, PartialEq)]
pub enum Signature {
    ED25519(Vec<u8>),
}

impl Signature {
    /// Verifies that this signature is indeed signs the data with given public key.
    /// Also if public key doesn't match on the curve returns `false`.
    pub fn verify(&self, data: &[u8], public_key: &PublicKey) -> bool {
        match (&self, public_key) {
            (Signature::ED25519(sig_bytes), PublicKey::ED25519(public_key)) => {
                match ed25519_dalek::PublicKey::from_bytes(&public_key.0) {
                    Err(_) => false,
                    Ok(public_key) => {
                        if let Ok(signature) = ed25519_dalek::Signature::from_bytes(sig_bytes) {
                            public_key.verify(data, &signature).is_ok()
                        } else {
                            false
                        }
                    }
                }
            }
        }
    }
}

impl TryFrom<u8> for KeyType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(KeyType::ED25519),
            _unknown_key_type => Err(Error::new(
                ErrorKind::InvalidData,
                format!("unknown key type: {}", value),
            )),
        }
    }
}

impl BorshSerialize for PublicKey {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            PublicKey::ED25519(public_key) => {
                BorshSerialize::serialize(&0u8, writer)?;
                writer.write_all(&public_key.0)?;
            }
        }
        Ok(())
    }
}

impl BorshDeserialize for PublicKey {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> Result<Self, Error> {
        let key_type = KeyType::try_from(u8::deserialize_reader(reader)?)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        match key_type {
            KeyType::ED25519 => Ok(PublicKey::ED25519(ED25519PublicKey(
                BorshDeserialize::deserialize_reader(reader)?,
            ))),
        }
    }
}

impl BorshSerialize for Signature {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            Signature::ED25519(signature) => {
                BorshSerialize::serialize(&0u8, writer)?;
                writer.write_all(signature)?;
            }
        }
        Ok(())
    }
}

impl BorshDeserialize for Signature {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> Result<Self, Error> {
        let key_type = KeyType::try_from(u8::deserialize_reader(reader)?)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        match key_type {
            KeyType::ED25519 => {
                let array: [u8; ed25519_dalek::SIGNATURE_LENGTH] =
                    BorshDeserialize::deserialize_reader(reader)?;
                Ok(Signature::ED25519(array.to_vec()))
            }
        }
    }
}
