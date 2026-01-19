use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::ApiError;

/// Cached JWKS key with expiration
#[derive(Clone)]
struct CachedJwksKey {
    decoding_key: DecodingKey,
    expires_at: Instant,
}

/// Global JWKS cache
struct JwksCache {
    keys: RwLock<std::collections::HashMap<String, CachedJwksKey>>,
}

impl JwksCache {
    fn new() -> Self {
        Self {
            keys: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get a decoding key from cache or fetch from JWKS endpoint
    async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, ApiError> {
        // Check cache first
        {
            let cache = self.keys.read().await;
            if let Some(cached) = cache.get(kid) {
                if cached.expires_at > Instant::now() {
                    tracing::debug!("Using cached JWKS key for kid: {}", kid);
                    return Ok(cached.decoding_key.clone());
                }
            }
        }

        // Cache miss or expired, fetch from JWKS endpoint
        tracing::debug!("Fetching JWKS key for kid: {}", kid);
        let decoding_key = fetch_decoding_key_from_jwks(kid).await?;

        // Cache the key for 1 hour
        let cached = CachedJwksKey {
            decoding_key: decoding_key.clone(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        };

        {
            let mut cache = self.keys.write().await;
            cache.insert(kid.to_string(), cached);
        }

        Ok(decoding_key)
    }

    /// Clear the cache (useful for testing or force refresh)
    async fn clear(&self) {
        let mut cache = self.keys.write().await;
        cache.clear();
    }
}

/// Global JWKS cache instance
static JWKS_CACHE: OnceLock<JwksCache> = OnceLock::new();

/// Get the global JWKS cache instance
fn get_jwks_cache() -> &'static JwksCache {
    JWKS_CACHE.get_or_init(|| JwksCache::new())
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject - user ID
    pub sub: String,
    /// User display name
    pub name: Option<String>,
    /// User email
    pub email: Option<String>,
    /// Expiration time
    pub exp: usize,
}

/// Authentication context extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// User ID from the JWT
    pub user_id: Uuid,
    /// Original claims
    pub claims: Claims,
}

/// Extract bearer token from Authorization header
fn extract_bearer_token(headers: &HeaderMap) -> Result<String, ApiError> {
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| ApiError::BadRequest("Missing Authorization header".to_string()))?
        .to_str()
        .map_err(|_| ApiError::BadRequest("Invalid Authorization header".to_string()))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(ApiError::BadRequest(
            "Authorization header must be Bearer token".to_string(),
        ));
    }

    Ok(auth_header[7..].to_string())
}

/// Get Casdoor URL from environment
fn get_casdoor_url() -> String {
    std::env::var("CASDOOR_URL").unwrap_or_else(|_| "https://auth.yes-tek.com".to_string())
}

/// Get JWT secret from environment (for HS256 fallback)
fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!("JWT_SECRET not set, using default (NOT SECURE for production)");
        "change-this-secret-in-production".to_string()
    })
}

/// Validate JWT token and extract claims (async version)
/// Supports RS256 (Casdoor) - validates signature using JWKS
/// Supports HS256 (custom) - validates with shared secret
pub async fn validate_jwt_token_async(token: &str) -> Result<Claims, ApiError> {
    // First, try to decode the header to determine the algorithm
    let header = decode_header(token)
        .map_err(|e| ApiError::BadRequest(format!("Invalid token header: {}", e)))?;

    tracing::debug!("Token header: alg={:?}, kid={:?}", header.alg, header.kid);

    match header.alg {
        Algorithm::RS256 => {
            // For RS256, we need to validate using JWKS
            validate_rs256_token(token, &header).await
        }
        Algorithm::HS256 => {
            // Fallback to HS256 with shared secret
            validate_hs256_token(token)
        }
        alg => Err(ApiError::BadRequest(format!(
            "Unsupported algorithm: {:?}. Expected RS256 or HS256",
            alg
        ))),
    }
}

