use anyhow::{bail, Context, Result};
use scimeet_core::ScimeetConfig;
use std::path::Path;

pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

pub fn load_scimeet_config(explicit_config: Option<&Path>) -> Result<ScimeetConfig> {
    let mut c = match explicit_config {
        Some(p) if p.exists() => read_toml_file(p)?,
        Some(p) => bail!("config file not found: {}", p.display()),
        None => {
            if let Ok(cwd) = std::env::current_dir() {
                let p = cwd.join("scimeet.toml");
                if p.exists() {
                    read_toml_file(&p)?
                } else {
                    ScimeetConfig::defaults()
                }
            } else {
                ScimeetConfig::defaults()
            }
        }
    };
    apply_env_overrides(&mut c)?;
    Ok(c)
}

fn read_toml_file(path: &Path) -> Result<ScimeetConfig> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("read config {}", path.display()))?;
    ScimeetConfig::from_toml_str(&s).map_err(Into::into)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_u64(name: &str, raw: &str) -> Result<u64> {
    raw.parse::<u64>()
        .with_context(|| format!("{name} must be a non-negative integer, got {raw:?}"))
}

fn parse_usize(name: &str, raw: &str) -> Result<usize> {
    raw.parse::<usize>()
        .with_context(|| format!("{name} must be a non-negative integer, got {raw:?}"))
}

pub fn apply_env_overrides(c: &mut ScimeetConfig) -> Result<()> {
    if let Ok(v) = std::env::var("OLLAMA_HOST") {
        if !v.is_empty() {
            c.ollama_base = v;
        }
    } else if let Ok(v) = std::env::var("SCIMEET_OLLAMA_BASE") {
        if !v.is_empty() {
            c.ollama_base = v;
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_EMBED_MODEL") {
        if !v.is_empty() {
            c.embed_model = v;
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_CHAT_MODEL") {
        if !v.is_empty() {
            c.chat_model = v;
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_TRANSLATE_MODEL") {
        if !v.is_empty() {
            c.translate_model = v;
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_TRANSLATE_ON_QUERY") {
        if let Some(b) = parse_bool(&v) {
            c.translate_on_query = b;
        } else {
            bail!("SCIMEET_TRANSLATE_ON_QUERY must be true/false/1/0, got {v:?}");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_TRANSLATE_ON_INGEST") {
        if let Some(b) = parse_bool(&v) {
            c.translate_on_ingest = b;
        } else {
            bail!("SCIMEET_TRANSLATE_ON_INGEST must be true/false/1/0, got {v:?}");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_TRANSLATE_FALLBACK_TO_ORIGINAL") {
        if let Some(b) = parse_bool(&v) {
            c.translate_fallback_to_original = b;
        } else {
            bail!("SCIMEET_TRANSLATE_FALLBACK_TO_ORIGINAL must be true/false/1/0, got {v:?}");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_DATA_DIR") {
        if !v.is_empty() {
            c.data_dir = v.into();
        }
    }
    if let Ok(v) = std::env::var("NCBI_API_KEY") {
        if !v.is_empty() {
            c.ncbi_api_key = Some(v);
        }
    } else if let Ok(v) = std::env::var("SCIMEET_NCBI_API_KEY") {
        if !v.is_empty() {
            c.ncbi_api_key = Some(v);
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_REQUEST_TIMEOUT_SECS") {
        c.request_timeout_secs = parse_u64("SCIMEET_REQUEST_TIMEOUT_SECS", &v)?;
        if c.request_timeout_secs == 0 {
            bail!("SCIMEET_REQUEST_TIMEOUT_SECS must be greater than zero");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_CONNECT_TIMEOUT_SECS") {
        c.connect_timeout_secs = parse_u64("SCIMEET_CONNECT_TIMEOUT_SECS", &v)?;
        if c.connect_timeout_secs == 0 {
            bail!("SCIMEET_CONNECT_TIMEOUT_SECS must be greater than zero");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_EMBED_DIM") {
        let n = parse_usize("SCIMEET_EMBED_DIM", &v)?;
        if n == 0 {
            bail!("SCIMEET_EMBED_DIM must be greater than zero");
        }
        c.embed_dim = n;
    }
    if let Ok(v) = std::env::var("SCIMEET_HTTP_POOL_MAX_IDLE_PER_HOST") {
        c.http_pool_max_idle_per_host = parse_usize("SCIMEET_HTTP_POOL_MAX_IDLE_PER_HOST", &v)?;
        if c.http_pool_max_idle_per_host == 0 {
            bail!("SCIMEET_HTTP_POOL_MAX_IDLE_PER_HOST must be greater than zero");
        }
    }
    if let Ok(v) = std::env::var("SCIMEET_HTTP_POOL_IDLE_TIMEOUT_SECS") {
        c.http_pool_idle_timeout_secs = parse_u64("SCIMEET_HTTP_POOL_IDLE_TIMEOUT_SECS", &v)?;
    }
    if let Ok(v) = std::env::var("SCIMEET_HTTP_USER_AGENT") {
        if !v.is_empty() {
            c.http_user_agent = v;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_bool_accepts_synonyms() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("TRUE"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("on"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn parse_u64_and_usize_reject_invalid() {
        assert!(parse_u64("X", "abc").is_err());
        assert!(parse_usize("X", "x").is_err());
        assert_eq!(parse_u64("K", "42").unwrap(), 42);
        assert_eq!(parse_usize("K", "7").unwrap(), 7);
    }

    #[test]
    fn defaults_match_core() {
        let c = ScimeetConfig::defaults();
        assert_eq!(c.embed_dim, scimeet_core::default_params::EMBED_DIM);
    }

    #[test]
    fn apply_env_overrides_embed_dim() {
        let _g = ENV_MUTEX.lock().unwrap();
        let prev = std::env::var("SCIMEET_EMBED_DIM").ok();
        std::env::set_var("SCIMEET_EMBED_DIM", "512");
        let mut c = ScimeetConfig::defaults();
        apply_env_overrides(&mut c).unwrap();
        assert_eq!(c.embed_dim, 512);
        match prev {
            Some(v) => std::env::set_var("SCIMEET_EMBED_DIM", v),
            None => std::env::remove_var("SCIMEET_EMBED_DIM"),
        }
    }
}
