use axum::{
    Router,
    routing::{IntoMakeService, get},
};

use crate::{AppState, DeploymentImpl};

pub mod approvals;
pub mod config;
pub mod containers;
pub mod filesystem;
// pub mod github;
pub mod events;
pub mod oauth;
pub mod execution_processes;
pub mod frontend;
pub mod health;
pub mod images;
pub mod projects;
pub mod repo;
pub mod scratch;
pub mod sessions;
pub mod shared_tasks;
pub mod tags;
pub mod task_attempts;
pub mod tasks;
pub mod tunnels;

pub fn router(deployment: DeploymentImpl) -> IntoMakeService<Router> {
    // Create AppState with tunnel manager
    let app_state = AppState::new(deployment);

    // Create tunnels router with AppState - needs to be nested separately
    let app_state_for_tunnels = app_state.clone();

    // Main router with LocalDeployment state
    let main_router = Router::new()
        .route("/health", get(health::health_check))
        .merge(config::router())
        .nest("/auth", oauth::router())
        .merge(containers::router(&app_state.deployment))
        .merge(projects::router(&app_state.deployment))
        .merge(tasks::router(&app_state.deployment))
        .merge(shared_tasks::router())
        .merge(task_attempts::router(&app_state.deployment))
        .merge(execution_processes::router(&app_state.deployment))
        .merge(tags::router(&app_state.deployment))
        .merge(filesystem::router())
        .merge(repo::router())
        .merge(events::router(&app_state.deployment))
        .merge(approvals::router())
        .merge(scratch::router(&app_state.deployment))
        .merge(sessions::router(&app_state.deployment))
        .nest("/images", images::routes());

    // Build the final router
    Router::new()
        .nest("/api", main_router.with_state(app_state.deployment))
        .nest("/api", tunnels::router().with_state(app_state_for_tunnels))
        .route("/", get(frontend::serve_frontend_root))
        .route("/{*path}", get(frontend::serve_frontend))
        .into_make_service()
}
