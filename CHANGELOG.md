# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Tunnel Management System**: New device tunneling feature using GOST for remote access
  - Device registration and management endpoints
  - Tunnel status tracking (online/offline)
  - Local tunnel record management
  - Tunnel access logging for audit trails
  - Tunnel manager service for gost_client process lifecycle

- **JWT Authentication**: Proper JWT-based authentication for protected endpoints
  - JWT token validation using `jsonwebtoken` crate
  - Authentication middleware (`required_auth_middleware`, `optional_auth_middleware`)
  - `Authorization: Bearer <token>` header support
  - User ID extraction from JWT claims

- **Database Migrations**:
  - `20260113000000_add_tunnels.sql`: Devices and tunnel access logs tables
  - `20260113000001_add_local_tunnels.sql`: Local tunnel configurations table

- **TypeScript Types**: Added tunnel-related types to `shared/types.ts`
  - `Device`, `DeviceStatus`
  - `LocalTunnel`, `TunnelType`, `TunnelStatus`
  - `RemoteTunnelConfig`, `RemoteGostConfig`, `RemoteRegisterResponse`
  - `RegisterDeviceRequest`, `RegisterDeviceResponse`
  - `GostClientConfig`, `HeartbeatResponse`, `TunnelAccessLog`

- **Frontend Features**:
  - Tunnel settings page (`frontend/src/pages/settings/TunnelSettings.tsx`)
  - Device registration dialog
  - Device list with status indicators
  - Multi-language support for tunnel-related UI (en, es, ja, ko, zh-Hans, zh-Hant)

- **Tests**: Unit tests for tunnel routes and authentication

### Changed

- **API Route Structure**: OAuth routes moved from `/api/oauth/*` to `/auth/*` for better organization
  - Old: `/api/oauth/handoff/init`
  - New: `/auth/handoff/init`

- **Frontend API Client**: Updated `makeRequest` to automatically include `Authorization` header
  - All API requests now include `Bearer` token if available in localStorage
  - Token stored under `casdoor_access_token` key

### Removed

- **Remote Features**: Removed organization management, shared tasks, and remote server functionality
  - `crates/remote/` crate removed
  - `remote-frontend/` removed
  - Related database migrations and models removed

### Fixed

- **Dead Code**: Removed unused `forward_port` field from `TunnelManager`
- **Authentication Placeholder**: Replaced `Uuid::new_v4()` placeholder with proper JWT extraction
- **TypeScript Generation**: Added missing tunnel types to `generate_types.rs`

### Security

- **JWT Secret**: Added `JWT_SECRET` environment variable for token validation
  - Default: `"change-this-secret-in-production"` (MUST be changed in production)
- **Token Expiration**: JWT tokens now properly validated for expiration
- **Ownership Verification**: Device operations verify user ownership before allowing modifications

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `JWT_SECRET` | Secret key for JWT token validation | `change-this-secret-in-production` |
| `TUNNEL_API_URL` | Remote tunnel service API URL | `https://ziso-backend.yes-tek.com` |
| `TUNNEL_API_KEY` | API key for tunnel service authentication | (empty) |
| `GOST_CLIENT_PATH` | Path to gost_client binary | `./bin/gost_client` |
| `FORWARD_PORT` | Default forward port for tunnels | `23001` |

### Migration Notes

When upgrading to this version:

1. **Set `JWT_SECRET`**: Generate a secure random string and set it as the `JWT_SECRET` environment variable
2. **Update API calls**: If using OAuth endpoints directly, update URLs from `/api/oauth/*` to `/auth/*`
3. **Run migrations**: New database migrations will be applied automatically on startup
4. **Update frontend**: Run `pnpm run generate-types` after pulling changes

### Breaking Changes

- **OAuth endpoint URLs changed**: Update any hardcoded `/api/oauth/*` URLs to `/auth/*`
- **Authorization required**: Tunnel endpoints now require valid JWT token in `Authorization` header
- **Remote features removed**: If using organization/shared task features, these are no longer available
