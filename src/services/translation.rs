use serde::{Deserialize, Serialize};

const LIBRETRANSLATE_URL: &str = "http://localhost:5000/translate";
const SOURCE_LANG: &str = "en";
const TARGET_LANG: &str = "es";

#[derive(Serialize)]
struct TranslateRequest<'a> {
    q: &'a str,
    source: &'a str,
    target: &'a str,
}

#[derive(Deserialize)]
struct TranslateResponse {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[derive(Debug)]
pub enum TranslationError {
    RequestFailed(String),
    ParseFailed(String),
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            TranslationError::ParseFailed(msg) => write!(f, "Parse failed: {}", msg),
        }
    }
}

pub fn translate(text: &str) -> Result<String, TranslationError> {
    translate_with_langs(text, SOURCE_LANG, TARGET_LANG)
}

pub fn translate_with_langs(
    text: &str,
    source: &str,
    target: &str,
) -> Result<String, TranslationError> {
    let client = reqwest::blocking::Client::new();

    let request = TranslateRequest {
        q: text,
        source,
        target,
    };

    let response = client
        .post(LIBRETRANSLATE_URL)
        .json(&request)
        .send()
        .map_err(|e| TranslationError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(TranslationError::RequestFailed(format!(
            "Status: {}",
            response.status()
        )));
    }

    let result: TranslateResponse = response
        .json()
        .map_err(|e| TranslationError::ParseFailed(e.to_string()))?;

    Ok(result.translated_text)
}
