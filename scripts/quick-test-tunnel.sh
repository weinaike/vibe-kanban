#!/bin/bash
# 隧道服务快速测试脚本

GATEWAY="https://ziso-backend.yes-tek.com"
API_BASE="$GATEWAY/api/v1"

echo "============================================"
echo "隧道服务快速测试 (域名: $GATEWAY)"
echo "============================================"

# 1. 获取登录 URL
echo ""
echo "步骤 1: 获取登录 URL"
echo "----------------------------"
LOGIN_URL=$(curl -s "$GATEWAY/api/auth/login?redirect_uri=http://localhost/callback" | jq -r '.login_url')
echo "请访问以下 URL 进行登录:"
echo ""
echo "$LOGIN_URL"
echo ""
echo "登录后，浏览器会跳转到 callback URL，"
echo "URL 中包含 code 参数，请复制完整的 callback URL"
echo ""

# 2. 获取 token
read -p "请输入 callback URL: " CALLBACK_URL

# 提取 code
CODE=$(echo "$CALLBACK_URL" | grep -oP 'code=\K[^&]+' | head -1)

if [ -z "$CODE" ]; then
    echo "错误: 无法从 URL 中提取 code"
    exit 1
fi

echo ""
echo "步骤 2: 使用 code 获取 token"
echo "----------------------------"
TOKEN_RESPONSE=$(curl -s "$GATEWAY/api/auth/callback?code=$CODE&state=test")
ACCESS_TOKEN=$(echo "$TOKEN_RESPONSE" | jq -r '.access_token')

if [ -z "$ACCESS_TOKEN" ] || [ "$ACCESS_TOKEN" = "null" ]; then
    echo "错误: 无法获取 access_token"
    echo "响应: $TOKEN_RESPONSE"
    exit 1
fi

echo "✓ Token 获取成功"
echo "  Token: ${ACCESS_TOKEN:0:50}..."

# 保存 token
export CASDOOR_TOKEN="$ACCESS_TOKEN"

# 3. 列出设备
echo ""
echo "步骤 3: 列出已有设备"
echo "----------------------------"
curl -s "$API_BASE/devices" \
    -H "Authorization: Bearer $CASDOOR_TOKEN" \
    -H "Content-Type: application/json" | jq '.'

# 4. 注册设备
echo ""
echo "步骤 4: 注册新设备"
echo "----------------------------"

# 获取 MAC 地址
MAC=$(cat /sys/class/net/*/address 2>/dev/null | head -1 | tr 'a-f' 'A-F' | head -1)
if [ -z "$MAC" ]; then
    MAC="00:11:22:33:44:55"
fi

DEVICE_NAME="test-device-$(date +%s)"

echo "MAC 地址: $MAC"
echo "设备名称: $DEVICE_NAME"

REGISTER_RESPONSE=$(curl -s -X POST "$API_BASE/devices/register" \
    -H "Authorization: Bearer $CASDOOR_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"mac_address\": \"$MAC\",
        \"device_name\": \"$DEVICE_NAME\",
        \"device_type\": \"test\",
        \"firmware_version\": \"1.0.0\",
        \"tunnel_types\": [\"http\", \"ws\"]
    }")

echo "$REGISTER_RESPONSE" | jq '.'

DEVICE_ID=$(echo "$REGISTER_RESPONSE" | jq -r '.device_id')

if [ -z "$DEVICE_ID" ] || [ "$DEVICE_ID" = "null" ]; then
    echo "错误: 设备注册失败"
    exit 1
fi

echo "✓ 设备注册成功"
echo "  设备 ID: $DEVICE_ID"

# 显示隧道配置
echo ""
echo "隧道配置:"
echo "----------------------------"
echo "$REGISTER_RESPONSE" | jq -r '.tunnels[] | "
\(.tunnel_type | ascii_upcase):
  隧道ID: \(.tunnel_id)
  访问URL: \(.access_url)
  本地端口: \(.local_port)
  GOST配置:
    -server \(.gost_config.server_addr)
    -tunnel-id \(.gost_config.tunnel_id)
    -local \(.gost_config.local_addr)
    -forward \(.gost_config.forwarder)
"'

# 5. 发送心跳
echo ""
echo "步骤 5: 发送心跳"
echo "----------------------------"
HEARTBEAT_RESPONSE=$(curl -s -X POST "$API_BASE/devices/$DEVICE_ID/heartbeat")
echo "$HEARTBEAT_RESPONSE" | jq '.'
echo "✓ 心跳成功"

# 6. 查询设备状态
echo ""
echo "步骤 6: 查询设备状态"
echo "----------------------------"
STATUS_RESPONSE=$(curl -s "$API_BASE/devices/$DEVICE_ID" \
    -H "Authorization: Bearer $CASDOOR_TOKEN")
echo "$STATUS_RESPONSE" | jq '.'

# 7. 测试访问 URL (如果有)
echo ""
echo "步骤 7: 测试访问 URL"
echo "----------------------------"
ACCESS_URL=$(echo "$REGISTER_RESPONSE" | jq -r '.tunnels[0].access_url')
echo "访问 URL: $ACCESS_URL"
echo ""
echo "在浏览器中访问上面的 URL，应该能看到你的本地服务"

echo ""
echo "============================================"
echo "测试完成!"
echo "============================================"
echo ""
echo "后续操作:"
echo "  1. 启动 GOST 客户端 (如果需要)"
echo "  2. 每 30 秒发送一次心跳"
echo "  3. 测试完成后取消注册"
echo ""
echo "取消注册命令:"
echo "  curl -X DELETE $API_BASE/devices/$DEVICE_ID/unregister \\"
echo "    -H 'Authorization: Bearer $CASDOOR_TOKEN'"
echo ""
echo "持续心跳:"
echo "  while true; do sleep 30; curl -s -X POST $API_BASE/devices/$DEVICE_ID/heartbeat | jq .; done"
echo ""
