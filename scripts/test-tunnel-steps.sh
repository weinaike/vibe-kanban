#!/bin/bash
# 隧道服务分步测试指南

GATEWAY="https://ziso-backend.yes-tek.com"
API_BASE="$GATEWAY/api/v1"

cat << 'EOF'
╔══════════════════════════════════════════════════════════════════════╗
║                    隧道服务业务流程测试                              ║
║                    域名: ziso-backend.yes-tek.com                    ║
╚══════════════════════════════════════════════════════════════════════╝

📋 测试步骤:

═══════════════════════════════════════════════════════════════════════
步骤 1: 获取登录 URL
═══════════════════════════════════════════════════════════════════════

在浏览器中访问以下 URL 进行登录:

EOF

# 获取登录 URL
LOGIN_URL=$(curl -s "$GATEWAY/api/v1/auth/login?redirect_uri=http://localhost/callback" | jq -r '.login_url')
echo "$LOGIN_URL"

cat << 'EOF'

登录后会跳转到: https://ziso-backend.yes-tek.com/callback?code=xxx&state=xxx

复制浏览器地址栏中的完整 callback URL

═══════════════════════════════════════════════════════════════════════
步骤 2: 获取 Access Token
═══════════════════════════════════════════════════════════════════════

将下面的 CALLBACK_URL 替换为你复制的 URL，然后执行:

curl -s "EOF
echo "$GATEWAY/api/v1/auth/callback?code=YOUR_CODE&state=test"
cat << 'EOF'

或者设置环境变量后继续:

export CALLBACK_URL="你复制的完整URL"

═══════════════════════════════════════════════════════════════════════
步骤 3: 列出已有设备
═══════════════════════════════════════════════════════════════════════

curl -s "EOF
echo "$API_BASE/devices"
cat << 'EOF'
" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" | jq '.'

═══════════════════════════════════════════════════════════════════════
步骤 4: 注册新设备
═══════════════════════════════════════════════════════════════════════

curl -s -X POST "EOF
echo "$API_BASE/devices/register"
cat << 'EOF'
" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "mac_address": "00:11:22:33:44:55",
    "device_name": "test-device",
    "device_type": "test",
    "firmware_version": "1.0.0",
    "tunnel_types": ["http", "ws"]
  }' | jq '.'

═══════════════════════════════════════════════════════════════════════
步骤 5: 发送心跳
═══════════════════════════════════════════════════════════════════════

curl -s -X POST "EOF
echo "$API_BASE/devices/DEVICE_ID/heartbeat"
cat << 'EOF'
" | jq '.'

每 30 秒发送一次心跳:

while true; do
  echo "$(date '+%H:%M:%S') - 发送心跳..."
  curl -s -X POST "EOF
echo "$API_BASE/devices/DEVICE_ID/heartbeat"
cat << 'EOF'
" | jq '.'
  sleep 30
done

═══════════════════════════════════════════════════════════════════════
步骤 6: 取消注册设备
═══════════════════════════════════════════════════════════════════════

curl -s -X DELETE "EOF
echo "$API_BASE/devices/DEVICE_ID/unregister"
cat << 'EOF'
" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" | jq '.'

═══════════════════════════════════════════════════════════════════════
═══════════════════════════════════════════════════════════════════════

现在打开浏览器，访问以下登录 URL:

EOF

echo "$LOGIN_URL"
echo ""
