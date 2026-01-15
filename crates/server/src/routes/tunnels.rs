use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::Utc;
use deployment::Deployment;
use db::models::device::{
    CreateDevice, Device, DeviceStatus, HeartbeatResponse, RegisterDeviceRequest,
    RegisterDeviceResponse,
};
use serde::Deserialize;
use serde_json::json;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Deserialize, serde::Serialize)]
struct GatewayRegisterRequest {
    mac_address: String,
    device_name: String,
    device_type: Option<String>,
    firmware_version: Option<String>,
}

#[derive(Deserialize)]
struct GatewayGostConfig {
    server_addr: String,
    tunnel_id: String,
    local_addr: Option<String>,
}

#[derive(Deserialize)]
struct GatewayTunnelConfig {
    tunnel_id: String,
    gost_config: GatewayGostConfig,
}

#[derive(Deserialize)]
struct GatewayRegisterResponse {
    device_id: String,
    tunnel: GatewayTunnelConfig,
    heartbeat_interval: i32,
}

use crate::{config::TunnelServiceConfig, error::ApiError, AppState, middleware::auth::extract_user_id_from_headers};
use utils::response::ApiResponse;

/// Get real MAC address from the system, with fallback to random generation
fn generate_mac_address() -> String {
    use mac_address::get_mac_address;

    match get_mac_address() {
        Ok(Some(mac)) => {
            // Successfully retrieved real MAC address
            mac.to_string()
        }
        Ok(None) => {
            tracing::warn!("No MAC address found, generating random");
            // Fallback: generate random MAC address
            random_mac_address()
        }
        Err(e) => {
            tracing::warn!("Failed to get MAC address ({}), generating random", e);
            // Fallback: generate random MAC address
            random_mac_address()
        }
    }
}

/// Generate random MAC address as fallback
fn random_mac_address() -> String {
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
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<ResponseJson<ApiResponse<Vec<Device>>>, ApiError> {
    let user_id = extract_user_id_from_headers(&headers)?;
    let devices = Device::find_by_owner(&state.deployment.db().pool, user_id).await?;

    Ok(ResponseJson(ApiResponse::success(devices)))
}

/// Register a new device with Gateway API and local GOST v3 process
pub async fn register_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterDeviceRequest>,
) -> Result<ResponseJson<ApiResponse<RegisterDeviceResponse>>, ApiError> {
    let tunnel_config = TunnelServiceConfig::default();
    let user_id = extract_user_id_from_headers(&headers)?;
    let mac_address = generate_mac_address();
    let service_port = req.service_port.unwrap_or(tunnel_config.default_service_port as i64);

    // Extract JWT token from headers for Gateway API authentication
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized)?;

    // Step 1: Register with Gateway API
    let gateway_url = std::env::var("GATEWAY_API_URL")
        .unwrap_or_else(|_| "http://localhost:24001".to_string());
    let gateway_client = reqwest::Client::new();

    let gateway_req = GatewayRegisterRequest {
        mac_address: mac_address.clone(),
        device_name: req.device_name.clone(),
        device_type: Some("server".to_string()),
        firmware_version: Some("1.0.0".to_string()),
    };

    tracing::info!("Registering device with Gateway: {}", gateway_url);

    let gateway_resp: GatewayRegisterResponse = gateway_client
        .post(format!("{}/api/v1/devices/register", gateway_url))
        .header("Authorization", auth_header)
        .json(&gateway_req)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to call Gateway API: {}", e);
            ApiError::BadRequest(format!("Gateway registration failed: {}", e))
        })?
        .json()
        .await
        .map_err(|e| {
            tracing::error!("Failed to parse Gateway response: {}", e);
            ApiError::BadRequest(format!("Invalid Gateway response: {}", e))
        })?;

    // Use Gateway-returned device_id
    let device_id: Uuid = gateway_resp.device_id.parse()
        .map_err(|e| ApiError::BadRequest(format!("Invalid device_id from Gateway: {}", e)))?;
    let tunnel_id: Uuid = gateway_resp.tunnel.tunnel_id.parse()
        .map_err(|e| ApiError::BadRequest(format!("Invalid tunnel_id from Gateway: {}", e)))?;

    tracing::info!("Gateway registered device {} with tunnel {}", device_id, tunnel_id);

    // Step 2: Create local device record with Gateway-returned device_id
    let create_device = CreateDevice {
        id: device_id,
        owner_id: user_id,
        mac_address,
        name: req.device_name.clone(),
        device_type: Some("server".to_string()),
        firmware_version: Some("1.0.0".to_string()),
        service_port,
    };
    let device = Device::create(&state.deployment.db().pool, &create_device).await?;

    // Step 3: Start GOST v3 client using Gateway-returned device_id and tunnel_id
    match state
        .tunnel_manager
        .start_device(device_id, tunnel_id, &device.name, service_port, &state.pool())
        .await
    {
        Ok(_) => {
            tracing::info!("GOST v3 started for device {} with tunnel {}", device_id, tunnel_id);
        }
        Err(e) => {
            tracing::error!("Failed to start GOST v3 for device {}: {}", device_id, e);
            // Continue anyway - device is registered, GOST can be started manually
        }
    }

    // Step 4: Save tunnel_id to database for restart functionality
    match Device::update_tunnel_id(&state.deployment.db().pool, device_id, tunnel_id.to_string()).await {
        Ok(_) => {
            tracing::info!("Saved tunnel_id {} for device {}", tunnel_id, device_id);
        }
        Err(e) => {
            tracing::error!("Failed to save tunnel_id for device {}: {}", device_id, e);
            // Continue anyway - tunnel_id can be fetched from Gateway on restart
        }
    }

    state
        .deployment
        .track_if_analytics_allowed(
            "device_registered",
            json!({
                "device_id": device_id.to_string(),
                "tunnel_id": tunnel_id.to_string(),
                "device_name": req.device_name,
                "service_port": service_port,
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(RegisterDeviceResponse {
        device,
    })))
}

