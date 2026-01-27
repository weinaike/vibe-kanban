#!/bin/bash

# ============================================
# GitHub Secrets 配置脚本
# ============================================
# 
# 此脚本帮助您快速将环境变量配置到 GitHub Secrets
# 
# 使用方法：
#   1. 安装 GitHub CLI: https://cli.github.com/
#   2. 登录 GitHub: gh auth login
#   3. 运行此脚本: ./scripts/setup-github-secrets.sh
# ============================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检查 GitHub CLI 是否安装
if ! command -v gh &> /dev/null; then
    echo -e "${RED}错误: GitHub CLI (gh) 未安装${NC}"
    echo -e "${YELLOW}请访问 https://cli.github.com/ 安装 GitHub CLI${NC}"
    exit 1
fi

# 检查是否已登录
if ! gh auth status &> /dev/null; then
    echo -e "${RED}错误: 未登录 GitHub CLI${NC}"
    echo -e "${YELLOW}请运行: gh auth login${NC}"
    exit 1
fi

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}GitHub Secrets 配置向导${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# 获取当前仓库信息
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
echo -e "${GREEN}当前仓库: ${REPO}${NC}"
echo ""

# 定义需要配置的 secrets
declare -A SECRETS=(
    ["BUILD_CASDOOR_URL"]="Casdoor 服务器地址 (例如: https://auth.yes-tek.com)"
    ["BUILD_CASDOOR_CLIENT_ID"]="Casdoor 客户端 ID"
    ["BUILD_CASDOOR_CLIENT_SECRET"]="Casdoor 客户端密钥 (敏感信息)"
    ["BUILD_GOST_SERVER_ADDR"]="GOST 服务器地址和端口 (例如: 114.55.59.207:19000)"
    ["BUILD_JWKS_ENDPOINT"]="JWKS 端点 (例如: https://auth.yes-tek.com/.well-known/jwks)"
    ["BUILD_DEFAULT_SERVICE_PORT"]="默认服务端口 (例如: 23001)"
)

# 尝试从 .env 文件读取默认值
declare -A DEFAULT_VALUES
if [ -f ".env" ]; then
    echo -e "${YELLOW}发现 .env 文件，将使用其中的值作为默认值${NC}"
    echo ""
    
    while IFS='=' read -r key value; do
        # 跳过注释和空行
        [[ $key =~ ^#.*$ ]] && continue
        [[ -z $key ]] && continue
        
        # 移除值前后的空格和引号
        value=$(echo "$value" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e 's/^"//' -e 's/"$//')
        
        # 映射 .env 中的键到 BUILD_ 前缀的键
        case $key in
            CASDOOR_URL)
                DEFAULT_VALUES["BUILD_CASDOOR_URL"]="$value"
                ;;
            CASDOOR_CLIENT_ID)
                DEFAULT_VALUES["BUILD_CASDOOR_CLIENT_ID"]="$value"
                ;;
            CASDOOR_CLIENT_SECRET)
                DEFAULT_VALUES["BUILD_CASDOOR_CLIENT_SECRET"]="$value"
                ;;
            GOST_SERVER_ADDR)
                DEFAULT_VALUES["BUILD_GOST_SERVER_ADDR"]="$value"
                ;;
            JWKS_ENDPOINT)
                DEFAULT_VALUES["BUILD_JWKS_ENDPOINT"]="$value"
                ;;
            FRONTEND_PORT)
                DEFAULT_VALUES["BUILD_DEFAULT_SERVICE_PORT"]="$value"
                ;;
        esac
    done < .env
fi

# 交互式配置每个 secret
echo -e "${YELLOW}请输入以下配置信息（按 Enter 使用默认值）:${NC}"
echo ""

declare -A VALUES

for secret in "${!SECRETS[@]}"; do
    description="${SECRETS[$secret]}"
    default="${DEFAULT_VALUES[$secret]}"
    
    if [ -n "$default" ]; then
        # 对于敏感信息，只显示部分内容
        if [[ $secret == *"SECRET"* ]] || [[ $secret == *"TOKEN"* ]]; then
            display_default="${default:0:8}..."
        else
            display_default="$default"
        fi
        echo -e "${BLUE}${description}${NC}"
        read -p "$(echo -e ${GREEN}${secret}${NC}) [默认: ${display_default}]: " value
    else
        echo -e "${BLUE}${description}${NC}"
        read -p "$(echo -e ${GREEN}${secret}${NC}): " value
    fi
    
    # 如果用户未输入，使用默认值
    if [ -z "$value" ] && [ -n "$default" ]; then
        value="$default"
    fi
    
    # 验证必填字段
    if [ -z "$value" ]; then
        echo -e "${RED}错误: ${secret} 不能为空${NC}"
        exit 1
    fi
    
    VALUES[$secret]="$value"
    echo ""
done

# 确认配置
echo -e "${YELLOW}============================================${NC}"
echo -e "${YELLOW}请确认以下配置:${NC}"
echo -e "${YELLOW}============================================${NC}"
echo ""

for secret in "${!VALUES[@]}"; do
    value="${VALUES[$secret]}"
    # 对于敏感信息，只显示部分内容
    if [[ $secret == *"SECRET"* ]] || [[ $secret == *"TOKEN"* ]]; then
        display_value="${value:0:8}...******"
    else
        display_value="$value"
    fi
    echo -e "${GREEN}${secret}${NC}: ${display_value}"
done

echo ""
read -p "$(echo -e ${YELLOW}确认配置这些 secrets 到 GitHub? [y/N]: ${NC})" confirm

if [[ ! $confirm =~ ^[Yy]$ ]]; then
    echo -e "${RED}已取消${NC}"
    exit 0
fi

# 设置 secrets
echo ""
echo -e "${BLUE}正在配置 GitHub Secrets...${NC}"
echo ""

for secret in "${!VALUES[@]}"; do
    value="${VALUES[$secret]}"
    echo -e "${GREEN}设置 ${secret}...${NC}"
    
    if echo "$value" | gh secret set "$secret" --repo "$REPO"; then
        echo -e "${GREEN}✓ ${secret} 已设置${NC}"
    else
        echo -e "${RED}✗ ${secret} 设置失败${NC}"
    fi
    echo ""
done

echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}配置完成！${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo -e "${YELLOW}提示：您可以运行以下命令查看已配置的 secrets:${NC}"
echo -e "${BLUE}gh secret list --repo ${REPO}${NC}"
echo ""
echo -e "${YELLOW}现在可以触发 GitHub Actions 工作流了！${NC}"
