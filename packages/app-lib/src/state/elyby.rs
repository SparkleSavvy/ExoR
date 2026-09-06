use chrono::{DateTime, TimeDelta, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;

use super::minecraft_auth::{
    auth_retry, MinecraftAuthenticationError, MinecraftAuthStep,
};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;

pub const ELYBY_OAUTH_AUTHORIZE_URL: &str = "https://account.ely.by/oauth2/v1";
pub const ELYBY_OAUTH_TOKEN_URL: &str = "https://account.ely.by/api/oauth2/v1/token";
pub const ELYBY_ACCOUNT_INFO_URL: &str = "https://account.ely.by/api/account/v1/info";
pub const ELYBY_SESSION_JOIN_URL: &str = "https://sessionserver.ely.by/session/minecraft/join";

pub const ELYBY_SCOPE: &str = "account_info minecraft_server_session offline_access";
const ELYBY_CLIENT_ID: &str = "elyrinth2";
pub const ELYBY_DEFAULT_REDIRECT_URI: &str = "https://elyrinth-modrinth/oauth";

pub fn elyby_client_id() -> String {
    std::env::var("ELYBY_CLIENT_ID")
        .unwrap_or_else(|_| ELYBY_CLIENT_ID.to_string())
}

pub fn elyby_client_secret() -> Option<String> {
    std::env::var("ELYBY_CLIENT_SECRET")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn elyby_redirect_uri() -> String {
    std::env::var("ELYBY_REDIRECT_URI")
        .unwrap_or_else(|_| ELYBY_DEFAULT_REDIRECT_URI.to_string())
}

pub fn elyby_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: Option<&str>,
) -> Result<url::Url, MinecraftAuthenticationError> {
    let mut url = url::Url::parse(ELYBY_OAUTH_AUTHORIZE_URL)
        .map_err(|source| MinecraftAuthenticationError::InvalidUrl(source.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", ELYBY_SCOPE)
        .append_pair("state", state);
    if let Some(challenge) = challenge {
        url.query_pairs_mut()
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }
    Ok(url)
}

#[derive(Deserialize, Debug)]
pub struct ElyByTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

impl ElyByTokenResponse {
    pub fn expires_at(&self, base: DateTime<Utc>) -> DateTime<Utc> {
        base + TimeDelta::seconds(self.expires_in as i64)
    }
}

#[derive(Deserialize, Debug)]
pub struct ElyByAccount {
    #[allow(dead_code)]
    pub id: i64,
    pub username: String,
    pub uuid: String,
}

async fn elyby_token_request(
    form: &HashMap<&str, String>,
    step: MinecraftAuthStep,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let status;
    let text;
    {
        let res = auth_retry(|| {
            INSECURE_REQWEST_CLIENT
                .post(ELYBY_OAUTH_TOKEN_URL)
                .header("Accept", "application/json")
                .form(form)
                .send()
        })
        .await
        .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;
        status = res.status();
        text = res.text().await.map_err(|source| {
            MinecraftAuthenticationError::Request { source, step }
        })?;
    }
    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source, raw: text, step, status_code: status,
        }
    })
}

pub async fn elyby_exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let mut form = HashMap::new();
    form.insert("client_id", client_id.to_string());
    form.insert("grant_type", "authorization_code".to_string());
    form.insert("code", code.to_string());
    form.insert("redirect_uri", redirect_uri.to_string());
    form.insert("code_verifier", verifier.to_string());
    if let Some(secret) = client_secret {
        form.insert("client_secret", secret.to_string());
    }
    elyby_token_request(&form, MinecraftAuthStep::ElyByToken).await
}

pub async fn elyby_refresh_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let mut form = HashMap::new();
    form.insert("client_id", client_id.to_string());
    form.insert("grant_type", "refresh_token".to_string());
    form.insert("refresh_token", refresh_token.to_string());
    if let Some(secret) = client_secret {
        form.insert("client_secret", secret.to_string());
    }
    elyby_token_request(&form, MinecraftAuthStep::ElyByRefresh).await
}

pub async fn elyby_account_info(
    access_token: &str,
) -> Result<ElyByAccount, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .get(ELYBY_ACCOUNT_INFO_URL)
            .header("Accept", "application/json")
            .bearer_auth(access_token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::ElyByAccountInfo,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::ElyByAccountInfo,
        }
    })?;

    if let StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN = status {
        return Err(MinecraftAuthenticationError::InvalidToken);
    }

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source, raw: text, step: MinecraftAuthStep::ElyByAccountInfo,
            status_code: status,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_builds_correct_query() {
        let url = elyby_authorize_url(
            "elyrinth2",
            "https://elyrinth-modrinth/oauth",
            "abc123",
            Some("challenge"),
        )
        .unwrap();
        assert_eq!(url.as_str().starts_with("https://account.ely.by/oauth2/v1"), true);
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert!(pairs.contains(&("client_id".into(), "elyrinth2".into())));
        assert!(pairs.contains(&("response_type".into(), "code".into())));
        assert!(pairs.contains(&("state".into(), "abc123".into())));
        assert!(pairs.contains(&("code_challenge".into(), "challenge".into())));
    }

    #[test]
    fn parses_token_response() {
        let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT","scope":"account_info minecraft_server_session offline_access"}"#;
        let parsed: ElyByTokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.access_token, "AT");
        assert_eq!(parsed.refresh_token, "RT");
        assert_eq!(parsed.expires_in, 3600);
    }

    #[test]
    fn parses_account_info() {
        let body = r#"{"id":123,"username":"TestUser","uuid":"6e1c2b99-0000-0000-0000-000000000000"}"#;
        let parsed: ElyByAccount = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.username, "TestUser");
    }
}