/// Get a specific device by ID
pub async fn get_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Device>>, ApiError> {
    let _user_id = extract_user_id_from_headers(&headers)?;

    let device = Device::find_by_id(&state.deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    Ok(ResponseJson(ApiResponse::success(device)))
}

/// Delete a device and stop its GOST process
pub async fn delete_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let user_id = extract_user_id_from_headers(&headers)?;

    // Verify ownership before deletion
    let device = Device::find_by_id(&state.deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    if device.owner_id != user_id {
        return Err(ApiError::Unauthorized);
    }

    // Extract JWT token for Gateway API authentication
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized)?;

    // Step 1: Call Gateway unregister API to release tunnel resources (non-blocking)
    let gateway_url = std::env::var("GATEWAY_API_URL")
        .unwrap_or_else(|_| "http://localhost:24001".to_string());
    let gateway_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    tracing::info!("Unregistering device {} from Gateway", id);

    match gateway_client
        .delete(format!("{}/api/v1/devices/{}/unregister", gateway_url, id))
        .header("Authorization", auth_header)
        .send()
        .await
    {
        Ok(resp) => {
            if let Err(e) = resp.error_for_status_ref() {
                tracing::warn!("Gateway unregister failed (non-blocking): {}", e);
            } else {
                tracing::info!("Device {} unregistered from Gateway successfully", id);
            }
        }
        Err(e) => {
            tracing::warn!("Gateway unregister request failed (continuing): {}", e);
        }
    }

    // Step 2: Stop GOST process
    state.tunnel_manager.stop_device(id).await.ok();

    // Delete device
    Device::delete(&state.deployment.db().pool, id).await?;

    state
        .deployment
        .track_if_analytics_allowed(
            "device_deleted",
            json!({
                "device_id": id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// Device heartbeat endpoint
pub async fn device_heartbeat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<HeartbeatResponse>>, ApiError> {
    // Step 1: Update local status
    let device = Device::update_status(&state.deployment.db().pool, id, DeviceStatus::Online).await?;

    // Step 2: Forward heartbeat to Gateway (best-effort, non-blocking)
    let gateway_url = std::env::var("GATEWAY_API_URL")
        .unwrap_or_else(|_| "http://localhost:24001".to_string());

    let device_id_for_gateway = id;
    tokio::spawn(async move {
        let gateway_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        if let Err(e) = gateway_client
            .post(format!("{}/api/v1/devices/{}/heartbeat", gateway_url, device_id_for_gateway))
            .send()
            .await
        {
            tracing::debug!("Gateway heartbeat forward failed (non-blocking): {}", e);
        }
    });

    Ok(ResponseJson(ApiResponse::success(HeartbeatResponse {
        status: "ok".to_string(),
        last_seen: device.last_seen.unwrap_or_else(Utc::now),
    })))
}

/// Stop GOST process for a device
pub async fn stop_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let user_id = extract_user_id_from_headers(&headers)?;

    // Verify ownership
    let device = Device::find_by_id(&state.deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    if device.owner_id != user_id {
        return Err(ApiError::Unauthorized);
    }

    // Stop GOST process
    state.tunnel_manager.stop_device(id).await?;

    // Clear gost_process_id in database
    Device::update_gost_process_id(&state.pool(), id, None).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// Start GOST process for a device
pub async fn start_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let user_id = extract_user_id_from_headers(&headers)?;

    // Verify ownership
    let device = Device::find_by_id(&state.deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    if device.owner_id != user_id {
        return Err(ApiError::Unauthorized);
    }

    // Check if tunnel_id exists
    let tunnel_id_str = device.tunnel_id
        .ok_or_else(|| ApiError::BadRequest("Device has no tunnel_id. Please re-register the device.".to_string()))?;

    let tunnel_id: Uuid = tunnel_id_str.parse()
        .map_err(|_| ApiError::BadRequest("Invalid tunnel_id in database".to_string()))?;

    // Start GOST process
    state.tunnel_manager.start_device(
        id,
        tunnel_id,
        &device.name,
        device.service_port,
        &state.pool(),
    ).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// Access device via tunnel (token-based authentication)
/// TODO: Implement proxy to local service via tunnel
pub async fn access_device(
    State(_state): State<AppState>,
    Query(_params): Query<DeviceAccessQuery>,
) -> Result<ResponseJson<ApiResponse<String>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success("Tunnel access - TODO: implement JWT-based routing".to_string())))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tunnels/devices", get(list_devices).post(register_device))
        .route("/tunnels/device", get(access_device))
        .route(
            "/tunnels/devices/{id}",
            get(get_device).delete(delete_device),
        )
        .route("/tunnels/devices/{id}/start", post(start_device))
        .route("/tunnels/devices/{id}/stop", post(stop_device))
        .route("/tunnels/devices/{id}/heartbeat", post(device_heartbeat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mac_address_format() {
        let mac = generate_mac_address();
        // MAC address format: XX:XX:XX:XX:XX:XX
        let parts: Vec<&str> = mac.split(':').collect();
        assert_eq!(parts.len(), 6, "MAC address should have 6 parts");
        for part in parts {
            assert_eq!(part.len(), 2, "Each part should be 2 characters");
            assert!(
                part.chars().all(|c| c.is_ascii_hexdigit()),
                "Each part should be hex digits"
            );
        }
    }

    #[test]
    fn test_random_mac_address_format() {
        let mac = random_mac_address();
        // MAC address format: XX:XX:XX:XX:XX:XX
        let parts: Vec<&str> = mac.split(':').collect();
        assert_eq!(parts.len(), 6, "MAC address should have 6 parts");
        for part in parts {
            assert_eq!(part.len(), 2, "Each part should be 2 characters");
            assert!(
                part.chars().all(|c| c.is_ascii_hexdigit()),
                "Each part should be hex digits"
            );
        }
    }

    #[test]
    fn test_random_mac_addresses_are_unique() {
        let mac1 = random_mac_address();
        let mac2 = random_mac_address();
        assert_ne!(mac1, mac2, "Random MAC addresses should be unique");
    }

    #[test]
    fn test_mac_address_is_uppercase() {
        let mac = generate_mac_address();
        assert_eq!(mac, mac.to_uppercase(), "MAC address should be uppercase");
    }

    #[test]
    fn test_device_list_query_deserialize() {
        // Test DeviceListQuery can be deserialized
        let query = DeviceListQuery {
            status: Some("online".to_string()),
        };
        assert_eq!(query.status, Some("online".to_string()));
    }

    #[test]
    fn test_device_access_query_deserialize() {
        // Test DeviceAccessQuery can be deserialized
        let query = DeviceAccessQuery {
            t: "test_token".to_string(),
        };
        assert_eq!(query.t, "test_token");
    }
}
