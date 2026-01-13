use axum::{
    Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use ts_rs::TS;
use utils::{response::ApiResponse};

use crate::DeploymentImpl;

// Global in-memory store for processed OAuth states
static PROCESSED_STATES: tokio::sync::OnceCell<Arc<RwLock<HashSet<String>>>> = tokio::sync::OnceCell::const_new();

async fn get_processed_states() -> Arc<RwLock<HashSet<String>>> {
    PROCESSED_STATES.get_or_init(|| async {
        Arc::new(RwLock::new(HashSet::new()))
    }).await.clone()
}

async fn is_state_processed(state: &str) -> bool {
    let states = get_processed_states().await;
    states.read().await.contains(state)
}

async fn mark_state_processed(state: &str) {
    let states = get_processed_states().await;
    states.write().await.insert(state.to_string());
}

fn get_casdoor_url() -> String {
    std::env::var("CASDOOR_URL").unwrap_or_else(|_| "https://auth.yes-tek.com".to_string())
}

fn get_client_id() -> String {
    std::env::var("CASDOOR_CLIENT_ID").unwrap_or_else(|_| "29fce9095dee17102a87".to_string())
}

fn get_client_secret() -> String {
    std::env::var("CASDOOR_CLIENT_SECRET").unwrap_or_else(|_| "".to_string())
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/handoff/init", post(handoff_init))
        .route("/handoff/complete", get(handoff_complete))
        .route("/status", get(auth_status))
        .route("/logout", post(logout))
        .route("/token", get(get_token))
        .route("/user", get(get_user))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct HandoffInitRequest {
    pub provider: String,
    pub return_to: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct HandoffInitResponse {
    pub handoff_id: String,
    pub authorize_url: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct HandoffCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct AuthStatusResponse {
    pub is_signed_in: bool,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CurrentUserResponse {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
}

/// Initialize OAuth handoff - generates authorization URL for Authorization Code flow
async fn handoff_init(
    State(_deployment): State<DeploymentImpl>,
    Json(_req): Json<HandoffInitRequest>,
) -> Json<ApiResponse<HandoffInitResponse>> {
    let handoff_id = uuid::Uuid::new_v4().to_string();

    // Build the Casdoor authorization URL for Authorization Code flow
    let redirect_uri = get_callback_url();
    let casdoor_url = get_casdoor_url();
    let client_id = get_client_id();

    let authorize_url = format!(
        "{}/login/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=openid+profile+email&state={}",
        casdoor_url,
        client_id,
        percent_encode(&redirect_uri),
        handoff_id
    );

    tracing::info!("OAuth handoff initiated: {}", handoff_id);

    Json(ApiResponse::success(HandoffInitResponse {
        handoff_id,
        authorize_url,
    }))
}

/// Handle OAuth callback from Casdoor - Authorization Code flow
#[axum::debug_handler]
async fn handoff_complete(
    Query(params): Query<HandoffCallbackParams>,
) -> impl IntoResponse {
    tracing::info!("OAuth callback received: {:?}", params);

    // Check if this state has already been processed (prevent duplicate requests)
    if let Some(state) = &params.state {
        if is_state_processed(state).await {
            tracing::info!("OAuth callback already processed for state: {}, skipping", state);
            // Redirect to home page since this was already processed
            return Redirect::to("/");
        }
    }

    // Handle error from Casdoor
    if let Some(error) = &params.error {
        let description = params.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("OAuth error: {} - {}", error, description);
        return Redirect::to(&format!("/?error={}", error));
    }

    // Handle logout callback - no code or error means redirect from logout
    if params.code.is_none() && params.error.is_none() {
        tracing::info!("Logout callback received, redirecting to home");
        return Redirect::to("/");
    }

    // Handle Authorization Code flow
    if let Some(code) = params.code {
        // Mark this state as processed BEFORE token exchange to avoid Send issues
        if let Some(state) = &params.state {
            mark_state_processed(state).await;
        }

        // Exchange code for token
        match exchange_code_for_token(&code).await {
            Ok(token_response) => {
                // Redirect to frontend with tokens in hash
                let hash = format!(
                    "access_token={}&token_type={}&expires_in={}",
                    token_response.access_token,
                    token_response.token_type,
                    token_response.expires_in.unwrap_or(3600)
                );
                return Redirect::to(&format!("/#/callback#{}", hash));
            }
            Err(e) => {
                tracing::error!("Failed to exchange code for token: {}", e);
                return Redirect::to("/?error=token_exchange_failed");
            }
        }
    }

    Redirect::to("/?error=missing_code")
}

/// Get authentication status
async fn auth_status() -> Json<ApiResponse<AuthStatusResponse>> {
    // TODO: Implement session storage and token validation
    // For now, return not signed in
    Json(ApiResponse::success(AuthStatusResponse {
        is_signed_in: false,
        user_id: None,
        display_name: None,
        email: None,
    }))
}

/// Logout request body with access token
#[derive(Debug, Deserialize, TS)]
pub struct LogoutRequest {
    pub access_token: Option<String>,
}

/// Logout
async fn logout(Json(body): Json<LogoutRequest>) -> Json<ApiResponse<LogoutResponse>> {
    let casdoor_url = get_casdoor_url();
    let client_id = get_client_id();
    let app_name = std::env::var("CASDOOR_APP_NAME").unwrap_or_else(|_| "application_ziso".to_string());

    // Build the Casdoor logout URL
    // With access token (id_token_hint) and redirect URI, Casdoor will redirect back after logout
    // Use the already-configured callback URL as redirect URI to avoid validation errors
    let logout_url = if let Some(token) = body.access_token {
        let callback_url = get_callback_url();
        format!(
            "{}/api/logout?id_token_hint={}&post_logout_redirect_uri={}",
            casdoor_url,
            percent_encode(&token),
            percent_encode(&callback_url)
        )
    } else {
        // Fallback: no token, just clear session (no redirect)
        format!(
            "{}/api/logout?client_id={}&application={}",
            casdoor_url,
            client_id,
            app_name
        )
    };

    Json(ApiResponse::success(LogoutResponse { logout_url }))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct LogoutResponse {
    pub logout_url: String,
}

/// Get current access token
async fn get_token() -> Json<ApiResponse<Option<TokenResponse>>> {
    // TODO: Implement token retrieval from session
    Json(ApiResponse::success(None))
}

/// Get current user info
async fn get_user() -> Json<ApiResponse<Option<CurrentUserResponse>>> {
    // TODO: Implement user info retrieval from session
    Json(ApiResponse::success(None))
}

/// Exchange authorization code for access token
async fn exchange_code_for_token(
    code: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let redirect_uri = get_callback_url();
    let casdoor_url = get_casdoor_url();
    let client_id = get_client_id();
    let client_secret = get_client_secret();

    if client_secret.is_empty() {
        return Err("CASDOOR_CLIENT_SECRET environment variable is not set".into());
    }

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("code", code),
        ("redirect_uri", &redirect_uri),
    ];

    tracing::info!("Exchanging code for token with Casdoor");

    let response = client
        .post(format!("{}/api/login/oauth/access_token", casdoor_url))
        .form(&params)
        .send()
        .await?;

    if response.status().is_success() {
        let token_response: CasdoorTokenResponse = response.json().await?;
        tracing::info!("Token exchange successful");
        Ok(TokenResponse {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            token_type: token_response.token_type,
            expires_in: token_response.expires_in,
        })
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        tracing::error!("Token exchange failed: {} - {}", status, error_text);
        Err(format!("Token exchange failed: {}", error_text).into())
    }
}

/// Get the callback URL based on environment
fn get_callback_url() -> String {
    std::env::var("OAUTH_CALLBACK_URL")
        .unwrap_or_else(|_| "http://ziso.yes-tek.com/api/auth/handoff/complete".to_string())
}

/// Simple percent encoding for URL parameters
fn percent_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | ':' | '/' => {
                vec![c as u8]
            }
            _ => {
                let mut encoded = vec![b'%'];
                let b = c as u8;
                encoded.push(HEX_CHARS[(b >> 4) as usize]);
                encoded.push(HEX_CHARS[(b & 0x0F) as usize]);
                encoded
            }
        })
        .map(|b| b as char)
        .collect()
}

const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";

/// Casdoor token response structure
#[derive(Debug, Serialize, Deserialize)]
struct CasdoorTokenResponse {
    access_token: String,
    #[serde(rename = "token_type")]
    token_type: String,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    id_token: Option<String>,
}
