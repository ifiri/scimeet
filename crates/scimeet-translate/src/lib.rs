mod ollama;

pub use ollama::OllamaTranslator;

use scimeet_core::ScimeetConfig;

pub fn should_translate_query(config: &ScimeetConfig, text: &str) -> bool {
    if !config.translate_on_query {
        return false;
    }
    should_translate_inner(text)
}

pub fn should_translate_ingest(config: &ScimeetConfig, text: &str) -> bool {
    if !config.translate_on_ingest {
        return false;
    }
    should_translate_inner(text)
}

fn should_translate_inner(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    match whatlang::detect(text) {
        Some(info) => info.lang().code() != "eng",
        None => true,
    }
}
