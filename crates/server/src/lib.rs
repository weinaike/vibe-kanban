pub mod config;
pub mod error;
pub mod mcp;
pub mod middleware;
pub mod routes;
pub mod services;

use std::sync::Arc;
use local_deployment::LocalDeployment;
use crate::services::tunnel_manager::TunnelManager;

// #[cfg(feature = "cloud")]
// type DeploymentImpl = vibe_kanban_cloud::deployment::CloudDeployment;
// #[cfg(not(feature = "cloud"))]
pub type DeploymentImpl = LocalDeployment;

/// Application state that includes both deployment and tunnel manager
#[derive(Clone)]
pub struct AppState {
    pub deployment: DeploymentImpl,
    pub tunnel_manager: Arc<TunnelManager>,
}

impl AppState {
    pub fn new(deployment: DeploymentImpl) -> Self {
        let tunnel_config = config::TunnelServiceConfig::default();
        let tunnel_manager = Arc::new(TunnelManager::new(
            tunnel_config.gost_binary_path,
            tunnel_config.gost_server_addr,
        ));
        Self {
            deployment,
            tunnel_manager,
        }
    }

    /// Get database pool
    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.deployment.pool()
    }
}