/// Validate JWT token and extract claims (sync version for backward compatibility)
/// Note: This will create a runtime if needed - prefer validate_jwt_token_async in async contexts
pub fn validate_jwt_token(token: &str) -> Result<Claims, ApiError> {
    // First, try to decode the header to determine the algorithm
    let header = decode_header(token)
        .map_err(|e| ApiError::BadRequest(format!("Invalid token header: {}", e)))?;

    tracing::debug!("Token header: alg={:?}, kid={:?}", header.alg, header.kid);

    match header.alg {
        Algorithm::RS256 => {
            // For RS256, we need to validate using JWKS
            // Try to use existing runtime or create a new one
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                // Use the existing runtime
                handle.block_on(validate_rs256_token(token, &header))
            } else {
                // Create a new runtime
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| ApiError::BadRequest(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(validate_rs256_token(token, &header))
            }
        }
        Algorithm::HS256 => {
            // Fallback to HS256 with shared secret
            validate_hs256_token(token)
        }
        alg => Err(ApiError::BadRequest(format!(
            "Unsupported algorithm: {:?}. Expected RS256 or HS256",
            alg
        ))),
    }
}

/// Validate RS256 token using JWKS from Casdoor
async fn validate_rs256_token(token: &str, header: &jsonwebtoken::Header) -> Result<Claims, ApiError> {
    // Get the key ID from the token header
    let kid = header.kid.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Token header missing 'kid' claim".to_string())
    })?;

    // Get decoding key from cache (will fetch from JWKS if not cached)
    let cache = get_jwks_cache();
    let decoding_key = cache.get_decoding_key(kid).await?;

    tracing::debug!("Got decoding key for kid: {}", kid);

    // Create validation that doesn't check audience
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_aud = false;  // Disable audience validation

    // Validate the token with RS256 algorithm
    let token_data = decode::<Claims>(
        token,
        &decoding_key,
        &validation,
    )
    .map_err(|e| {
        tracing::error!("JWT validation failed: kind={:?}, message={}", e.kind(), e);
        ApiError::BadRequest(format!("Invalid token: {}", e))
    })?;

    tracing::debug!("JWT validated successfully for sub: {}", token_data.claims.sub);
    Ok(token_data.claims)
}

/// Fetch a specific decoding key from Casdoor JWKS by kid
async fn fetch_decoding_key_from_jwks(kid: &str) -> Result<DecodingKey, ApiError> {
    let jwks_url = format!("{}/.well-known/jwks", get_casdoor_url().trim_end_matches('/'));

    tracing::debug!("Fetching JWKS from: {}", jwks_url);

    let client = reqwest::Client::new();
    let response = client
        .get(&jwks_url)
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to fetch JWKS: {}", e)))?;

    if !response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "JWKS fetch failed with status: {}",
            response.status()
        )));
    }

    #[derive(Deserialize)]
    struct JwksResponse {
        keys: Vec<JwksKey>,
    }

    #[derive(Deserialize)]
    struct JwksKey {
        kid: Option<String>,
        n: Option<String>,
        e: Option<String>,
        kty: Option<String>,
    }

    let jwks: JwksResponse = response
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to parse JWKS: {}", e)))?;

    // Find the key with matching kid
    let jwk = jwks.keys.iter()
        .find(|k| k.kid.as_deref() == Some(kid) && k.kty.as_deref() == Some("RSA"))
        .ok_or_else(|| ApiError::BadRequest(format!("No matching RSA key found for kid: {}", kid)))?;

    // Decode the modulus and exponent
    use base64::Engine;
    let n = jwk.n.as_ref().ok_or_else(|| ApiError::BadRequest("Missing modulus".to_string()))?;
    let e = jwk.e.as_ref().ok_or_else(|| ApiError::BadRequest("Missing exponent".to_string()))?;

    let n_decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(n)
        .map_err(|e| ApiError::BadRequest(format!("Invalid modulus encoding: {}", e)))?;
    let e_decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(e)
        .map_err(|e| ApiError::BadRequest(format!("Invalid exponent encoding: {}", e)))?;

    // Create DecodingKey from RSA components using the rsa crate
    let decoding_key = build_rsa_decoding_key(&n_decoded, &e_decoded)?;

    tracing::debug!("Built RSA decoding key for kid: {}", kid);

    Ok(decoding_key)
}

