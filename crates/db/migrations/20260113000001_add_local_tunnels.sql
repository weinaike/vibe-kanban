-- Local tunnel records table
-- Stores tunnel configurations received from remote service
CREATE TABLE IF NOT EXISTS local_tunnels (
    id BLOB PRIMARY KEY,
    device_id BLOB NOT NULL,
    tunnel_type TEXT NOT NULL CHECK (tunnel_type IN ('http', 'ws', 'tcp')),
    tunnel_id TEXT NOT NULL,
    access_url TEXT NOT NULL,
    local_port INTEGER NOT NULL,
    process_id INTEGER,
    status TEXT NOT NULL DEFAULT 'inactive' CHECK (status IN ('active', 'inactive')),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_local_tunnels_device_id ON local_tunnels(device_id);
CREATE INDEX IF NOT EXISTS idx_local_tunnels_status ON local_tunnels(status);
CREATE INDEX IF NOT EXISTS idx_local_tunnels_process_id ON local_tunnels(process_id);
