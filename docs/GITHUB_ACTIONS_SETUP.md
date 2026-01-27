# GitHub Actions 配置指南

## 概述

本项目使用 GitHub Actions 进行自动化构建和部署。敏感信息通过 GitHub Secrets 在构建时注入，确保密钥安全。

## 配置 GitHub Secrets

在 GitHub 仓库中配置以下 Secrets：

### 1. 访问仓库设置

1. 进入你的 GitHub 仓库
2. 点击 `Settings` (设置)
3. 在左侧菜单中选择 `Secrets and variables` > `Actions`
4. 点击 `New repository secret` 按钮

### 2. 添加必需的 Secrets

按照以下表格添加所有必需的 secrets：

| Secret 名称                  | 说明                | 示例值                                  |
| ---------------------------- | ------------------- | --------------------------------------- |
| `BUILD_CASDOOR_URL`          | Casdoor 地址        | `https://auth.yes-tek.com`              |
| `BUILD_CASDOOR_CLIENT_ID`    | Casdoor 客户端 ID   | `29fce9095dee17102a87`                  |
| `BUILD_CASDOOR_CLIENT_SECRET`| Casdoor 客户端密钥  | `bc3d2dc95d618b142ee525b3360bdd46dab5b778` |
| `BUILD_GOST_SERVER_ADDR`     | GOST 服务器地址     | `114.55.59.207:19000`                   |
| `BUILD_JWKS_ENDPOINT`        | JWKS 端点           | `https://auth.yes-tek.com/.well-known/jwks` |
| `BUILD_DEFAULT_SERVICE_PORT` | 默认服务端口        | `23001`                                 |

### 3. 额外的可选 Secrets

如果需要发布到 npm，还需要配置：

| Secret 名称  | 说明           | 获取方式                            |
| ------------ | -------------- | ----------------------------------- |
| `NPM_TOKEN`  | npm 认证令牌   | 在 [npmjs.com](https://www.npmjs.com) 生成 |

## 工作流说明

### Build and Deploy (`build-deploy.yml`)

**触发条件：**
- Push 到 `main` 或 `develop` 分支
- 针对 `main` 或 `develop` 的 Pull Request
- 手动触发 (workflow_dispatch)

**主要步骤：**

1. **构建阶段 (build job)**
   - 检出代码
   - 设置 Node.js 和 Rust 环境
   - 安装依赖并缓存
   - 生成 TypeScript 类型
   - 从 Secrets 创建 `.env` 文件
   - 构建前端和后端
   - 运行测试
   - 上传构建产物

2. **Docker 构建阶段 (build-docker job)**
   - 仅在 push 到 main/develop 时触发
   - 构建 Docker 镜像并推送到 GitHub Container Registry
   - 使用 Secrets 作为构建参数传递到 Dockerfile

### 其他工作流

- **test.yml**: 运行测试套件
- **pre-release.yml**: 创建预发布版本
- **publish.yml**: 发布到 npm

## 使用说明

### 自动触发

当你推送代码到 `main` 或 `develop` 分支时，GitHub Actions 会自动：

1. 构建应用程序（使用配置的 Secrets）
2. 运行测试
3. 构建并推送 Docker 镜像到 GHCR

### 手动触发

1. 访问 GitHub 仓库的 `Actions` 标签
2. 选择 `Build and Deploy` 工作流
3. 点击 `Run workflow` 按钮
4. 选择分支并确认

### 查看构建日志

1. 访问 `Actions` 标签
2. 点击任意工作流运行
3. 查看各步骤的详细日志

## Docker 镜像使用

构建的 Docker 镜像会推送到 GitHub Container Registry：

```bash
# 拉取最新镜像
docker pull ghcr.io/<your-username>/vibe-kanban:latest

# 运行容器（需要传递运行时环境变量）
docker run -d \
  -p 23001:3000 \
  -e CASDOOR_URL="$BUILD_CASDOOR_URL" \
  -e CASDOOR_CLIENT_ID="$BUILD_CASDOOR_CLIENT_ID" \
  -e CASDOOR_CLIENT_SECRET="$BUILD_CASDOOR_CLIENT_SECRET" \
  -e GOST_SERVER_ADDR="$BUILD_GOST_SERVER_ADDR" \
  -e JWKS_ENDPOINT="$BUILD_JWKS_ENDPOINT" \
  ghcr.io/<your-username>/vibe-kanban:latest
```

## 环境变量说明

### 构建时变量（Build-time）
这些变量在构建时通过 GitHub Secrets 注入：

- `BUILD_CASDOOR_URL`: Casdoor OAuth 服务地址
- `BUILD_CASDOOR_CLIENT_ID`: Casdoor 应用客户端 ID
- `BUILD_CASDOOR_CLIENT_SECRET`: Casdoor 应用客户端密钥
- `BUILD_GOST_SERVER_ADDR`: GOST 隧道服务器地址
- `BUILD_JWKS_ENDPOINT`: JSON Web Key Set 端点
- `BUILD_DEFAULT_SERVICE_PORT`: 应用监听的默认端口

### 运行时变量（Runtime）
这些变量在运行 Docker 容器时需要提供（如果构建时未硬编码）：

- `CASDOOR_URL`
- `CASDOOR_CLIENT_ID`
- `CASDOOR_CLIENT_SECRET`
- `GOST_SERVER_ADDR`
- `JWKS_ENDPOINT`
- `HOST`: 监听地址（默认：`0.0.0.0`）
- `PORT`: 监听端口（默认：`3000`）

## 安全注意事项

1. **永远不要**在代码中硬编码敏感信息
2. **定期轮换** GitHub Secrets 中的密钥
3. **限制访问权限**：只授予必要的人员访问 Secrets 的权限
4. **监控使用情况**：定期检查 Actions 运行日志，确保没有异常
5. **使用环境保护规则**：为生产环境配置额外的审批流程

## 故障排查

### 构建失败

1. 检查 Actions 日志中的错误信息
2. 验证所有必需的 Secrets 是否已正确配置
3. 确保 Secret 值中没有多余的空格或特殊字符

### Secret 不生效

1. 确认 Secret 名称与工作流文件中的引用完全一致
2. 重新运行工作流，有时需要时间同步
3. 检查是否在正确的环境（repository/environment）中配置了 Secret

### Docker 镜像拉取失败

1. 确保已登录到 GitHub Container Registry
2. 检查镜像标签是否正确
3. 验证仓库的 Package 权限设置

## 相关链接

- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [GitHub Secrets 文档](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Docker Buildx 文档](https://docs.docker.com/buildx/working-with-buildx/)
