# OpenHuman-ZN (OpenHuman 中文版)

**本地商家的 AI 超级智能助手** | 基于 OpenHuman v0.53.47 构建

## 许可证与商业化说明

本仓库为 [OpenHuman](https://github.com/tinyhumansai/openhuman) 的 GPL v3 fork。
**全部代码（含新增模块）以 GPL v3 发布**，你可以自由使用、修改、分发。

商业化通过 **SaaS 托管服务** 实现，而非代码闭源：
- 桌面端：全开源 (GPL v3)，本仓库
- 许可证验证 API：独立 HTTP 服务（可闭源，通过网络与桌面端通信）
- 支付回调服务：独立 HTTP 服务（可闭源）

此架构符合 GPL v3 §2 系统库例外条款 — 独立进程通过网络协议通信不构成衍生作品。

```
桌面端 (GPL v3)               云端服务 (可闭源)
┌─────────────────┐           ┌──────────────────┐
│ openhuman-zn    │──HTTP──→  │ license-server    │
│ (本仓库)        │←──JSON──  │ (独立仓库)        │
│                 │           │                   │
│ license/ops.rs  │──HTTP──→  │ payment-server    │
│ billing/ops.rs  │←──JSON──  │ (独立仓库)        │
└─────────────────┘           └──────────────────┘
```

## 新增模块

| 模块 | 路径 | 功能 | License |
|------|------|------|---------|
| 许可证系统 | `src/openhuman/license/` | 激活/验证/配额（客户端部分） | GPL v3 |
| 国内支付 | `src/openhuman/billing/china_payments.rs` | 微信+支付宝（客户端部分） | GPL v3 |
| GATE闸机 | `src/openhuman/doctor/gate_check.rs` | 5关启动自检 | GPL v3 |
| 国内模型 | `src/openhuman/config/china_models.rs` | DeepSeek→豆包→通义→Moonshot | GPL v3 |
| 中文Prompt | `agent/prompts/zh-CN/` | 身份+行为+场景模板 | GPL v3 |

## 定价（通过 SaaS 服务实现）

| 方案 | 价格 | 内容 |
|------|------|------|
| 免费试用 | 0元/7天 | 20条/天 |
| 个人版 | 199元/月 | 无限, 2设备 |
| 团队版 | 499元/月 | 多店铺, CSV导入, 10设备 |
| 企业版 | 面议 | SSO, 私有部署 |

## 快速开始

```bash
git clone https://github.com/YOUR_ACCOUNT/openhuman-zn.git
cd openhuman-zn && pnpm install
git submodule update --init --recursive
cp .env.example .env  # 填入 DEEPSEEK_API_KEY
pnpm dev
```

## 上游同步

```bash
git fetch upstream && git merge upstream/main
```

## Copyright

SPDX-License-Identifier: GPL-3.0-only
Copyright (C) 2026 OpenHuman-ZN Contributors
基于 tinyhumansai/openhuman (GPL v3) 构建
