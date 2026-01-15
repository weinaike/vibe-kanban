# 隧道服务业务流程测试指南

## 服务信息

- **域名**: `https://ziso-backend.yes-tek.com`
- **API Base**: `https://ziso-backend.yes-tek.com/api`

---

## 步骤 1: 获取登录 URL

在浏览器中访问：

```bash
curl -s "https://ziso-backend.yes-tek.com/api/auth/login?redirect_uri=http://localhost/callback" | jq -r '.login_url'
```

示例返回：
```
https://auth.yes-tek.com/login/oauth/authorize?client_id=xxx&response_type=code&redirect_uri=https%3A%2F%2Fziso-backend.yes-tek.com%2Fcallback&scope=openid+profile+email&state=xxx
```

在浏览器中打开这个 URL，登录后会跳转到：
```
https://ziso-backend.yes-tek.com/callback?code=xxx&state=xxx
```

**复制浏览器地址栏中的完整 callback URL**

---

## 步骤 2: 获取 Access Token

从 callback URL 中提取 `code` 参数，然后调用：

```bash
curl -s "https://ziso-backend.yes-tek.com/api/auth/callback?code=YOUR_CODE&state=test" | jq .
```

响应示例：
```json
{
  "access_token": "eyJhbGc...",
  "token_type": "Bearer",
  "expires_in": 7200,
  "user": {
    "id": "9c5b3c0d-9420-4435-89ef-81cedb19b6ef",
    "name": "test01",
    "email": ""
  }
}
```

**保存 `access_token`，后续请求需要使用**

---

## 步骤 3: 列出已有设备

```bash
curl -s "https://ziso-backend.yes-tek.com/api/devices" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" | jq .
```

---

## 步骤 4: 注册新设备

```bash
curl -s -X POST "https://ziso-backend.yes-tek.com/api/devices/register" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "mac_address": "00:11:22:33:44:55",
    "device_name": "test-device",
    "device_type": "test",
    "firmware_version": "1.0.0",
    "tunnel_types": ["http", "ws"]
  }' | jq .
```

响应示例：
```json
{
  "device_id": "123e4567-e89b-12d3-a456-426614174000",
  "tunnels": [
    {
      "tunnel_type": "http",
      "tunnel_id": "987fcdeb-51a2-43f1-a456-426614174000",
      "access_url": "https://ziso-backend.yes-tek.com/device?t=xxx",
      "local_port": 80,
      "gost_config": {
        "server_addr": "localhost:9000",
        "tunnel_id": "987fcdeb-51a2-43f1-a456-426614174000",
        "local_addr": "127.0.0.1:80",
        "forwarder": "tcp"
      }
    },
    {
      "tunnel_type": "ws",
      "tunnel_id": "abc123-def4-5678-90ab-cdef12345678",
      "access_url": "https://ziso-backend.yes-tek.com/device?t=yyy",
      "local_port": 81,
      "gost_config": {
        "server_addr": "localhost:9000",
        "tunnel_id": "abc123-def4-5678-90ab-cdef12345678",
        "local_addr": "127.0.0.1:81",
        "forwarder": "tcp"
      }
    }
  ],
  "heartbeat_interval": 30
}
```

**保存 `device_id`，后续操作需要使用**

---

## 步骤 5: 发送心跳

```bash
curl -s -X POST "https://ziso-backend.yes-tek.com/api/devices/DEVICE_ID/heartbeat" | jq .
```

持续心跳（每 30 秒）：
```bash
while true; do
  echo "$(date '+%H:%M:%S') - 发送心跳..."
  curl -s -X POST "https://ziso-backend.yes-tek.com/api/devices/DEVICE_ID/heartbeat" | jq .
  sleep 30
done
```

---

## 步骤 6: 查询设备状态

```bash
curl -s "https://ziso-backend.yes-tek.com/api/devices/DEVICE_ID" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" | jq .
```

---

## 步骤 7: 启动 GOST 客户端

根据注册响应中的 `gost_config` 启动客户端：

