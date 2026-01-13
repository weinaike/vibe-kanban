// Shared tasks functionality has been removed
// This module is kept for compatibility but provides no routes

use axum::Router;
use crate::DeploymentImpl;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
}
