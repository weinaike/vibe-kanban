/// Tunnel manager service for managing GOST v3 processes
/// Each device has a single GOST process that handles all tunnel types
use db::models::device::Device;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::process::Child;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, warn};
use uuid::Uuid;

/// Tunnel manager handles GOST v3 process lifecycle
/// Key changes from previous version:
/// - Single GOST process per device (not per tunnel type)
/// - Uses GOST v3 tunnel protocol with direct routing
/// - Command format: gost -L rtcp://:0/127.0.0.1:PORT -F "tunnel://SERVER?tunnel.id=DEVICE-UUID"
pub struct TunnelManager {
    gost_binary_path: String,
    gost_server_addr: String,
    active_processes: Arc<RwLock<HashMap<Uuid, Child>>>,
}

impl TunnelManager {
    /// Create a new tunnel manager
    pub fn new(gost_binary_path: String, gost_server_addr: String) -> Self {
        Self {
            gost_binary_path,
            gost_server_addr,
            active_processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a GOST v3 process for a device
    /// This replaces the previous per-tunnel-type process spawning
    /// device_id: used for local tracking and database storage
    /// tunnel_id: used for GOST tunnel.id parameter
    pub async fn start_device(
        &self,
        device_id: Uuid,
        tunnel_id: Uuid,
        device_name: &str,
        service_port: i64,
        pool: &SqlitePool,
    ) -> Result<(), String> {
        info!(
            "Starting GOST v3 for device '{}' (device_id: {}, tunnel_id: {}, service_port: {})",
            device_name, device_id, tunnel_id, service_port
        );

        // Build forward address - where GOST forwards traffic to (local service)
        let forward_addr = format!("127.0.0.1:{}", service_port);

        // Build GOST v3 command with tunnel protocol
        // Format: gost -L rtcp://:0/forward_addr -F "tunnel://server_addr?tunnel.id=tunnel_id&tunnel.direct=true"
        let chain = format!(
            "tunnel://{}?tunnel.id={}&tunnel.direct=true",
            self.gost_server_addr, tunnel_id
        );

        let args = vec![
            "-L".to_string(),
            format!("rtcp://:0/{}", forward_addr),
            "-F".to_string(),
            chain,
        ];

        info!(
            "GOST v3 command: {} {}",
            self.gost_binary_path,
            args.join(" ")
        );

        // Spawn the GOST process
        let child = std::process::Command::new(&self.gost_binary_path)
            .args(&args)
            .spawn()
            .map_err(|e| {
                error!("Failed to start GOST v3: {}", e);
                format!("Failed to start GOST v3: {}", e)
            })?;

        let pid = child.id() as i64;
        info!("GOST v3 started with PID {} for device {}", pid, device_id);

        // Store process reference using device_id for local tracking
        let mut processes = self.active_processes.write().await;
        processes.insert(device_id, child);

        // Update gost_process_id in database using device_id
        if let Err(e) = Device::update_gost_process_id(pool, device_id, Some(pid)).await {
            error!("Failed to update gost_process_id for device {}: {}", device_id, e);
        }

        Ok(())
    }

    /// Stop the GOST process for a specific device
    pub async fn stop_device(&self, device_id: Uuid) -> Result<(), String> {
        let mut processes = self.active_processes.write().await;

        if let Some(mut child) = processes.remove(&device_id) {
            info!("Stopping GOST v3 for device {}", device_id);

            child.kill()
                .map_err(|e| {
                    error!("Failed to kill process for device {}: {}", device_id, e);
                    format!("Failed to kill process: {}", e)
                })?;

            info!("GOST v3 stopped for device {}", device_id);
            Ok(())
        } else {
            warn!("No active process found for device {}", device_id);
            Err(format!("No active process found for device {}", device_id))
        }
    }

    /// Check if a device's GOST process is still alive
    pub async fn is_device_alive(&self, device_id: Uuid) -> bool {
        let mut processes = self.active_processes.write().await;
        if let Some(child) = processes.get_mut(&device_id) {
            // try_wait() checks if process has exited without blocking
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    warn!(
                        "Device {} GOST process exited with status: {}",
                        device_id, status
                    );
                    // Remove dead process
                    processes.remove(&device_id);
                    false
                }
                Ok(None) => true, // Process is still running
                Err(e) => {
                    error!(
                        "Failed to check process status for device {}: {}",
                        device_id, e
                    );
                    false
                }
            }
        } else {
            false
        }
    }

    /// Check and clean up dead processes, updating database status
    /// Returns list of device IDs that were found dead
    pub async fn reap_dead_processes(&self, pool: &SqlitePool) -> Vec<Uuid> {
        let mut dead_devices = Vec::new();
        let mut processes = self.active_processes.write().await;

        let mut to_remove = Vec::new();
        for (device_id, child) in processes.iter_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has died
                    warn!("Device {} GOST process has died, removing from tracking", device_id);
                    to_remove.push(*device_id);
                }
                Ok(None) => {
                    // Process is still running
                }
                Err(_) => {
                    // Check failed, also mark as dead
                    warn!(
                        "Failed to check device {} GOST process status, marking as dead",
                        device_id
                    );
                    to_remove.push(*device_id);
                }
            }
        }

        // Remove dead processes and update database
        for device_id in to_remove {
            processes.remove(&device_id);
            // Clear the gost_process_id in database to indicate process is dead
            if let Err(e) = Device::update_gost_process_id(pool, device_id, None).await {
                error!("Failed to clear gost_process_id for device {}: {}", device_id, e);
            }
            dead_devices.push(device_id);
        }

        if !dead_devices.is_empty() {
            info!("Reaped {} dead GOST processes", dead_devices.len());
        }

        dead_devices
    }

    /// Shutdown all GOST processes (for graceful server shutdown)
    pub async fn shutdown(&self) {
        info!("Shutting down all GOST processes");

        let mut processes = self.active_processes.write().await;
        let count = processes.len();

        for (id, mut child) in processes.drain() {
            if let Err(e) = child.kill() {
                warn!("Failed to kill process for device {}: {}", id, e);
            }
        }

        info!("Shutdown complete: {} processes stopped", count);
    }

    /// Get count of active GOST processes
    pub async fn active_count(&self) -> usize {
        self.active_processes.read().await.len()
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        // Note: We can't use async in Drop, so this is a best-effort check
        // The main shutdown should be called explicitly via shutdown()
        if let Ok(processes) = self.active_processes.try_read() {
            if !processes.is_empty() {
                warn!("TunnelManager dropped without explicit shutdown");
            }
        }
    }
}
