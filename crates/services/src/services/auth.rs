use std::sync::Arc;

use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AuthContext {
    profile: Arc<RwLock<Option<serde_json::Value>>>,
}

impl AuthContext {
    pub fn new(profile: Arc<RwLock<Option<serde_json::Value>>>) -> Self {
        Self {
            profile,
        }
    }

    pub async fn cached_profile(&self) -> Option<serde_json::Value> {
        self.profile.read().await.clone()
    }

    pub async fn set_profile(&self, profile: serde_json::Value) {
        *self.profile.write().await = Some(profile)
    }

    pub async fn clear_profile(&self) {
        *self.profile.write().await = None
    }
}
