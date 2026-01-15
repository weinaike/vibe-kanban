-- Add tunnel_id back to devices table for GOST v3 restart functionality
-- This allows restarting stopped devices without re-registering

-- Add tunnel_id column to devices table
ALTER TABLE devices ADD COLUMN tunnel_id TEXT;

CREATE INDEX IF NOT EXISTS idx_devices_tunnel_id ON devices(tunnel_id);
