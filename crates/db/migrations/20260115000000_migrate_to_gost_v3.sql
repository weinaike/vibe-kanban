-- Migration for GOST v3 direct routing mode
-- Removes per-tunnel tracking, simplifies to single GOST process per device

-- Drop the local_tunnels table (no longer needed with single GOST process)
DROP TABLE IF EXISTS local_tunnels;

-- Recreate devices table without tunnel_id, adding service_port and gost_process_id
-- SQLite doesn't support dropping columns with UNIQUE constraint directly,
-- so we need to recreate the table
CREATE TABLE IF NOT EXISTS devices_new (
    id BLOB PRIMARY KEY,
    owner_id BLOB NOT NULL,
    mac_address TEXT NOT NULL,
    name TEXT NOT NULL,
    device_type TEXT,
    status TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('online', 'offline')),
    firmware_version TEXT,
    last_seen TEXT,
    service_port INTEGER NOT NULL DEFAULT 23001,
    gost_process_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Copy data from old table to new table (excluding tunnel_id)
INSERT INTO devices_new (id, owner_id, mac_address, name, device_type, status, firmware_version, last_seen, created_at, updated_at)
SELECT id, owner_id, mac_address, name, device_type, status, firmware_version, last_seen, created_at, updated_at
FROM devices;

-- Drop old table and rename new table
DROP TABLE devices;
ALTER TABLE devices_new RENAME TO devices;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_devices_owner_id ON devices(owner_id);
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen);

-- Remove tunnel_id from tunnel_access_logs (simplify logging)
CREATE TABLE IF NOT EXISTS tunnel_access_logs_new (
    id BLOB PRIMARY KEY,
    device_id BLOB NOT NULL,
    accessed_by TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Copy data from old table to new table (excluding tunnel_id)
INSERT INTO tunnel_access_logs_new (id, device_id, accessed_by, ip_address, user_agent, success, created_at)
SELECT id, device_id, accessed_by, ip_address, user_agent, success, created_at
FROM tunnel_access_logs;

-- Drop old table and rename new table
DROP TABLE tunnel_access_logs;
ALTER TABLE tunnel_access_logs_new RENAME TO tunnel_access_logs;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_tunnel_access_logs_device_id ON tunnel_access_logs(device_id);
CREATE INDEX IF NOT EXISTS idx_tunnel_access_logs_created_at ON tunnel_access_logs(created_at);
