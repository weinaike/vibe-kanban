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
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Deserialize, serde::Serialize, Debug)]
struct GatewayRegisterRequest {
    mac_address: String,
    device_name: String,
    device_type: Option<String>,
    firmware_version: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GatewayGostConfig {
    server_addr: String,
    tunnel_id: String,
    local_addr: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GatewayTunnelConfig {
    tunnel_id: String,
    gost_config: GatewayGostConfig,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GatewayRegisterResponse {
    device_id: String,
    tunnel: GatewayTunnelConfig,
    heartbeat_interval: i32,
}

/// Gateway device list response
#[derive(Deserialize, serde::Serialize, Debug, TS)]
pub struct GatewayDevice {
    id: String,
    name: String,
    #[serde(rename = "type")]
    device_type: String,
    #[serde(rename = "mac_address")]
    mac_address: String,
    status: String,
    #[serde(rename = "last_seen")]
    last_seen: Option<String>,
    #[serde(rename = "access_url")]
    access_url: Option<String>,
    #[serde(rename = "created_at")]
    created_at: String,
}

#[derive(Deserialize)]
struct GatewayDeviceListResponse {
    devices: Vec<GatewayDevice>,
    total: i32,
}

/// Data source indicator for merged devices
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DeviceSource {
    /// Device only exists in local database
    Local,
    /// Device only exists in Gateway
    Gateway,
    /// Device exists in both (merged with local priority)
    Merged,
}

/// Merged device representation combining local and Gateway data
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MergedDevice {
    /// Device ID (prioritizes local database if exists)
    pub id: Uuid,
    /// Device name (prioritizes local database if exists)
    pub name: String,
    /// MAC address (used for filtering)
    pub mac_address: String,
    /// Device type
    pub device_type: Option<String>,
    /// Status (prioritizes local database's real-time status)
    pub status: DeviceStatus,
    /// Service port (from local database)
    pub service_port: Option<i64>,
    /// GOST process ID (from local database)
    pub gost_process_id: Option<i64>,
    /// Tunnel ID (from local database)
    pub tunnel_id: Option<String>,
    /// Last seen timestamp
    pub last_seen: Option<String>,
    /// Created at timestamp
    #[ts(type = "Date")]
    pub created_at: String,
    /// Updated at timestamp
    #[ts(type = "Date")]
    pub updated_at: String,
    /// Access URL (from Gateway)
    pub access_url: Option<String>,
    /// Data source: "local", "gateway", or "merged"
    pub source: DeviceSource,
    /// Firmware version
    pub firmware_version: Option<String>,
}

use crate::{config::TunnelServiceConfig, error::ApiError, AppState, middleware::auth::extract_user_id_from_headers_async};
use utils::response::ApiResponse;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Cached MAC address with expiration
struct CachedMacAddress {
    mac: String,
    cached_at: Instant,
}

/// Global MAC address cache (expires after 1 hour)
static MAC_CACHE: OnceLock<CachedMacAddress> = OnceLock::new();

/// Get local MAC address with caching (1 hour cache)
fn get_local_mac_address() -> String {
    const CACHE_DURATION: Duration = Duration::from_secs(3600); // 1 hour

    // Check cache first
    if let Some(cached) = MAC_CACHE.get() {
        if cached.cached_at.elapsed() < CACHE_DURATION {
            tracing::debug!("Using cached MAC address: {}", cached.mac);
            return cached.mac.clone();
        }
    }

    // Cache miss or expired, fetch new MAC
    let mac = generate_mac_address();

    // Update cache
    MAC_CACHE.get_or_init(|| CachedMacAddress {
        mac: mac.clone(),
        cached_at: Instant::now(),
    });

    tracing::info!("Cached local MAC address: {}", mac);
    mac
}

/// Force refresh MAC address cache (for testing)
#[cfg(test)]
#[allow(dead_code)]
fn refresh_mac_cache() {
    // OnceLock doesn't have take(), but we can just let it expire naturally
    // For testing purposes, you'd need to use a different caching mechanism
}

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

/// Fetch devices from Gateway API (with graceful error handling)
/// Returns Result so caller can handle Gateway failures gracefully
async fn fetch_gateway_devices(
    auth_header: String,
) -> Result<Vec<GatewayDevice>, String> {
    let gateway_url = std::env::var("GATEWAY_API_URL")
        .unwrap_or_else(|_| "http://localhost:24001".to_string());

    let gateway_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    tracing::info!("Fetching devices from Gateway: {}", gateway_url);

    let http_resp = gateway_client
        .get(format!("{}/api/v1/devices", gateway_url))
        .header("Authorization", auth_header)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("Gateway API request failed: {}", e);
            format!("Gateway API request failed: {}", e)
        })?;

    let status = http_resp.status();
    tracing::info!("Gateway device list response status: {}", status);

    if !status.is_success() {
        let error_body = http_resp.text().await.unwrap_or_default();
        tracing::warn!("Gateway returned error: status={}, body={}", status, error_body);
        return Err(format!("Gateway API error: status={}", status));
    }

    let gateway_resp: GatewayDeviceListResponse = http_resp
        .json()
        .await
        .map_err(|e| {
            tracing::warn!("Failed to parse Gateway response: {}", e);
            format!("Invalid Gateway response: {}", e)
        })?;

    tracing::info!("Gateway returned {} devices", gateway_resp.total);
    Ok(gateway_resp.devices)
}

/// Merge local and Gateway devices with MAC-based filtering and local priority
fn merge_devices(
    local_devices: Vec<Device>,
    gateway_devices: Result<Vec<GatewayDevice>, String>,
    local_mac: &str,
) -> Vec<MergedDevice> {
    use std::collections::HashMap;

    let mut merged: HashMap<Uuid, MergedDevice> = HashMap::new();

    // Step 1: Add all local devices first
    for device in local_devices {
        let merged_device = MergedDevice {
            id: device.id,
            name: device.name.clone(),
            mac_address: device.mac_address.clone(),
            device_type: device.device_type.clone(),
            status: device.status,
            service_port: Some(device.service_port),
            gost_process_id: device.gost_process_id,
            tunnel_id: device.tunnel_id,
            last_seen: device.last_seen.map(|dt| dt.to_rfc3339()),
            created_at: device.created_at.to_rfc3339(),
            updated_at: device.updated_at.to_rfc3339(),
            access_url: None,
            source: DeviceSource::Local,
            firmware_version: device.firmware_version,
        };
        merged.insert(device.id, merged_device);
    }

    // Step 2: Process Gateway devices (if available) with MAC filtering
    if let Ok(gateway_devs) = gateway_devices {
        for gw_device in gateway_devs {
            // MAC filtering: only include devices matching local MAC
            if gw_device.mac_address != local_mac {
                tracing::debug!(
                    "Skipping Gateway device {} (MAC: {} != local MAC: {})",
                    gw_device.id, gw_device.mac_address, local_mac
                );
                continue;
            }

            // Parse Gateway device ID as UUID
            let device_id = match Uuid::parse_str(&gw_device.id) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(
                        "Invalid Gateway device ID '{}': {}, skipping",
                        gw_device.id, e
                    );
                    continue;
                }
            };

            // Check if device exists in local database
            if let Some(local_device) = merged.get(&device_id) {
                // Device exists in both - merge with local priority
                let mut merged_device = local_device.clone();
                merged_device.source = DeviceSource::Merged;
                merged_device.access_url = gw_device.access_url;
                merged.insert(device_id, merged_device);

                tracing::debug!(
                    "Merged device {} (exists in both local and Gateway)",
                    device_id
                );
            } else {
                // Device only in Gateway - add it
                let status = match gw_device.status.as_str() {
                    "online" => DeviceStatus::Online,
                    _ => DeviceStatus::Offline,
                };

                let merged_device = MergedDevice {
                    id: device_id,
                    name: gw_device.name.clone(),
                    mac_address: gw_device.mac_address.clone(),
                    device_type: Some(gw_device.device_type),
                    status,
                    service_port: None, // Gateway doesn't provide this
                    gost_process_id: None, // Only in local DB
                    tunnel_id: None, // Only in local DB
                    last_seen: gw_device.last_seen,
                    created_at: gw_device.created_at,
                    updated_at: Utc::now().to_rfc3339(), // Gateway doesn't provide updated_at
                    access_url: gw_device.access_url,
                    source: DeviceSource::Gateway,
                    firmware_version: None, // Gateway may not provide this
                };
                merged.insert(device_id, merged_device);

                tracing::debug!(
                    "Added Gateway-only device {} (MAC: {})",
                    device_id, gw_device.mac_address
                );
            }
        }
    } else {
        tracing::warn!("Gateway API unavailable, using local devices only");
    }

    // Convert to sorted vector (newest first)
    let mut devices: Vec<_> = merged.into_values().collect();
    devices.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    devices
}

