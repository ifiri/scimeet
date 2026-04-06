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

#[cfg(test)]
mod tests {
    use super::*;
    use scimeet_core::ScimeetConfig;

    #[test]
    fn should_translate_query_respects_flag_off() {
        let mut c = ScimeetConfig::defaults();
        c.translate_on_query = false;
        assert!(!should_translate_query(&c, "Bonjour le monde entier"));
    }

    #[test]
    fn should_translate_query_skips_empty() {
        let c = ScimeetConfig::defaults();
        assert!(!should_translate_query(&c, "   "));
    }

    #[test]
    fn should_translate_query_english_long_text() {
        let c = ScimeetConfig::defaults();
        let t = "The quick brown fox jumps over the lazy dog. Repeated for detection.";
        assert!(!should_translate_query(&c, t));
    }

    #[test]
    fn should_translate_query_non_english() {
        let c = ScimeetConfig::defaults();
        let t = "Der schnelle braune Fuchs springt über den faulen Hund. Noch ein Satz auf Deutsch.";
        assert!(should_translate_query(&c, t));
    }

    #[test]
    fn should_translate_ingest_respects_flag_off() {
        let mut c = ScimeetConfig::defaults();
        c.translate_on_ingest = false;
        assert!(!should_translate_ingest(
            &c,
            "Der schnelle braune Fuchs springt über den faulen Hund."
        ));
    }

    #[test]
    fn should_translate_ingest_non_english_when_enabled() {
        let mut c = ScimeetConfig::defaults();
        c.translate_on_ingest = true;
        let t = "Der schnelle braune Fuchs springt über den faulen Hund.";
        assert!(should_translate_ingest(&c, t));
    }
}
