use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use ts_rs::TS;
use utils::{response::ApiResponse};

use crate::{DeploymentImpl, middleware::auth::validate_jwt_token};

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
    // Priority: runtime env var > compile-time default > fallback
    if let Ok(url) = std::env::var("CASDOOR_URL") {
        return url;
    }
    if let Some(url) = option_env!("BUILD_CASDOOR_URL") {
        return url.to_owned();
    }
    "https://auth.yes-tek.com".to_string()
}

fn get_client_id() -> String {
    if let Ok(id) = std::env::var("CASDOOR_CLIENT_ID") {
        return id;
    }
    if let Some(id) = option_env!("BUILD_CASDOOR_CLIENT_ID") {
        return id.to_owned();
    }
    "29fce9095dee17102a87".to_string()
}

fn get_client_secret() -> String {
    if let Ok(secret) = std::env::var("CASDOOR_CLIENT_SECRET") {
        return secret;
    }
    if let Some(secret) = option_env!("BUILD_CASDOOR_CLIENT_SECRET") {
        return secret.to_owned();
    }
    // Client secret is required in production
    // This empty fallback will cause auth to fail
    String::new()
}

/// Check if a host is local/LAN address
/// Returns true for: localhost, 127.0.0.1, 0.0.0.0, .local domains, and private IP ranges
fn is_local_or_lan_host(host: &str) -> bool {
    // Parse host to get hostname (remove port)
    let hostname = host.split(':').next().unwrap_or(host);

    // Localhost variants
    if hostname == "localhost" || hostname == "127.0.0.1" || hostname == "0.0.0.0" || hostname == "::1" {
        return true;
    }

    // .local domains (mDNS/Bonjour)
    if hostname.ends_with(".local") {
        return true;
    }

    // Private IP ranges
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() == 4 {
        // Try to parse IP octets
        if let (Ok(first), Ok(second)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
            // 10.0.0.0/8
            if first == 10 {
                return true;
            }
            // 172.16.0.0/12
            if first == 172 && second >= 16 && second <= 31 {
                return true;
            }
            // 192.168.0.0/16
            if first == 192 && second == 168 {
                return true;
            }
        }
    }

    false
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
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
    headers: HeaderMap,
    Json(req): Json<HandoffInitRequest>,
) -> Json<ApiResponse<HandoffInitResponse>> {
    // Check if request is from local/LAN access
    // Use the origin from request if provided, otherwise check headers
    let host_to_check = if let Some(ref origin) = req.origin {
        // Parse origin to get host
        match url::Url::parse(origin) {
            Ok(parsed_url) => parsed_url.host_str().unwrap_or("localhost").to_string(),
            Err(_) => "localhost".to_string(),
        }
    } else {
        // Use host header
        headers
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost")
            .to_string()
    };

    if !is_local_or_lan_host(&host_to_check) {
        tracing::warn!("Login attempt from non-local/LAN host: {}", host_to_check);
        return Json(ApiResponse::error("Login is only allowed from local/LAN access"));
    }

    let handoff_id = uuid::Uuid::new_v4().to_string();

    // Build the Casdoor authorization URL for Authorization Code flow
    // Use the origin from request if provided (from frontend's window.location.origin)
    // otherwise fall back to header-based detection
    let redirect_uri = if let Some(origin) = req.origin {
        tracing::info!("Using origin from request: {}", origin);
        format!("{}/api/auth/handoff/complete", origin)
    } else {
        get_callback_url(&headers)
    };
    tracing::info!("OAuth handoff initiated: {} with redirect_uri: {}", handoff_id, redirect_uri);

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
    headers: HeaderMap,
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
        let frontend_origin = get_frontend_origin(&headers);
        return Redirect::to(&frontend_origin);
    }

    // Handle Authorization Code flow
    if let Some(code) = params.code {
        // Mark this state as processed BEFORE token exchange to avoid Send issues
        if let Some(state) = &params.state {
            mark_state_processed(state).await;
        }

        // Exchange code for token
        match exchange_code_for_token(&code, &headers).await {
            Ok(token_response) => {
                // Get the frontend origin from environment or request headers
                let frontend_origin = get_frontend_origin(&headers);
                
                // Redirect to frontend with tokens in hash
                let hash = format!(
                    "access_token={}&token_type={}&expires_in={}",
                    token_response.access_token,
                    token_response.token_type,
                    token_response.expires_in.unwrap_or(3600)
                );
                let redirect_url = format!("{}/#/callback#{}", frontend_origin, hash);
                tracing::info!("Redirecting to frontend: {}", redirect_url);
                return Redirect::to(&redirect_url);
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
async fn auth_status(headers: HeaderMap) -> Json<ApiResponse<AuthStatusResponse>> {
    // Try to extract and validate the JWT token from Authorization header
    let auth_header = headers.get("Authorization");
    let is_signed_in = auth_header.is_some();

    if !is_signed_in {
        return Json(ApiResponse::success(AuthStatusResponse {
            is_signed_in: false,
            user_id: None,
            display_name: None,
            email: None,
        }));
    }

    // Validate the token
    match auth_header.and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|t| validate_jwt_token(t))
    {
        Some(Ok(claims)) => Json(ApiResponse::success(AuthStatusResponse {
            is_signed_in: true,
            user_id: Some(claims.sub.clone()),
            display_name: claims.name.clone(),
            email: claims.email.clone(),
        })),
        _ => Json(ApiResponse::success(AuthStatusResponse {
            is_signed_in: false,
            user_id: None,
            display_name: None,
            email: None,
        })),
    }
}

/// Logout request body with access token
#[derive(Debug, Deserialize, TS)]
pub struct LogoutRequest {
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// Logout
async fn logout(
    headers: HeaderMap,
    Json(body): Json<LogoutRequest>,
) -> Json<ApiResponse<LogoutResponse>> {
    let casdoor_url = get_casdoor_url();
    let client_id = get_client_id();
    let app_name = std::env::var("CASDOOR_APP_NAME").unwrap_or_else(|_| "application_ziso".to_string());

    // Build the Casdoor logout URL
    // With access token (id_token_hint) and redirect URI, Casdoor will redirect back after logout
    // Use the origin from request if provided (from frontend's window.location.origin)
    // otherwise fall back to header-based detection
    let logout_url = if let Some(token) = body.access_token {
        let callback_url = if let Some(origin) = body.origin {
            tracing::info!("Using origin from request for logout: {}", origin);
            format!("{}/api/auth/handoff/complete", origin)
        } else {
            get_callback_url(&headers)
        };
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
async fn get_token(headers: HeaderMap) -> Json<ApiResponse<Option<TokenResponse>>> {
    // Extract token from Authorization header
    let auth_header = match headers.get("Authorization") {
        Some(h) => h,
        None => return Json(ApiResponse::success(None)),
    };

    let token_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => return Json(ApiResponse::success(None)),
    };

    let token = match token_str.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return Json(ApiResponse::success(None)),
    };

    // Validate the token and return response
    match validate_jwt_token(token) {
        Ok(_) => Json(ApiResponse::success(Some(TokenResponse {
            access_token: token.to_string(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_in: None,
        }))),
        Err(_) => Json(ApiResponse::success(None)),
    }
}

/// Get current user info
async fn get_user(headers: HeaderMap) -> Json<ApiResponse<Option<CurrentUserResponse>>> {
    // Extract token from Authorization header
    let auth_header = match headers.get("Authorization") {
        Some(h) => h,
        None => return Json(ApiResponse::success(None)),
    };

    let token_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => return Json(ApiResponse::success(None)),
    };

    let token = match token_str.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return Json(ApiResponse::success(None)),
    };

    // Validate the token and extract user info
    match validate_jwt_token(token) {
        Ok(claims) => Json(ApiResponse::success(Some(CurrentUserResponse {
            id: claims.sub.clone(),
            name: claims.name.clone().unwrap_or_else(|| claims.sub.clone()),
            display_name: claims.name.clone().unwrap_or_default(),
            email: claims.email.clone(),
            avatar: None,
        }))),
        Err(_) => Json(ApiResponse::success(None)),
    }
}

/// Exchange authorization code for access token
async fn exchange_code_for_token(
    code: &str,
    headers: &HeaderMap,
) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let redirect_uri = get_callback_url(headers);
    let casdoor_url = get_casdoor_url();
    let client_id = get_client_id();
    let client_secret = get_client_secret();

    tracing::info!("Exchanging code for token with Casdoor, redirect_uri: {}", redirect_uri);

    if client_secret.is_empty() {
        tracing::error!("CASDOOR_CLIENT_SECRET is not set!");
        return Err("CASDOOR_CLIENT_SECRET environment variable is not set".into());
    }

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("code", code),
        ("redirect_uri", &redirect_uri),
    ];

    tracing::debug!("Token exchange params: client_id={}, redirect_uri={}", client_id, redirect_uri);

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

/// Get the callback URL dynamically from request headers or environment variable
fn get_callback_url(headers: &HeaderMap) -> String {
    // Check for environment variable first (for production deployments)
    // But skip it if it looks like a fixed domain and we have better header info
    let env_callback = std::env::var("OAUTH_CALLBACK_URL").ok();
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok());

    // If we have a host header and it differs from env, use headers (dynamic access)
    if let Some(host_header) = host {
        if let Some(env_url) = &env_callback {
            if let Ok(env_url_parsed) = url::Url::parse(env_url) {
                // If env URL host differs from request host, prefer request headers
                if env_url_parsed.host_str() != Some(host_header.split(':').next().unwrap_or(host_header)) {
                    tracing::info!("Request host {} differs from OAUTH_CALLBACK_URL host {}, using request headers for dynamic access", host_header, env_url_parsed.host_str().unwrap_or("none"));
                } else {
                    tracing::debug!("Using OAUTH_CALLBACK_URL from environment: {}", env_url);
                    return env_url.clone();
                }
            }
        }
    } else if let Some(env_url) = &env_callback {
        tracing::debug!("Using OAUTH_CALLBACK_URL from environment: {}", env_url);
        return env_url.clone();
    }

    // Extract origin from headers (Host + scheme)
    let host = host.unwrap_or("localhost:8080");

    // Get X-Forwarded-Proto if exists
    let forwarded_proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok());

    // Get X-Forwarded-Host if exists (for reverse proxy scenarios)
    let forwarded_host = headers
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok());

    // Use forwarded host if available, otherwise use original host
    let effective_host = forwarded_host.unwrap_or(host);

    // Determine scheme from X-Forwarded-Proto
    // Default to http for localhost/127.0.0.1, https for others
    let scheme = forwarded_proto.unwrap_or_else(|| {
        if effective_host.starts_with("localhost") || effective_host.starts_with("127.0.0.1") {
            "http"
        } else {
            "https"
        }
    });

    let url = format!("{}://{}/api/auth/handoff/complete", scheme, effective_host);
    tracing::info!(
        "Building callback URL: host={}, forwarded-host={:?}, forwarded-proto={:?}, effective_host={}, scheme={}, url={}",
        host,
        forwarded_host,
        forwarded_proto,
        effective_host,
        scheme,
        url
    );

    url
}

