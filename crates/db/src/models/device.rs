use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, Sqlite, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Device {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub mac_address: String,
    pub name: String,
    pub device_type: Option<String>,
    pub status: DeviceStatus,
    pub firmware_version: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    /// Port where the vibe-kanban service runs
    pub service_port: i64,
    /// GOST process PID (single process for all tunnel types)
    pub gost_process_id: Option<i64>,
    /// Tunnel ID from Gateway API for GOST v3 restart functionality
    pub tunnel_id: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, TS)]
#[sqlx(type_name = "device_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DeviceStatus {
    Online,
    Offline,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct RegisterDeviceRequest {
    pub device_name: String,
    pub service_port: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RegisterDeviceResponse {
    pub device: Device,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct HeartbeatResponse {
    pub status: String,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, TS)]
#[ts(export)]
pub struct TunnelAccessLog {
    pub id: Uuid,
    pub device_id: Uuid,
    pub accessed_by: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
}

impl TunnelAccessLog {
    pub async fn create(
        pool: &SqlitePool,
        log: &CreateTunnelAccessLog,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            TunnelAccessLog,
            r#"INSERT INTO tunnel_access_logs (
                    id, device_id, accessed_by, ip_address, user_agent, success
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id as "id!: Uuid",
                          device_id as "device_id!: Uuid",
                          accessed_by,
                          ip_address,
                          user_agent,
                          success as "success: bool",
                          created_at as "created_at!: DateTime<Utc>""#,
            log.id,
            log.device_id,
            log.accessed_by,
            log.ip_address,
            log.user_agent,
            log.success,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_device_id(
        pool: &SqlitePool,
        device_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            TunnelAccessLog,
            r#"SELECT id as "id!: Uuid",
                      device_id as "device_id!: Uuid",
                      accessed_by,
                      ip_address,
                      user_agent,
                      success as "success: bool",
                      created_at as "created_at!: DateTime<Utc>"
               FROM tunnel_access_logs
               WHERE device_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
            device_id,
            limit
        )
        .fetch_all(pool)
        .await
    }
}

#[derive(Debug, Clone)]
pub struct CreateTunnelAccessLog {
    pub id: Uuid,
    pub device_id: Uuid,
    pub accessed_by: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
}

impl Device {
    pub async fn find_by_owner(
        pool: &SqlitePool,
        owner_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Device,
            r#"SELECT id as "id!: Uuid",
                      owner_id as "owner_id!: Uuid",
                      mac_address,
                      name,
                      device_type,
                      status as "status: DeviceStatus",
                      firmware_version,
                      last_seen as "last_seen?: DateTime<Utc>",
                      service_port as "service_port!: i64",
                      gost_process_id,
                      tunnel_id,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM devices
               WHERE owner_id = $1
               ORDER BY created_at DESC"#,
            owner_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Device,
            r#"SELECT id as "id!: Uuid",
                      owner_id as "owner_id!: Uuid",
                      mac_address,
                      name,
                      device_type,
                      status as "status: DeviceStatus",
                      firmware_version,
                      last_seen as "last_seen?: DateTime<Utc>",
                      service_port as "service_port!: i64",
                      gost_process_id,
                      tunnel_id,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM devices
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        executor: impl Executor<'_, Database = Sqlite>,
        device: &CreateDevice,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Device,
            r#"INSERT INTO devices (
                    id, owner_id, mac_address, name,
                    device_type, firmware_version, service_port
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING id as "id!: Uuid",
                          owner_id as "owner_id!: Uuid",
                          mac_address,
                          name,
                          device_type,
                          status as "status: DeviceStatus",
                          firmware_version,
                          last_seen as "last_seen?: DateTime<Utc>",
                          service_port,
                          gost_process_id,
                          tunnel_id,
                          created_at as "created_at!: DateTime<Utc>",
                          updated_at as "updated_at!: DateTime<Utc>""#,
            device.id,
            device.owner_id,
            device.mac_address,
            device.name,
            device.device_type,
            device.firmware_version,
            device.service_port,
        )
        .fetch_one(executor)
        .await
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: Uuid,
        status: DeviceStatus,
    ) -> Result<Self, sqlx::Error> {
        let status_str = match status {
            DeviceStatus::Online => "online",
            DeviceStatus::Offline => "offline",
        };

        sqlx::query_as!(
            Device,
            r#"UPDATE devices
               SET status = $2, last_seen = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id as "id!: Uuid",
                         owner_id as "owner_id!: Uuid",
                         mac_address,
                         name,
                         device_type,
                         status as "status: DeviceStatus",
                         firmware_version,
                         last_seen as "last_seen?: DateTime<Utc>",
                         service_port,
                         gost_process_id,
                         tunnel_id,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            status_str,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_gost_process_id(
        pool: &SqlitePool,
        id: Uuid,
        gost_process_id: Option<i64>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Device,
            r#"UPDATE devices
               SET gost_process_id = $2, updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id as "id!: Uuid",
                         owner_id as "owner_id!: Uuid",
                         mac_address,
                         name,
                         device_type,
                         status as "status: DeviceStatus",
                         firmware_version,
                         last_seen as "last_seen?: DateTime<Utc>",
                         service_port,
                         gost_process_id,
                         tunnel_id,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            gost_process_id,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_tunnel_id(
        pool: &SqlitePool,
        id: Uuid,
        tunnel_id: String,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Device,
            r#"UPDATE devices
               SET tunnel_id = $2, updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id as "id!: Uuid",
                         owner_id as "owner_id!: Uuid",
                         mac_address,
                         name,
                         device_type,
                         status as "status: DeviceStatus",
                         firmware_version,
                         last_seen as "last_seen?: DateTime<Utc>",
                         service_port,
                         gost_process_id,
                         tunnel_id,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            tunnel_id,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result: sqlx::sqlite::SqliteQueryResult = sqlx::query!("DELETE FROM devices WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark devices as offline if they haven't been seen recently
    pub async fn mark_offline_devices(
        pool: &SqlitePool,
        timeout_seconds: i64,
    ) -> Result<u64, sqlx::Error> {
        let timeout_threshold = Utc::now() - chrono::Duration::seconds(timeout_seconds);

        let result: sqlx::sqlite::SqliteQueryResult = sqlx::query!(
            r#"
            UPDATE devices
            SET status = 'offline'
            WHERE status = 'online'
            AND (last_seen IS NULL OR last_seen < $1)
            "#,
            timeout_threshold
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone)]
pub struct CreateDevice {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub mac_address: String,
    pub name: String,
    pub device_type: Option<String>,
    pub firmware_version: Option<String>,
    pub service_port: i64,
}
