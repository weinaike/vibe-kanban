#!/bin/bash
# 隧道服务业务流程调试脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 配置
GATEWAY_URL="https://ziso-backend.yes-tek.com"
API_BASE="$GATEWAY_URL/api/v1"

# 打印消息
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

step() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}>>> $1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

# 检查服务状态
check_service() {
    step "检查服务状态"

    info "检查本地 Gateway 服务..."
    if curl -s -f "$LOCAL_GATEWAY/health" > /dev/null 2>&1; then
        info "✓ 本地 Gateway 服务运行正常"
    else
        warn "本地 Gateway 服务未运行，尝试远程服务"
        if curl -s -f "$GATEWAY_URL/health" > /dev/null 2>&1; then
            info "✓ 远程 Gateway 服务运行正常"
        else
            error "Gateway 服务无法访问"
            return 1
        fi
    fi
}

# 获取 Casdoor Token
get_casdoor_token() {
    step "获取 Casdoor 访问令牌"

    info "请访问以下 URL 进行登录并获取 token:"
    echo -e "${YELLOW}$GATEWAY_URL/api/auth/login${NC}\n"

    read -p "请输入从回调 URL 中获取的 access_token: " ACCESS_TOKEN

    if [ -z "$ACCESS_TOKEN" ]; then
        error "未提供 access_token"
        return 1
    fi

    info "Token 已设置"
    export CASDOOR_TOKEN="$ACCESS_TOKEN"
}

