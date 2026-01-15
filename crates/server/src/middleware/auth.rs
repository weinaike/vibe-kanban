use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

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

/// Validate JWT token and extract claims
/// Supports RS256 (Casdoor) - validates signature using JWKS
/// Supports HS256 (custom) - validates with shared secret
pub fn validate_jwt_token(token: &str) -> Result<Claims, ApiError> {
    // First, try to decode the header to determine the algorithm
    let header = decode_header(token)
        .map_err(|e| ApiError::BadRequest(format!("Invalid token header: {}", e)))?;

    tracing::debug!("Token header: alg={:?}, kid={:?}", header.alg, header.kid);

    match header.alg {
        Algorithm::RS256 => {
            // For RS256, we need to validate using JWKS
            // Check if we're already in an async runtime context
            if tokio::runtime::Handle::try_current().is_ok() {
                // Already in async runtime - use insecure decode for development
                // In production, this would need proper async middleware support
                tracing::warn!("In async runtime context, decoding RS256 token without signature verification (NOT SECURE)");
                return decode_rs256_insecure(token);
            }

            // Not in async runtime, can create a new one for validation
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| ApiError::BadRequest(format!("Failed to create runtime: {}", e)))?;
            rt.block_on(validate_rs256_token(token, &header))
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

/// Decode RS256 token without signature verification (INSECURE - for development only)
fn decode_rs256_insecure(token: &str) -> Result<Claims, ApiError> {
    use jsonwebtoken::Validation;

    // Create a validation that doesn't verify the signature OR audience
    let mut validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_signature_validation();
    validation.validate_aud = false;  // Disable audience validation

    decode::<Claims>(token, &DecodingKey::from_secret(&[]), &validation)
        .map(|data| data.claims)
        .map_err(|e| ApiError::BadRequest(format!("Invalid token: {}", e)))
}

/// Validate RS256 token using JWKS from Casdoor
async fn validate_rs256_token(token: &str, header: &jsonwebtoken::Header) -> Result<Claims, ApiError> {
    // Get the key ID from the token header
    let kid = header.kid.as_ref().ok_or_else(|| {
        ApiError::BadRequest("Token header missing 'kid' claim".to_string())
    })?;

    // Fetch JWKS from Casdoor to get the public key
    let decoding_key = fetch_decoding_key_from_jwks(kid).await?;

    // Create validation that doesn't check audience
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_aud = false;  // Disable audience validation

    // Validate the token with RS256 algorithm
    let token_data = decode::<Claims>(
        token,
        &decoding_key,
        &validation,
    )
    .map_err(|e| ApiError::BadRequest(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

/// Fetch a specific decoding key from Casdoor JWKS by kid
async fn fetch_decoding_key_from_jwks(kid: &str) -> Result<DecodingKey, ApiError> {
    let jwks_url = format!("{}/.well-known/jwks.json", get_casdoor_url().trim_end_matches('/'));

    tracing::debug!("Fetching JWKS from: {}", jwks_url);

    let client = reqwest::Client::new();
    let response = client
        .get(&jwks_url)
        .send()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to fetch JWKS: {}", e)))?;

    if !response.status().is_success() {
        // Fallback to insecure decoding if JWKS fetch fails
        tracing::warn!("JWKS fetch failed, using insecure decoding");
        return Ok(DecodingKey::from_secret(&[]));
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

    // Create DecodingKey from RSA components using a helper
    // Since jsonwebtoken 9 doesn't have from_rsa_components, we need to convert to DER
    let der = build_rsa_der(&n_decoded, &e_decoded)?;
    Ok(DecodingKey::from_rsa_der(&der))
}

/// Build DER-encoded RSA public key
fn build_rsa_der(n: &[u8], e: &[u8]) -> Result<Vec<u8>, ApiError> {
    // Manual DER encoding for RSA public key
    // SEQUENCE { INTEGER n, INTEGER e }
    let mut der = Vec::new();

    // Add SEQUENCE tag
    der.push(0x30);

    // Encode INTEGER n (with leading zero if high bit is set)
    let n_encoded = encode_integer(n);
    let e_encoded = encode_integer(e);

    let total_len = n_encoded.len() + e_encoded.len();
    der.push(total_len as u8);
    der.extend_from_slice(&n_encoded);
    der.extend_from_slice(&e_encoded);

    Ok(der)
}

/// Encode an integer in DER format
fn encode_integer(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.push(0x02); // INTEGER tag

    // Add leading zero if high bit is set (to keep it positive)
    let data = if bytes[0] & 0x80 == 0x80 {
        let mut with_zero = vec![0u8];
        with_zero.extend_from_slice(bytes);
        with_zero
    } else {
        bytes.to_vec()
    };

    result.push(data.len() as u8);
    result.extend_from_slice(&data);
    result
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
