/// Tunnel service configuration for GOST v3 direct routing
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TunnelServiceConfig {
    /// GOST v3 binary path
    pub gost_binary_path: String,
    /// GOST server address (ziso-backend with tunnel.direct=true)
    pub gost_server_addr: String,
    /// JWKS endpoint for ziso-backend to validate vibe-kanban JWTs
    pub jwks_endpoint: String,
    /// Default service port (where vibe-kanban service runs)
    pub default_service_port: u16,
}

impl Default for TunnelServiceConfig {
    fn default() -> Self {
        Self {
            gost_binary_path: std::env::var("GOST_BINARY_PATH")
                .unwrap_or_else(|_| "gost".to_string()),
            // Local GOST server for testing (direct connection to localhost:9000)
            gost_server_addr: std::env::var("GOST_SERVER_ADDR")
                .unwrap_or_else(|_| "localhost:9000".to_string()),
            jwks_endpoint: std::env::var("JWKS_ENDPOINT")
                .unwrap_or_else(|_| "https://vibe-kanban.example.com/.well-known/jwks.json".to_string()),
            // Default service port (frontend for local testing)
            default_service_port: std::env::var("DEFAULT_SERVICE_PORT")
                .unwrap_or_else(|_| "23001".to_string())
                .parse()
                .unwrap_or(23001),
        }
    }
}
