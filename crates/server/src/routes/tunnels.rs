use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::Utc;
use db::models::device::{
    CreateDevice, CreateTunnelAccessLog, Device, DeviceStatus, GostClientConfig,
    HeartbeatResponse, RegisterDeviceResponse, TunnelAccessLog,
};
use deployment::Deployment;
use serde::Deserialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};
use utils::response::ApiResponse;

/// Generate random MAC address
fn generate_mac_address() -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        rand::random::<u8>(),
        rand::random::<u8>(),
        rand::random::<u8>(),
        rand::random::<u8>(),
        rand::random::<u8>(),
        rand::random::<u8>()
    )
}

#[derive(Deserialize, TS)]
pub struct DeviceListQuery {
    /// Filter by status: "online" or "offline"
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct DeviceAccessQuery {
    pub t: String,
}

/// List all devices for the authenticated user
pub async fn list_devices(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<Device>>>, ApiError> {
    // TODO: Extract user_id from JWT token
    // For now, use a placeholder - this should be replaced with proper auth
    let user_id = extract_user_id_from_auth(&deployment).await?;

    let devices = Device::find_by_owner(&deployment.db().pool, user_id).await?;

    Ok(ResponseJson(ApiResponse::success(devices)))
}

/// Register a new device (requires authentication)
pub async fn register_device(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<db::models::device::RegisterDeviceRequest>,
) -> Result<ResponseJson<ApiResponse<RegisterDeviceResponse>>, ApiError> {
    // TODO: Extract user_id from JWT token
    let user_id = extract_user_id_from_auth(&deployment).await?;

    let device_id = Uuid::new_v4();
    let tunnel_id = Uuid::new_v4();
    let mac_address = generate_mac_address();

    let create_device = CreateDevice {
        id: device_id,
        tunnel_id,
        owner_id: user_id,
        mac_address,
        name: req.device_name.clone(),
        device_type: None,
        firmware_version: None,
    };

    let device = Device::create(&deployment.db().pool, &create_device).await?;

    // Generate GOST configuration
    let gost_server_addr = std::env::var("GOST_SERVER_ADDR")
        .unwrap_or_else(|_| "localhost:8080".to_string());
    let gateway_base_url = std::env::var("GATEWAY_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let gost_config = GostClientConfig {
        server_addr: gost_server_addr.clone(),
        tunnel_id: tunnel_id.to_string(),
        local_addr: ":0".to_string(),
        forwarder: "127.0.0.1:80".to_string(),
    };

    // Generate access token (simplified - should use proper encryption in production)
    let access_token = generate_access_token(&user_id, &device_id, &tunnel_id);
    let access_url = format!("{}/api/tunnels/device?t={}", gateway_base_url, access_token);

    let response = RegisterDeviceResponse {
        device,
        access_url,
        gost_config,
        heartbeat_interval: 30,
    };

    deployment
        .track_if_analytics_allowed(
            "device_registered",
            serde_json::json!({
                "device_id": device_id.to_string(),
                "tunnel_id": tunnel_id.to_string(),
                "device_name": req.device_name,
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(response)))
}

/// Get a specific device by ID
pub async fn get_device(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Device>>, ApiError> {
    let device = Device::find_by_id(&deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    // TODO: Verify ownership - check if device.owner_id matches authenticated user
    let _user_id = extract_user_id_from_auth(&deployment).await?;

    Ok(ResponseJson(ApiResponse::success(device)))
}

/// Delete a device
pub async fn delete_device(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let user_id = extract_user_id_from_auth(&deployment).await?;

    // Verify ownership before deletion
    let device = Device::find_by_id(&deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    if device.owner_id != user_id {
        return Err(ApiError::Unauthorized);
    }

    Device::delete(&deployment.db().pool, id).await?;

    deployment
        .track_if_analytics_allowed(
            "device_deleted",
            serde_json::json!({
                "device_id": id.to_string(),
                "tunnel_id": device.tunnel_id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// Device heartbeat endpoint (no authentication required - for GOST client)
pub async fn device_heartbeat(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<HeartbeatResponse>>, ApiError> {
    let device = Device::update_status(&deployment.db().pool, id, DeviceStatus::Online).await?;

    Ok(ResponseJson(ApiResponse::success(HeartbeatResponse {
        status: "ok".to_string(),
        last_seen: device.last_seen.unwrap_or_else(Utc::now),
    })))
}

/// Access device via tunnel (token-based authentication)
pub async fn access_device(
    State(deployment): State<DeploymentImpl>,
    Query(params): Query<DeviceAccessQuery>,
) -> Result<ResponseJson<ApiResponse<String>>, ApiError> {
    // Parse and validate access token
    let (_user_id, device_id, tunnel_id) = parse_access_token(&params.t)?;

    // Find device by tunnel_id
    let device = Device::find_by_tunnel_id(&deployment.db().pool, tunnel_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    // Check if device is online
    if matches!(device.status, DeviceStatus::Offline) {
        return Err(ApiError::BadRequest("Device is offline".to_string()));
    }

    // Log access
    let _log = TunnelAccessLog::create(
        &deployment.db().pool,
        &CreateTunnelAccessLog {
            id: Uuid::new_v4(),
            device_id,
            tunnel_id,
            accessed_by: "token".to_string(),
            ip_address: None, // TODO: Extract from request
            user_agent: None, // TODO: Extract from request
            success: true,
        },
    )
    .await;

    // TODO: Proxy to GOST server with tunnel_id
    Ok(ResponseJson(ApiResponse::success(format!(
        "Tunnel access granted for tunnel {}",
        tunnel_id
    ))))
}

/// Generate encrypted access token (simplified version)
fn generate_access_token(user_id: &Uuid, device_id: &Uuid, tunnel_id: &Uuid) -> String {
    // TODO: Use proper encryption (AES-256-GCM) in production
    // For now, use a simple base64-encoded format
    format!("{}.{}.{}", user_id, device_id, tunnel_id)
}

/// Parse access token
fn parse_access_token(token: &str) -> Result<(Uuid, Uuid, Uuid), ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::BadRequest("Invalid token format".to_string()));
    }

    let user_id = Uuid::parse_str(parts[0])
        .map_err(|_| ApiError::BadRequest("Invalid user_id in token".to_string()))?;
    let device_id = Uuid::parse_str(parts[1])
        .map_err(|_| ApiError::BadRequest("Invalid device_id in token".to_string()))?;
    let tunnel_id = Uuid::parse_str(parts[2])
        .map_err(|_| ApiError::BadRequest("Invalid tunnel_id in token".to_string()))?;

    Ok((user_id, device_id, tunnel_id))
}

/// Extract user_id from authentication context
/// TODO: Implement proper JWT extraction
async fn extract_user_id_from_auth(
    _deployment: &DeploymentImpl,
) -> Result<Uuid, ApiError> {
    // Placeholder: Return a default user ID for development
    // In production, this should extract user_id from JWT token in the Authorization header
    // and validate it against the OAuth server

    // For now, use a fixed UUID for development
    Ok(Uuid::new_v4())

    // Production implementation would look something like:
    // let token = extract_bearer_token(request)?;
    // let claims = validate_jwt_token(&token)?;
    // Ok(claims.user_id)
}

pub fn router(_deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/tunnels/devices", get(list_devices).post(register_device))
        .route("/tunnels/device", get(access_device))
        .route("/tunnels/devices/{id}", get(get_device).delete(delete_device))
        .route("/tunnels/devices/{id}/heartbeat", post(device_heartbeat))
}
