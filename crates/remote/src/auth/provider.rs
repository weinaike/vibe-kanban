use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Duration;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

const USER_AGENT: &str = "VibeKanbanRemote/1.0";

pub const VALIDATE_TOKEN_MAX_RETRIES: u32 = 3;
const RETRY_INTERVAL_SECONDS: u64 = 2;

#[derive(Debug, Clone)]
pub struct AuthorizationGrant {
    pub access_token: SecretString,
    pub token_type: String,
    pub scopes: Vec<String>,
    pub refresh_token: Option<SecretString>,
    pub expires_in: Option<Duration>,
    pub id_token: Option<SecretString>,
}

#[derive(Debug)]
pub struct ProviderUser {
    pub id: String,
    pub login: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Error)]
pub enum TokenValidationError {
    #[error("provider token invalid or revoked")]
    InvalidOrRevoked,
    #[error("provider validation temporarily unavailable: {0}")]
    Temporary(String),
}

impl TokenValidationError {
    fn temporary(message: impl Into<String>) -> Self {
        Self::Temporary(message.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTokenDetails {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn scopes(&self) -> &[&str];
    fn authorize_url(&self, state: &str, redirect_uri: &str) -> Result<Url>;
    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<AuthorizationGrant>;
    async fn fetch_user(&self, access_token: &SecretString) -> Result<ProviderUser>;
    async fn validate_token(
        &self,
        token_details: &ProviderTokenDetails,
        max_retries: u32,
    ) -> Result<Option<ProviderTokenDetails>, TokenValidationError>;
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AuthorizationProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P>(&mut self, provider: P)
    where
        P: AuthorizationProvider + 'static,
    {
        let key = provider.name().to_lowercase();
        self.providers.insert(key, Arc::new(provider));
    }

    pub fn get(&self, provider: &str) -> Option<Arc<dyn AuthorizationProvider>> {
        let key = provider.to_lowercase();
        self.providers.get(&key).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

/// Casdoor OAuth Provider
///
/// Casdoor is an open-source identity and access management (IAM) solution.
/// This provider integrates with Casdoor's OAuth 2.0 / OIDC endpoints.
///
/// API Documentation: https://casdoor.org/docs/oauth
pub struct CasdoorOAuthProvider {
    client: Client,
    client_id: String,
    client_secret: SecretString,
    endpoint: String,
    #[allow(dead_code)]
    organization: String,
    #[allow(dead_code)]
    application: String,
}

impl CasdoorOAuthProvider {
    pub fn new(
        client_id: String,
        client_secret: SecretString,
        endpoint: String,
        organization: String,
        application: String,
    ) -> Result<Self> {
        let client = Client::builder().user_agent(USER_AGENT).build()?;
        Ok(Self {
            client,
            client_id,
            client_secret,
            endpoint,
            organization,
            application,
        })
    }

    fn parse_scopes(scope: Option<String>) -> Vec<String> {
        scope
            .unwrap_or_default()
            .split(' ')
            .filter_map(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed.to_string())
            })
            .collect()
    }

    fn token_exchange_url(&self) -> String {
        format!("{}/api/login/oauth/access_token", self.endpoint)
    }

    fn user_info_url(&self) -> String {
        format!("{}/api/userinfo", self.endpoint)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CasdoorTokenResponse {
    Success {
        access_token: String,
        id_token: Option<String>,
        refresh_token: Option<String>,
        token_type: String,
        scope: Option<String>,
        expires_in: Option<i64>,
    },
    Error {
        error: String,
        error_description: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct CasdoorUser {
    sub: String,
    preferred_username: Option<String>,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

#[async_trait]
impl AuthorizationProvider for CasdoorOAuthProvider {
    fn name(&self) -> &'static str {
        "casdoor"
    }

    fn scopes(&self) -> &[&str] {
        &["openid", "profile", "email"]
    }

    fn authorize_url(&self, state: &str, redirect_uri: &str) -> Result<Url> {
        let mut url = Url::parse(&format!("{}/login/oauth/authorize", self.endpoint))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("client_id", &self.client_id);
            qp.append_pair("response_type", "code");
            qp.append_pair("redirect_uri", redirect_uri);
            qp.append_pair("scope", &self.scopes().join(" "));
            qp.append_pair("state", state);
        }
        Ok(url)
    }

    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<AuthorizationGrant> {
        let response = self
            .client
            .post(self.token_exchange_url())
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.expose_secret()),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await?
            .error_for_status()?;

        match response.json::<CasdoorTokenResponse>().await? {
            CasdoorTokenResponse::Success {
                access_token,
                id_token,
                refresh_token,
                token_type,
                scope,
                expires_in,
            } => Ok(AuthorizationGrant {
                access_token: SecretString::new(access_token.into()),
                token_type,
                scopes: Self::parse_scopes(scope),
                refresh_token: refresh_token.map(|v| SecretString::new(v.into())),
                expires_in: expires_in.map(Duration::seconds),
                id_token: id_token.map(|v| SecretString::new(v.into())),
            }),
            CasdoorTokenResponse::Error {
                error,
                error_description,
            } => {
                let detail = error_description.unwrap_or_else(|| error.clone());
                anyhow::bail!("casdoor token exchange failed: {detail}")
            }
        }
    }

    async fn fetch_user(&self, access_token: &SecretString) -> Result<ProviderUser> {
        let bearer = format!("Bearer {}", access_token.expose_secret());

        let user: CasdoorUser = self
            .client
            .get(self.user_info_url())
            .header("Authorization", bearer)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(ProviderUser {
            id: user.sub,
            login: user.preferred_username,
            email: user.email,
            name: user.name,
            avatar_url: user.picture,
        })
    }

    async fn validate_token(
        &self,
        token_details: &ProviderTokenDetails,
        max_retries: u32,
    ) -> Result<Option<ProviderTokenDetails>, TokenValidationError> {
        let mut attempt = 0;
        let access_token = SecretString::new(token_details.access_token.clone().into_boxed_str());

        loop {
            attempt += 1;

            let response = match self
                .client
                .get(self.user_info_url())
                .header("Authorization", format!("Bearer {}", access_token.expose_secret()))
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(err) => {
                    if attempt >= max_retries {
                        return Err(TokenValidationError::temporary(format!(
                            "casdoor userinfo request failed: {err}"
                        )));
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_INTERVAL_SECONDS))
                        .await;
                    continue;
                }
            };

            match response.status() {
                reqwest::StatusCode::OK => {
                    return Ok(None);
                }
                reqwest::StatusCode::UNAUTHORIZED => {
                    return Err(TokenValidationError::InvalidOrRevoked);
                }
                status if status.is_server_error() => {
                    if attempt >= max_retries {
                        return Err(TokenValidationError::temporary(format!(
                            "casdoor server error: {status}"
                        )));
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_INTERVAL_SECONDS))
                        .await;
                }
                status => {
                    if attempt >= max_retries {
                        return Err(TokenValidationError::temporary(format!(
                            "unexpected casdoor validation status: {status}"
                        )));
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_INTERVAL_SECONDS))
                        .await;
                }
            }
        }
    }
}