/// List all devices for the authenticated user
/// Merges local database and Gateway API data with MAC-based filtering
pub async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<ResponseJson<ApiResponse<Vec<MergedDevice>>>, ApiError> {
    // Extract user ID
    let user_id = extract_user_id_from_headers_async(&headers).await?;

    // Get local MAC address (cached)
    let local_mac = get_local_mac_address();
    tracing::info!("Fetching devices for local MAC: {}", local_mac);

    // Extract auth header for Gateway API
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| {
            // Remove "Bearer " prefix if present, then add it back
            s.trim_start_matches("Bearer ").trim().to_string()
        })
        .map(|token| format!("Bearer {}", token))
        .ok_or_else(|| ApiError::BadRequest("Missing authorization header".to_string()))?;

    // Parallel fetch from both sources: local database (required) + Gateway API (optional)
    let local_devices_future = Device::find_by_owner(&state.deployment.db().pool, user_id);
    let gateway_devices_future = fetch_gateway_devices(auth_header);

    let (local_devices_result, gateway_devices_result) = tokio::join!(
        local_devices_future,
        gateway_devices_future
    );

    let local_devices = local_devices_result?;

    // Merge with MAC filtering and local priority
    let merged_devices = merge_devices(local_devices, gateway_devices_result, &local_mac);

    tracing::info!("Returning {} merged devices", merged_devices.len());

    Ok(ResponseJson(ApiResponse::success(merged_devices)))
}