```bash
# HTTP 隧道
./bin/gost_client \
  -server localhost:9000 \
  -tunnel-id 987fcdeb-51a2-43f1-a456-426614174000 \
  -local 127.0.0.1:80 \
  -forward tcp

# WebSocket 隧道
./bin/gost_client \
  -server localhost:9000 \
  -tunnel-id abc123-def4-5678-90ab-cdef12345678 \
  -local 127.0.0.1:81 \
  -forward tcp
```

---

## 步骤 8: 测试访问设备

在浏览器中访问 `access_url`：

```
https://ziso-backend.yes-tek.com/device?t=YOUR_TOKEN
```

这会将请求转发到你本地服务的 `127.0.0.1:80` 或 `127.0.0.1:81`

---

## 步骤 9: 取消注册设备

```bash
curl -s -X DELETE "https://ziso-backend.yes-tek.com/api/devices/DEVICE_ID/unregister" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" | jq .
```

---

## 快速测试脚本

```bash
# 设置变量
export GATEWAY="https://ziso-backend.yes-tek.com"
export API_BASE="$GATEWAY/api/v1"

# 1. 获取登录 URL
echo "登录 URL:"
curl -s "$API_BASE/auth/login?redirect_uri=http://localhost/callback" | jq -r '.login_url'
echo ""

# 2. 使用 code 获取 token (替换 YOUR_CODE)
# curl -s "$API_BASE/auth/callback?code=YOUR_CODE&state=test" | jq .

# 3. 注册设备 (替换 YOUR_TOKEN)
# curl -s -X POST "$API_BASE/devices/register" \
#   -H "Authorization: Bearer YOUR_TOKEN" \
#   -H "Content-Type: application/json" \
#   -d '{"mac_address":"00:11:22:33:44:55","device_name":"test","tunnel_types":["http"]}' | jq .
```

---

## 业务流程图

```
┌─────────┐          ┌──────────┐          ┌─────────────┐
│  用户   │──────────>│ Casdoor  │──────────>│  Gateway    │
│         │ OAuth登录│          │  Token   │             │
└─────────┘          └──────────┘          └──────┬──────┘
                                                   │
         ┌─────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────────────────────────┐
│                     设备注册流程                             │
├────────────────────────────────────────────────────────────┤
│  1. 用户通过 JWT 认证                                       │
│  2. 提交设备信息 (MAC, 名称, 类型等)                        │
│  3. Gateway 创建设备记录                                    │
│  4. Gateway 为每个隧道类型生成:                             │
│     - 隧道 ID (UUID)                                        │
│     - URL Token (加密的访问凭证)                            │
│     - Access URL (前端访问地址)                             │
│     - GOST Config (客户端配置)                              │
│  5. 返回设备 ID 和隧道配置                                  │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────────────────────────┐
│                     隧道建立流程                             │
├────────────────────────────────────────────────────────────┤
│  1. 客户端根据 GOST Config 启动 gost_client                │
│  2. gost_client 连接到 GOST Server (localhost:9000)        │
│  3. 建立 反向隧道                                          │
│  4. 客户端每 30 秒发送心跳保持在线                          │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────────────────────────┐
│                     设备访问流程                             │
├────────────────────────────────────────────────────────────┤
│  1. 用户访问 Access URL (包含 Token)                      │
│  2. Gateway 验证 Token，提取设备/隧道信息                  │
│  3. Gateway 通过已建立的隧道转发请求                       │
│  4. 响应通过隧道返回                                       │
└────────────────────────────────────────────────────────────┘
```

---

## 常见问题

### Q: 如何获取真实的 MAC 地址？

```bash
cat /sys/class/net/*/address | head -1 | tr 'a-f' 'A-F'
```

### Q: 心跳超时时间是多久？

默认 90 秒未收到心跳则标记为离线。

### Q: 一个设备可以有多少个隧道？

可以同时创建多个类型的隧道：`http`, `ws`, `tcp`。

### Q: Access URL 中的 Token 有效期多久？

Token 默认永不过期（TTL=0），直到设备取消注册。
