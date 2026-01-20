/// Tunnel service configuration for GOST v3 direct routing
use serde::Deserialize;

/// Helper to get config value with priority:
/// 1. Runtime environment variable (user override)
/// 2. Compile-time default (set during CI/CD build)
/// 3. Fallback value (for local development)
fn get_config_value(
    env_key: &str,
    compile_default: Option<&'static str>,
    fallback: &str,
) -> String {
    // First check runtime environment variable
    if let Ok(value) = std::env::var(env_key) {
        return value;
    }

    // Then check compile-time default (set during CI/CD build)
    if let Some(value) = compile_default {
        return value.to_owned();
    }

    // Finally use fallback value
    fallback.to_string()
}

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
            gost_binary_path: get_config_value("GOST_BINARY_PATH", option_env!("BUILD_GOST_BINARY_PATH"), "gost"),
            gost_server_addr: get_config_value("GOST_SERVER_ADDR", option_env!("BUILD_GOST_SERVER_ADDR"), "localhost:9000"),
            jwks_endpoint: get_config_value(
                "JWKS_ENDPOINT",
                option_env!("BUILD_JWKS_ENDPOINT"),
                "https://vibe-kanban.example.com/.well-known/jwks",
            ),
            default_service_port: get_config_value(
                "DEFAULT_SERVICE_PORT",
                option_env!("BUILD_DEFAULT_SERVICE_PORT"),
                "23001",
            )
            .parse()
            .unwrap_or(23001),
        }
    }
}
