use crate::{ScimeetConfig, ScimeetError};
use reqwest::Client;
use std::time::Duration;

pub fn build_http_client(config: &ScimeetConfig) -> Result<Client, ScimeetError> {
    Client::builder()
        .timeout(Duration::from_secs(config.request_timeout_secs))
        .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
        .pool_max_idle_per_host(config.http_pool_max_idle_per_host)
        .pool_idle_timeout(Some(Duration::from_secs(config.http_pool_idle_timeout_secs)))
        .user_agent(config.http_user_agent.as_str())
        .build()
        .map_err(|e| ScimeetError::Http(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_from_defaults() {
        let c = crate::ScimeetConfig::defaults();
        build_http_client(&c).unwrap();
    }
}
