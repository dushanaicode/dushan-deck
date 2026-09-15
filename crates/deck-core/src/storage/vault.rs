use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadCore, OsRng, Payload, rand_core::RngCore},
};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::error::{DeckError, Result};

pub(crate) fn salt() -> Result<[u8; 16]> {
    let mut salt = [0u8; 16];
    OsRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| DeckError::Encryption)?;
    Ok(salt)
}

pub(crate) fn derive(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(DeckError::Invalid("本地口令须为 12–1024 字节"));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    // Version 1 vault parameters are fixed, so a library update cannot change unlock behavior.
    let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| DeckError::Encryption)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| DeckError::Encryption)?;
    Ok(key)
}

pub(crate) fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| DeckError::Encryption)?;
    Ok([nonce.as_slice(), &ciphertext].concat())
}

pub(crate) fn open(key: &[u8; 32], sealed: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if sealed.len() < 28 {
        return Err(DeckError::VaultAuthentication);
    }
    let cipher = Aes256Gcm::new(key.into());
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&sealed[..12]),
            Payload {
                msg: &sealed[12..],
                aad,
            },
        )
        .map_err(|_| DeckError::VaultAuthentication)?;
    Ok(Zeroizing::new(plaintext))
}
