use aes::Aes256;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use pleno_value::{RuntimeError, Value};
use sha2::{Digest, Sha256, Sha512};
use std::rc::Rc;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub fn hash(_vm: &mut dyn pleno_value::VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError::new("hash expects 2 arguments: algo, data"));
    }
    let algo = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("algo must be a string")),
    };
    let data = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("data must be a string")),
    };

    match algo {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(data.as_bytes());
            let result = hasher.finalize();
            Ok(Value::String(Rc::new(hex::encode(result))))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(data.as_bytes());
            let result = hasher.finalize();
            Ok(Value::String(Rc::new(hex::encode(result))))
        }
        _ => Err(RuntimeError::new(format!(
            "Unsupported hash algorithm: {}",
            algo
        ))),
    }
}

pub fn hmac(_vm: &mut dyn pleno_value::VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(RuntimeError::new(
            "hmac expects 3 arguments: algo, key, data",
        ));
    }
    let algo = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("algo must be a string")),
    };
    let key = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("key must be a string")),
    };
    let data = match &args[2] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("data must be a string")),
    };

    match algo {
        "sha256" => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
                .map_err(|_| RuntimeError::new("Invalid key length"))?;
            mac.update(data.as_bytes());
            let result = mac.finalize();
            Ok(Value::String(Rc::new(hex::encode(result.into_bytes()))))
        }
        _ => Err(RuntimeError::new(format!(
            "Unsupported HMAC algorithm: {}",
            algo
        ))),
    }
}

pub fn encrypt(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.len() != 4 {
        return Err(RuntimeError::new(
            "encrypt expects 4 arguments: algo, key, iv, data",
        ));
    }
    let algo = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("algo must be a string")),
    };
    let key = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("key must be a string")),
    };
    let iv = match &args[2] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("iv must be a string")),
    };
    let data = match &args[3] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("data must be a string")),
    };

    match algo {
        "aes-256-cbc" => {
            // Key must be 32 bytes hex or raw? Assuming hex for simplicity or 32-char string?
            // Usually keys are binary. Let's assume input is hex encoded string if it needs precision,
            // or just bytes of the string.
            // Requirement usually: key is 32 bytes, IV is 16 bytes.
            // If user provides "secret", we might need to pad or hash it.
            // For stdlib low-level crypto, we expect correct lengths. To be user friendly, let's treat input as bytes.

            // To ensure 32 bytes, we can hash the key if it's not 32 bytes, OR error.
            // Let's expect hex string for key/iv to be precise.

            let key_bytes =
                hex::decode(key).map_err(|_| RuntimeError::new("Key must be hex string"))?;
            let iv_bytes =
                hex::decode(iv).map_err(|_| RuntimeError::new("IV must be hex string"))?;

            if key_bytes.len() != 32 {
                return Err(RuntimeError::new(
                    "AES-256 key must be 32 bytes (64 hex chars)",
                ));
            }
            if iv_bytes.len() != 16 {
                return Err(RuntimeError::new(
                    "AES-256 IV must be 16 bytes (32 hex chars)",
                ));
            }

            let pt_len = data.len();
            let mut buf = vec![0u8; pt_len + 16]; // buffer for padding
            buf[..pt_len].copy_from_slice(data.as_bytes());

            let ct = Aes256CbcEnc::new_from_slices(&key_bytes, &iv_bytes)
                .map_err(|e| RuntimeError::new(format!("Cipher init error: {}", e)))?
                .encrypt_padded_mut::<Pkcs7>(&mut buf, pt_len)
                .map_err(|e| RuntimeError::new(format!("Encryption error: {}", e)))?;

            // Return base64 for ciphertext
            Ok(Value::String(Rc::new(
                base64::prelude::BASE64_STANDARD.encode(ct),
            )))
        }
        _ => Err(RuntimeError::new(format!(
            "Unsupported encryption algorithm: {}",
            algo
        ))),
    }
}

