//! base64 encoding for byte sequences.
//!
//! `serde_json` encodes a `Vec<u8>` as an array of decimal numbers by default,
//! which inflates the payload 3~4x. Chunk payloads are therefore encoded as
//! base64 (1.33x) which also travels safely through JSON. Deserialization
//! accepts both the base64 string and the legacy number array so that an
//! upgrade can be rolled out gradually.

use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    serializer.serialize_str(&encoded)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use base64::Engine as _;
    use serde::de::Error as _;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        /// Current format: a base64 string.
        Text(String),
        /// Legacy format: an array of decimal numbers.
        Numbers(Vec<u8>),
    }

    match Repr::deserialize(deserializer)? {
        Repr::Text(v) => base64::engine::general_purpose::STANDARD
            .decode(v.as_bytes())
            .map_err(D::Error::custom),
        Repr::Numbers(v) => Ok(v),
    }
}