/// Build RSA DecodingKey from modulus and exponent bytes
/// Uses the rsa crate to properly construct an RSA public key
fn build_rsa_decoding_key(n: &[u8], e: &[u8]) -> Result<DecodingKey, ApiError> {
    use rsa::{pkcs1::EncodeRsaPublicKey, RsaPublicKey};
    use rsa::BigUint;

    // Convert modulus bytes to BigUint (rsa crate re-exports from num_bigint_dig)
    let modulus = BigUint::from_bytes_be(n);

    // Convert exponent bytes to BigUint
    // The exponent is typically small (65537 for RSA), so it's usually 1-3 bytes
    let exponent = BigUint::from_bytes_be(e);

    tracing::debug!("Creating RSA public key: modulus size={} bytes, exponent bytes={}", n.len(), e.len());

    // Create the RSA public key
    let public_key = RsaPublicKey::new(modulus, exponent)
        .map_err(|e| ApiError::BadRequest(format!("Failed to create RSA public key: {}", e)))?;

    // Convert to PKCS#1 DER encoding (requires EncodeRsaPublicKey trait in scope)
    let der_bytes = public_key.to_pkcs1_der()
        .map_err(|e| ApiError::BadRequest(format!("Failed to encode RSA public key: {}", e)))?;

    tracing::debug!("Created RSA DER: {} bytes", der_bytes.as_ref().len());

    Ok(DecodingKey::from_rsa_der(der_bytes.as_ref()))
}

/// Validate HS256 token using shared secret (fallback)
fn validate_hs256_token(token: &str) -> Result<Claims, ApiError> {
    let secret = get_jwt_secret();

    // Create validation that doesn't check audience
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;  // Disable audience validation

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|e| ApiError::BadRequest(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

/// Extract user_id from Authorization header (Bearer token)
pub fn extract_user_id_from_headers(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let token = extract_bearer_token(headers)?;
    let claims = validate_jwt_token(&token)?;

    // Parse user_id from "sub" claim
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::BadRequest("Invalid user_id in token".to_string()))?;

    Ok(user_id)
}

/// Extract user_id from Authorization header (async version - use in async contexts)
/// This version uses validate_jwt_token_async which properly handles async JWKS fetching
pub async fn extract_user_id_from_headers_async(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let token = extract_bearer_token(headers)?;
    let claims = validate_jwt_token_async(&token).await?;

    // Parse user_id from "sub" claim
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::BadRequest("Invalid user_id in token".to_string()))?;

    Ok(user_id)
}

/// Optional authentication middleware - extracts user if token is present, but doesn't require it
pub async fn optional_auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();

    // Try to extract auth context, but don't fail if not present
    if let Ok(user_id) = extract_user_id_from_headers(headers) {
        if let Ok(token) = extract_bearer_token(headers) {
            if let Ok(claims) = validate_jwt_token(&token) {
                let auth_context = AuthContext { user_id, claims };

                // Inject into request extensions
                let mut request = request;
                request.extensions_mut().insert(auth_context);
                return next.run(request).await;
            }
        }
    }

    // No auth or invalid auth, continue without it
    next.run(request).await
}

/// Required authentication middleware - returns 401 if no valid token
pub async fn required_auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();

    let user_id = extract_user_id_from_headers(headers).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let token = extract_bearer_token(headers).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let claims = validate_jwt_token(&token).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let auth_context = AuthContext { user_id, claims };

    let mut request = request;
    request.extensions_mut().insert(auth_context);
    Ok(next.run(request).await)
}

/// Axum extractor for AuthContext from request extensions
pub struct AuthExtractor(pub AuthContext);

impl TryFrom<Request> for AuthExtractor {
    type Error = StatusCode;

    fn try_from(request: Request) -> Result<Self, Self::Error> {
        request
            .extensions()
            .get::<AuthContext>()
            .cloned()
            .map(AuthExtractor)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// Extract user_id from request extensions (requires auth middleware to have run)
pub fn extract_user_id_from_request(request: &Request) -> Result<Uuid, ApiError> {
    request
        .extensions()
        .get::<AuthContext>()
        .map(|ctx| ctx.user_id)
        .ok_or_else(|| ApiError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer test_token_123"),
        );

        let result = extract_bearer_token(&headers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_token_123");
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let headers = HeaderMap::new();
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token_invalid_format() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("InvalidFormat token"));

        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_auth_context_creation() {
        let user_id = Uuid::new_v4();
        let claims = Claims {
            sub: user_id.to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            exp: 1735689600, // Future timestamp
        };

        let auth_context = AuthContext {
            user_id,
            claims: claims.clone(),
        };

        assert_eq!(auth_context.user_id, user_id);
        assert_eq!(auth_context.claims.sub, user_id.to_string());
    }
}