pub fn decrypt(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.len() != 4 {
        return Err(RuntimeError::new(
            "decrypt expects 4 arguments: algo, key, iv, encrypted_data_base64",
        ));
    }
    let algo = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("algo must be a string")),
    };
    let key = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("key must be a string")),
    };
    let iv = match &args[2] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("iv must be a string")),
    };
    let data_b64 = match &args[3] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("data must be a string")),
    };

    match algo {
        "aes-256-cbc" => {
            let key_bytes =
                hex::decode(key).map_err(|_| RuntimeError::new("Key must be hex string"))?;
            let iv_bytes =
                hex::decode(iv).map_err(|_| RuntimeError::new("IV must be hex string"))?;

            if key_bytes.len() != 32 {
                return Err(RuntimeError::new("AES-256 key must be 32 bytes"));
            }
            if iv_bytes.len() != 16 {
                return Err(RuntimeError::new("AES-256 IV must be 16 bytes"));
            }

            let mut buf = base64::prelude::BASE64_STANDARD
                .decode(data_b64)
                .map_err(|e| RuntimeError::new(format!("Base64 decode error: {}", e)))?;

            let pt = Aes256CbcDec::new_from_slices(&key_bytes, &iv_bytes)
                .map_err(|e| RuntimeError::new(format!("Cipher init error: {}", e)))?
                .decrypt_padded_mut::<Pkcs7>(&mut buf)
                .map_err(|e| RuntimeError::new(format!("Decryption error: {}", e)))?;

            Ok(Value::String(Rc::new(
                String::from_utf8(pt.to_vec())
                    .map_err(|_| RuntimeError::new("Decrypted data is not valid UTF-8"))?,
            )))
        }
        _ => Err(RuntimeError::new(format!(
            "Unsupported decryption algorithm: {}",
            algo
        ))),
    }
}

pub fn jwt_sign(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    // args: payload (Map), secret (String)
    if args.len() != 2 {
        return Err(RuntimeError::new(
            "jwt_sign expects 2 arguments: payload, secret",
        ));
    }
    // We treat payload as arbitrary map to serialize to JSON
    let payload_val = &args[0];
    let secret = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("secret must be a string")),
    };

    // Header: default HS256
    let header = serde_json::json!({
        "alg": "HS256",
        "typ": "JWT"
    });

    // Payload to JSON string (using serde_json via Value serialization if possible)
    // Note: We need to convert pleno Value to serde_json::Value or just stringify it if we have a way.
    // std::json has this logic, let's assume we can reuse json serialization or simple map logic.
    // We can use serde_json::to_string(&val) if Value implements Serialize properly OR construct manually.
    // pleno_value::Value implements Serialize.

    let payload_str = super::json::value_to_json_string(payload_val)?;

    let header_str = serde_json::to_string(&header).unwrap();

    let header_b64 = URL_SAFE_NO_PAD.encode(header_str.as_bytes());
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_str.as_bytes());

    let signing_input = format!("{}.{}", header_b64, payload_b64);

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| RuntimeError::new("Invalid secret"))?;
    mac.update(signing_input.as_bytes());
    let sig = mac.finalize();
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig.into_bytes());

    Ok(Value::String(Rc::new(format!(
        "{}.{}.{}",
        header_b64, payload_b64, sig_b64
    ))))
}

pub fn jwt_verify(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    // args: token (String), secret (String) -> Payload (Map) or Error
    if args.len() != 2 {
        return Err(RuntimeError::new(
            "jwt_verify expects 2 arguments: token, secret",
        ));
    }
    let token = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("token must be a string")),
    };
    let secret = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("secret must be a string")),
    };

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(RuntimeError::new("Invalid JWT format"));
    }

    let header_b64 = parts[0];
    let payload_b64 = parts[1];
    let sig_b64 = parts[2];

    let signing_input = format!("{}.{}", header_b64, payload_b64);

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| RuntimeError::new("Invalid secret"))?;
    mac.update(signing_input.as_bytes());
    let expected_sig = mac.finalize().into_bytes();

    let provided_sig = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| RuntimeError::new("Invalid signature encoding"))?;

    if provided_sig != expected_sig.as_slice() {
        return Err(RuntimeError::new("Invalid JWT signature"));
    }

    // Decode payload
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| RuntimeError::new("Invalid payload encoding"))?;

    let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| RuntimeError::new(format!("Payload JSON error: {}", e)))?;

    // Convert serde_json::Value to pleno_value::Value
    // We reuse logic from crates/pleno-stdlib/src/json.rs if accessible, or re-implement simple conversion
    // For this task, re-implementing basic conversion or making json::convert public would be good.
    // Let's assume we can use our own helper or similar. Copied from json.rs or yaml.rs briefly.

    super::json::convert_serde_to_value(vm, &payload_json)
}
