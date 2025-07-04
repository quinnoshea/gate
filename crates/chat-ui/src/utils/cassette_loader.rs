use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Raw HTTP request data from cassette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRequest {
    pub uri: String,
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, Vec<String>>,
    pub body: String,
}

/// Raw HTTP response data from cassette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResponse {
    #[serde(default = "default_status", deserialize_with = "deserialize_status")]
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, Vec<String>>,
    pub body: ResponseBody,
}

fn default_status() -> u16 {
    200
}

/// Deserialize status from either a number or an object with a "code" field
fn deserialize_status<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use serde_json::Value;

    struct StatusVisitor;

    impl<'de> Visitor<'de> for StatusVisitor {
        type Value = u16;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u16 or an object with a 'code' field")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value as u16)
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut code = None;

            while let Some(key) = map.next_key::<String>()? {
                if key == "code" {
                    code = Some(map.next_value::<u16>()?);
                } else {
                    // Skip other fields like "message"
                    map.next_value::<Value>()?;
                }
            }

            code.ok_or_else(|| de::Error::missing_field("code"))
        }
    }

    deserializer.deserialize_any(StatusVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseBody {
    pub string: String,
}

/// A single HTTP interaction from a cassette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteInteraction {
    pub request: RawRequest,
    pub response: RawResponse,
}

/// Root structure of a cassette file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CassetteFile {
    interactions: Vec<CassetteInteraction>,
}

/// Load a cassette file and return the first interaction
///
/// This is a simple, dumb loader that just parses the JSON
/// and returns the raw HTTP data without any interpretation
pub fn load_cassette(json_content: &str) -> Result<CassetteInteraction> {
    let cassette: CassetteFile = serde_json::from_str(json_content)?;

    cassette
        .interactions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No interactions found in cassette"))
}

/// Load all interactions from a cassette file
pub fn load_all_interactions(json_content: &str) -> Result<Vec<CassetteInteraction>> {
    let cassette: CassetteFile = serde_json::from_str(json_content)?;
    Ok(cassette.interactions)
}
