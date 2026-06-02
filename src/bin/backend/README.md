# OpenHuman-ZN 国产化 Backend 部署指南

## 快速启动

```bash
# 1. 编译
cargo build --release --bin openhuman-backend

# 2. 设置环境变量（或通过命令行参数）
export OPENHUMAN_JWT_SECRET="$(openssl rand -hex 32)"
export FEISHU_APP_ID="cli_xxxxxxxxxxxx"
export FEISHU_APP_SECRET="your-feishu-app-secret"

# 3. 启动
./target/release/openhuman-backend \
    --jwt-secret "$OPENHUMAN_JWT_SECRET" \
    --feishu-app-id "$FEISHU_APP_ID" \
    --feishu-app-secret "$FEISHU_APP_SECRET" \
    --bind 0.0.0.0:3000
```

## core 端接入

将 core 的 `config.toml` 中 `api_url` 指向国产 backend：

```toml
api_url = "http://your-ecs-ip:3000"
```

或者在 `app/.env.local` 中设置：

```env
VITE_BACKEND_URL=http://your-ecs-ip:3000
```

## API 端点

| 路径 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/auth/me` | GET | 获取当前用户（需 Bearer token） |
| `/auth/login` | POST | 邮箱登录/注册 |
| `/auth/feishu/login` | GET | 飞书 OAuth 授权 URL |
| `/auth/feishu/callback` | GET | 飞书 OAuth 回调 |
| `/payments/wechat/create` | POST | 创建微信支付订单 |
| `/payments/alipay/create` | POST | 创建支付宝订单 |
| `/payments/status/:id` | GET | 查询订单状态 |
| `/payments/plans/cn` | GET | 获取国产套餐列表 |
| `/api/v1/license/activate` | POST | 许可证激活 |
| `/channels/:channel/messages` | POST | 发送消息到 IM 通道 |
| `/webhooks/feishu/event` | POST | 飞书事件回调 |

## 飞书集成说明

1. 在[飞书开放平台](https://open.feishu.cn)创建企业自建应用
2. 获取 App ID 和 App Secret
3. 配置 OAuth 回调 URL: `http://your-server:3000/auth/feishu/callback`
4. 配置事件订阅 URL: `http://your-server:3000/webhooks/feishu/event`
5. 添加权限：`im:message`、`contact:user.base`