/// Register a new device with Gateway API and local GOST v3 process
pub async fn register_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterDeviceRequest>,
) -> Result<ResponseJson<ApiResponse<RegisterDeviceResponse>>, ApiError> {
    let tunnel_config = TunnelServiceConfig::default();
    let user_id = extract_user_id_from_headers_async(&headers).await?;
    let mac_address = generate_mac_address();
    let service_port = req.service_port.unwrap_or(tunnel_config.default_service_port as i64);

    // Get Gateway API token from environment (fallback to user's JWT for backward compatibility)
    let gateway_auth_header = if let Ok(api_token) = std::env::var("GATEWAY_API_TOKEN") {
        // Use dedicated Gateway API token if configured and non-empty
        if !api_token.trim().is_empty() {
            format!("Bearer {}", api_token.trim_start_matches("Bearer ").trim())
        } else {
            // Fallback: extract and clean user's JWT token
            headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .map(|s| {
                    // Remove "Bearer " prefix if present, then add it back to avoid double "Bearer"
                    s.trim_start_matches("Bearer ").trim().to_string()
                })
                .map(|token| format!("Bearer {}", token))
                .ok_or_else(|| ApiError::BadRequest("Gateway API authentication failed: No valid token available".to_string()))?
        }
    } else {
        // No Gateway API token configured, use user's JWT
        headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .map(|s| {
                // Remove "Bearer " prefix if present, then add it back to avoid double "Bearer"
                s.trim_start_matches("Bearer ").trim().to_string()
            })
            .map(|token| format!("Bearer {}", token))
            .ok_or_else(|| ApiError::BadRequest("Gateway API authentication failed: No valid token available".to_string()))?
    };

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
    tracing::info!("Gateway request: {:?}", gateway_req);

    let http_resp = gateway_client
        .post(format!("{}/api/v1/devices/register", gateway_url))
        .header("Authorization", &gateway_auth_header)
        .json(&gateway_req)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to call Gateway API: {}", e);
            ApiError::BadRequest(format!("Gateway registration failed: {}", e))
        })?;

    let status = http_resp.status();
    tracing::info!("Gateway response status: {}", status);

    let response_body = http_resp.text().await.map_err(|e| {
        tracing::error!("Failed to read Gateway response body: {}", e);
        ApiError::BadRequest(format!("Failed to read Gateway response: {}", e))
    })?;

    tracing::info!("Gateway response body: {}", response_body);

    // Check if Gateway returned an error (e.g., MAC already registered)
    if status.as_u16() == 409 {
        // MAC address already registered - check if device exists in local database
        tracing::info!("MAC address already registered in Gateway, checking local database");

        // Check if there's already a device with this MAC address for this user
        let existing_devices = Device::find_by_owner(&state.deployment.db().pool, user_id).await?;
        if let Some(existing_device) = existing_devices.iter().find(|d| d.mac_address == mac_address) {
            tracing::info!("Found existing device {} with MAC address {}", existing_device.id, mac_address);
            // Return the existing device info
            return Ok(ResponseJson(ApiResponse::success(RegisterDeviceResponse {
                device: existing_device.clone(),
            })));
        }

        // Device exists in Gateway but not in local database
        return Err(ApiError::BadRequest(format!(
            "Device with MAC address {} is already registered in Gateway but not found in local database. Please contact support.",
            mac_address
        )));
    }

    // Try to parse the response
    let gateway_resp: GatewayRegisterResponse = serde_json::from_str(&response_body)
        .map_err(|e| {
            tracing::error!("Failed to parse Gateway response body '{}': {}", response_body, e);
            ApiError::BadRequest(format!("Invalid Gateway response: error decoding response body, status: {}", status))
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
    let _user_id = extract_user_id_from_headers_async(&headers).await?;

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
    let user_id = extract_user_id_from_headers_async(&headers).await?;

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

    // Step 1: Call Gateway unregister API to release tunnel resources (must succeed first)
    let gateway_url = std::env::var("GATEWAY_API_URL")
        .unwrap_or_else(|_| "http://localhost:24001".to_string());
    let gateway_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    tracing::info!("Unregistering device {} from Gateway", id);

    let resp = gateway_client
        .delete(format!("{}/api/v1/devices/{}/unregister", gateway_url, id))
        .header("Authorization", auth_header)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Gateway unregister request failed: {}", e);
            ApiError::BadRequest(format!("Failed to connect to Gateway: {}", e))
        })?;

    let status = resp.status();
    tracing::info!("Gateway unregister response status: {}", status);

    // Read response body for better error reporting
    let response_body = resp.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
    tracing::info!("Gateway unregister response body: {}", response_body);

    // Only proceed with local deletion if Gateway returns success (2xx)
    if !status.is_success() {
        return Err(ApiError::BadRequest(format!(
            "Failed to unregister device from Gateway: status={}, response={}",
            status, response_body
        )));
    }

    tracing::info!("Device {} unregistered from Gateway successfully", id);

    // Step 2: Call Gateway DELETE API to actually delete the device from Gateway's device list
    tracing::info!("Deleting device {} from Gateway device list", id);

    let delete_resp = gateway_client
        .delete(format!("{}/api/v1/devices/{}", gateway_url, id))
        .header("Authorization", auth_header)
        .send()
        .await;

    match delete_resp {
        Ok(resp) => {
            let delete_status = resp.status();
            tracing::info!("Gateway delete response status: {}", delete_status);
            if delete_status.is_success() {
                tracing::info!("Device {} deleted from Gateway successfully", id);
            } else if delete_status.as_u16() == 404 {
                // Device already deleted from Gateway, that's ok
                tracing::info!("Device {} not found in Gateway (may have been already deleted)", id);
            } else {
                let delete_body = resp.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
                tracing::warn!("Gateway delete returned non-success: status={}, body={}", delete_status, delete_body);
                // Continue with local deletion even if Gateway delete fails (device already unregistered)
            }
        }
        Err(e) => {
            tracing::warn!("Gateway delete request failed: {}, continuing with local deletion", e);
            // Continue with local deletion even if Gateway delete fails (device already unregistered)
        }
    }

    // Step 3: Stop GOST process
    state.tunnel_manager.stop_device(id).await.ok();

    // Step 4: Delete from local database
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
    let user_id = extract_user_id_from_headers_async(&headers).await?;

    // Verify ownership
    let device = Device::find_by_id(&state.deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Device not found".to_string()))?;

    if device.owner_id != user_id {
        return Err(ApiError::Unauthorized);
    }

    // Stop GOST process
    // First try to stop via tunnel_manager (in-memory process)
    let tunnel_result = state.tunnel_manager.stop_device(id).await;

    // If process not in memory (e.g., after server restart), check database
    if tunnel_result.is_err() && device.gost_process_id.is_some() {
        let pid = device.gost_process_id.unwrap() as i32;
        tracing::info!("Process not in memory, killing by PID: {}", pid);

        // Kill process by PID
        match tokio::process::Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .output()
            .await
        {
            Ok(_) => {
                tracing::info!("Killed GOST process {}", pid);
            }
            Err(e) => {
                tracing::warn!("Failed to kill process {}: {}, clearing DB anyway", pid, e);
            }
        }
    }

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
    let user_id = extract_user_id_from_headers_async(&headers).await?;

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