/// Get the frontend origin for redirects after OAuth completion
fn get_frontend_origin(headers: &HeaderMap) -> String {
    // Check for environment variable first (for production deployments)
    // But skip it if it looks like a fixed domain and we have better header info
    let env_origin = std::env::var("FRONTEND_ORIGIN").ok();
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok());

    // If we have a host header and it differs from env, use headers (dynamic access)
    if let Some(host_header) = host {
        if let Some(env_url) = &env_origin {
            if let Ok(env_url_parsed) = url::Url::parse(env_url) {
                // If env URL host differs from request host, prefer request headers
                let host_without_port = host_header.split(':').next().unwrap_or(host_header);
                if env_url_parsed.host_str() != Some(host_without_port) {
                    tracing::info!("Request host {} differs from FRONTEND_ORIGIN host {}, using request headers for dynamic access", host_header, env_url_parsed.host_str().unwrap_or("none"));
                } else {
                    tracing::debug!("Using FRONTEND_ORIGIN from environment: {}", env_url);
                    return env_url.clone();
                }
            }
        }
    } else if let Some(env_url) = &env_origin {
        tracing::debug!("Using FRONTEND_ORIGIN from environment: {}", env_url);
        return env_url.clone();
    }

    // Try to get the Referer header (where the user came from) - most reliable for proxied requests
    if let Some(referer) = headers.get("referer").and_then(|h| h.to_str().ok()) {
        // Extract origin from referer URL
        if let Ok(url) = url::Url::parse(referer) {
            if let Some(host) = url.host_str() {
                let scheme = url.scheme();
                let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                let origin = format!("{}://{}{}", scheme, host, port);
                tracing::info!("Extracted frontend origin from referer: {}", origin);
                return origin;
            }
        }
    }

    // Fallback: try to construct from host header
    let host = host.unwrap_or("localhost:23001");

    // Get X-Forwarded-Proto and X-Forwarded-Host if exists (for reverse proxy scenarios)
    let forwarded_proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok());
    let forwarded_host = headers
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok());

    // Use forwarded host if available, otherwise use original host
    let effective_host = forwarded_host.unwrap_or(host);

    // Determine scheme from X-Forwarded-Proto
    let scheme = forwarded_proto.unwrap_or_else(|| {
        if effective_host.starts_with("localhost") || effective_host.starts_with("127.0.0.1") {
            "http"
        } else {
            "https"
        }
    });

    // For local development, assume frontend is on the next port down if we're on default backend port
    // This handles the case where backend is 23002 and frontend is 23001
    let frontend_host = if effective_host.contains(":23002") {
        effective_host.replace(":23002", ":23001")
    } else if effective_host.contains("127.0.0.1") || effective_host.contains("localhost") {
        // Default local frontend - explicitly use 23001
        "localhost:23001".to_string()
    } else {
        // Production: frontend and backend on same origin
        effective_host.to_string()
    };

    let origin = format!("{}://{}", scheme, frontend_host);
    tracing::info!(
        "Building frontend origin: host={}, forwarded-host={:?}, forwarded-proto={:?}, effective_host={}, frontend_host={}, origin={}",
        host,
        forwarded_host,
        forwarded_proto,
        effective_host,
        frontend_host,
        origin
    );

    origin
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
