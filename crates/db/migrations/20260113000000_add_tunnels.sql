-- Devices table (devices that connect via tunnels)
CREATE TABLE IF NOT EXISTS devices (
    id BLOB PRIMARY KEY,
    tunnel_id BLOB NOT NULL UNIQUE,
    owner_id BLOB NOT NULL,
    mac_address TEXT NOT NULL,
    name TEXT NOT NULL,
    device_type TEXT,
    status TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('online', 'offline')),
    firmware_version TEXT,
    last_seen TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Access logs for tunnel access auditing
CREATE TABLE IF NOT EXISTS tunnel_access_logs (
    id BLOB PRIMARY KEY,
    device_id BLOB NOT NULL,
    tunnel_id BLOB NOT NULL,
    accessed_by TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_devices_owner_id ON devices(owner_id);
CREATE INDEX IF NOT EXISTS idx_devices_tunnel_id ON devices(tunnel_id);
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen);
CREATE INDEX IF NOT EXISTS idx_tunnel_access_logs_device_id ON tunnel_access_logs(device_id);
CREATE INDEX IF NOT EXISTS idx_tunnel_access_logs_created_at ON tunnel_access_logs(created_at);