# 注册设备
register_device() {
    step "注册设备"

    # 获取真实 MAC 地址
    MAC_ADDRESS=$(cat /sys/class/net/*/address 2>/dev/null | head -1 | tr 'a-f' 'A-F')
    if [ -z "$MAC_ADDRESS" ]; then
        # 回退到随机生成
        MAC_ADDRESS=$(printf '%02X:%02X:%02X:%02X:%02X:%02X' $((RANDOM%256)) $((RANDOM%256)) $((RANDOM%256)) $((RANDOM%256)) $((RANDOM%256)) $((RANDOM%256)))
    fi

    DEVICE_NAME="test-device-$(date +%s)"

    info "MAC 地址: $MAC_ADDRESS"
    info "设备名称: $DEVICE_NAME"

    local request=$(cat <<EOF
{
  "mac_address": "$MAC_ADDRESS",
  "device_name": "$DEVICE_NAME",
  "device_type": "test",
  "firmware_version": "1.0.0",
  "tunnel_types": ["http", "ws"]
}
EOF
)

    info "发送注册请求..."
    local response=$(curl -s -w "\n%{http_code}" -X POST "$API_BASE/devices/register" \
        -H "Authorization: Bearer $CASDOOR_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$request")

    local http_code=$(echo "$response" | tail -1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" = "200" ]; then
        info "✓ 设备注册成功!"
        echo "$body" | jq '.'

        # 保存设备信息
        export DEVICE_ID=$(echo "$body" | jq -r '.device_id')
        export MAC_ADDRESS=$MAC_ADDRESS

        info "设备 ID: $DEVICE_ID"

        # 显示隧道配置
        info "\n隧道配置:"
        echo "$body" | jq -r '.tunnels[] | "  隧道类型: \(.tunnel_type)\n  隧道ID: \(.tunnel_id)\n  访问URL: \(.access_url)\n  本地端口: \(.local_port)\n  GOST配置:\n    server_addr: \(.gost_config.server_addr)\n    tunnel_id: \(.gost_config.tunnel_id)\n    local_addr: \(.gost_config.local_addr)\n    forwarder: \(.gost_config.forwarder)\n"'

        return 0
    else
        error "设备注册失败 (HTTP $http_code)"
        echo "$body"
        return 1
    fi
}

# 启动 GOST 客户端
start_gost_client() {
    step "启动 GOST 客户端"

    if [ -z "$DEVICE_ID" ]; then
        error "请先注册设备"
        return 1
    fi

    # 获取隧道配置
    info "获取设备隧道配置..."
    local tunnels=$(curl -s "$API_BASE/devices/$DEVICE_ID/tunnels" \
        -H "Authorization: Bearer $CASDOOR_TOKEN")

    echo "$tunnels" | jq '.'

    # 检查是否有 GOST 客户端二进制
    GOST_CLIENT_BIN="./bin/gost_client"
    if [ ! -f "$GOST_CLIENT_BIN" ]; then
        warn "GOST 客户端不存在: $GOST_CLIENT_BIN"
        info "请手动启动 GOST 客户端进程"
        return 0
    fi

    info "启动 GOST 客户端进程..."

    # 从 API 响应中提取配置
    local tunnel_count=$(echo "$tunnels" | jq '. | length')

    for ((i=0; i<tunnel_count; i++)); do
        local config=$(echo "$tunnels" | jq ".[$i].gost_config")
        local server_addr=$(echo "$config" | jq -r '.server_addr')
        local tunnel_id=$(echo "$config" | jq -r '.tunnel_id')
        local local_addr=$(echo "$config" | jq -r '.local_addr')
        local forwarder=$(echo "$config" | jq -r '.forwarder')

        info "启动隧道 $i: $tunnel_id"

        nohup "$GOST_CLIENT_BIN" \
            -server "$server_addr" \
            -tunnel-id "$tunnel_id" \
            -local "$local_addr" \
            -forward "$forwarder" \
            > /tmp/gost_client_$i.log 2>&1 &

        local pid=$!
        echo "$pid" > /tmp/gost_client_$i.pid
        info "  PID: $pid"
        info "  日志: /tmp/gost_client_$i.log"
    done

    info "GOST 客户端进程已启动"
}

# 发送心跳
send_heartbeat() {
    step "发送心跳"

    if [ -z "$DEVICE_ID" ]; then
        error "请先注册设备"
        return 1
    fi

    info "发送心跳到设备 $DEVICE_ID..."

    local response=$(curl -s -w "\n%{http_code}" -X POST "$API_BASE/devices/$DEVICE_ID/heartbeat")

    local http_code=$(echo "$response" | tail -1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" = "200" ]; then
        info "✓ 心跳成功"
        echo "$body" | jq '.'
    else
        error "心跳失败 (HTTP $http_code)"
        echo "$body"
    fi
}

# 查询设备状态
get_device_status() {
    step "查询设备状态"

    if [ -z "$DEVICE_ID" ]; then
        error "请先注册设备"
        return 1
    fi

    info "查询设备 $DEVICE_ID 状态..."

    local response=$(curl -s -w "\n%{http_code}" -X GET "$API_BASE/devices/$DEVICE_ID" \
        -H "Authorization: Bearer $CASDOOR_TOKEN")

    local http_code=$(echo "$response" | tail -1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" = "200" ]; then
        info "✓ 设备信息:"
        echo "$body" | jq '.'
    else
        error "查询失败 (HTTP $http_code)"
        echo "$body"
    fi
}

# 取消注册
unregister_device() {
    step "取消注册设备"

    if [ -z "$DEVICE_ID" ]; then
        error "请先注册设备"
        return 1
    fi

    warn "即将取消注册设备: $DEVICE_ID"
    read -p "确认? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        info "已取消"
        return 0
    fi

    info "发送取消注册请求..."

    local response=$(curl -s -w "\n%{http_code}" -X DELETE "$API_BASE/devices/$DEVICE_ID/unregister" \
        -H "Authorization: Bearer $CASDOOR_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"reason": "debug test"}')

    local http_code=$(echo "$response" | tail -1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" = "200" ]; then
        info "✓ 设备取消注册成功"
        echo "$body" | jq '.'
        unset DEVICE_ID
    else
        error "取消注册失败 (HTTP $http_code)"
        echo "$body"
    fi
}

# 停止 GOST 客户端
stop_gost_clients() {
    step "停止 GOST 客户端"

    info "停止所有 GOST 客户端进程..."

    for pidfile in /tmp/gost_client_*.pid; do
        if [ -f "$pidfile" ]; then
            local pid=$(cat "$pidfile")
            if ps -p "$pid" > /dev/null 2>&1; then
                info "停止 PID $pid..."
                kill "$pid"
            fi
            rm -f "$pidfile"
        fi
    done

    info "✓ 所有 GOST 客户端已停止"
}

# 列出设备
list_devices() {
    step "列出所有设备"

    info "获取设备列表..."

    local response=$(curl -s -w "\n%{http_code}" -X GET "$API_BASE/devices" \
        -H "Authorization: Bearer $CASDOOR_TOKEN")

    local http_code=$(echo "$response" | tail -1)
    local body=$(echo "$response" | head -n -1)

    if [ "$http_code" = "200" ]; then
        info "✓ 设备列表:"
        echo "$body" | jq '.'
    else
        error "获取列表失败 (HTTP $http_code)"
        echo "$body"
    fi
}

# 运行完整流程
run_full_flow() {
    step "运行完整业务流程"

    check_service
    get_casdoor_token
    register_device
    start_gost_client

    # 持续发送心跳
    info "每 30 秒发送一次心跳 (Ctrl+C 退出)..."
    local count=0
    while true; do
        sleep 30
        ((count++))
        info "[心跳 #$count] $(date '+%H:%M:%S')"
        send_heartbeat
    done
}

# 菜单
show_menu() {
    cat << EOF
隧道服务业务流程调试脚本

用法: $0 <command>

命令:
    check           检查服务状态
    token           获取 Casdoor 访问令牌
    register        注册设备
    start-gost      启动 GOST 客户端
    heartbeat       发送心跳
    status          查询设备状态
    list            列出所有设备
    unregister      取消注册设备
    stop-gost       停止 GOST 客户端
    flow            运行完整流程 (注册 -> 启动 -> 心跳)

环境变量:
    CASDOOR_TOKEN   Casdoor 访问令牌
    DEVICE_ID       当前设备 ID

示例:
    $0 token                    # 获取令牌
    $0 register                 # 注册设备
    $0 heartbeat                # 发送心跳
    $0 flow                     # 运行完整流程

EOF
}

# 主函数
main() {
    local command="${1:-help}"

    case "$command" in
        check)
            check_service
            ;;
        token)
            get_casdoor_token
            ;;
        register)
            register_device
            ;;
        start-gost)
            start_gost_client
            ;;
        heartbeat)
            send_heartbeat
            ;;
        status)
            get_device_status
            ;;
        list)
            list_devices
            ;;
        unregister)
            unregister_device
            ;;
        stop-gost)
            stop_gost_clients
            ;;
        flow)
            run_full_flow
            ;;
        help|--help|-h)
            show_menu
            ;;
        *)
            error "未知命令: $command"
            show_menu
            exit 1
            ;;
    esac
}

main "$@"
