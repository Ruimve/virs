<div align="center">

# VIRS

**下一代 AI 量化交易平台 — Crypto Only**

Rust 驱动的高性能交易引擎，集成 LLM 决策、实时行情、仓位管理与策略自优化。

![Rust](https://img.shields.io/badge/Rust-1.75+-CE422B?logo=rust&logoColor=white)
![Tests](https://img.shields.io/badge/Tests-572%20passed-brightgreen)
![Crates](https://img.shields.io/badge/Crates-17-blue)
![License](https://img.shields.io/badge/License-MIT-yellow)

</div>

---

## 核心特性

- **AI 驱动交易** — 接入 LLM 进行多周期市场方向分析，自动生成开仓/平仓/观望决策
- **实时行情引擎** — WebSocket 增量深度同步、K 线聚合广播、缺口检测与自动回填
- **完整仓位管理** — 双向持仓（Hedge Only）、确定性 UUID v5 仓位 ID、从成交记录聚合恢复仓位状态
- **25+ 技术指标** — EMA、MACD、RSI、ATR、ADX、布林带等原子/派生指标，支持批量计算
- **策略自优化** — 战术机器人周期性评估策略绩效，通过 LLM 生成改进方案并热更新
- **PaperMode 模拟交易** — 本地撮合引擎，支持限价单回调、保证金追踪与已实现盈亏计算
- **生产级架构** — 6 层分层依赖、trait 抽象解耦、统一错误收敛、virs-task 任务生命周期管理

## 架构总览

```
┌─────────────────────────────────────────────────────────┐
│                     apps/server (L5)                      │
│              Trait 装配 · 引擎管理 · 优雅关闭             │
├─────────────────────────────────────────────────────────┤
│                    crates/api/virs-api (L4)              │
│           REST API · WebSocket 推送 · JWT 鉴权            │
├──────────────┬──────────────┬──────────────┬────────────┤
│  virs-market │ virs-position│virs-trading- │virs-tactical│
│   (L3)       │    (L3)      │    bot (L3)  │ -bot (L3)  │
│ 行情/K线/深度 │ 仓位/成交/   │ Auto引擎/    │ 策略评估/   │
│              │ 恢复          │ 风控/执行    │ LLM优化     │
├──────────────┴──────────────┴──────────────┴────────────┤
│                   core (L2)                              │
│  virs-ccxt · virs-exchange · virs-indicator · virs-prompt│
│  币安适配器 · 交易抽象 · 指标计算 · 策略提示词            │
├─────────────────────────────────────────────────────────┤
│                    infra (L1)                            │
│  virs-type · virs-utils · virs-config · virs-ws · virs-llm│
│  共享类型 · 加密/JWT · 配置 · WS管理 · LLM客户端         │
├─────────────────────────────────────────────────────────┤
│                  foundation (L0)                         │
│         virs-error · virs-task                           │
│    统一错误收敛 · 异步任务生命周期                        │
└─────────────────────────────────────────────────────────┘
         依赖方向：仅向下流动 · 无同层依赖 · 无环
```

## 技术栈

### 后端

| 领域 | 技术 |
|------|------|
| 语言 | Rust 1.75+ |
| 异步运行时 | tokio (full) |
| Web 框架 | axum 0.8 |
| 数据库 | PostgreSQL 16 + sqlx 0.8 |
| WebSocket | tokio-tungstenite (rustls) |
| 序列化 | serde / serde_json |
| 加密 | aes-gcm · sha2 · hmac · ed25519-dalek · bcrypt |
| 日志 | tracing + tracing-subscriber (json) |
| 指标 | talib-rs + 自研指标库 |

### 前端

| 领域 | 技术 |
|------|------|
| 框架 | React 19 |
| 构建 | Vite 8 |
| 样式 | TailwindCSS 4 |
| 路由 | react-router-dom 7 |
| 图表 | lightweight-charts 5 |

## 快速开始

### Docker 一键部署

```bash
# 1. 克隆仓库
git clone https://github.com/yourname/virs.git
cd virs

# 2. 配置环境变量
cp .env.example .env
# 编辑 .env，填入必填项（ENCRYPTION_KEY, LLM_KEY, JWT_SECRET, ADMIN_PASSWORD）

# 3. 启动
docker compose up -d

# 4. 访问
open http://localhost:8080
```

### 本地开发

**前置条件：** Rust 1.75+、PostgreSQL 16+、Node.js 20+

```bash
# 后端
cp .env.example .env          # 配置数据库连接和密钥
cargo build                   # 编译
cargo run -p virs-app         # 启动后端 :8080

# 前端
pnpm install                  # 安装依赖（根目录执行）
pnpm dev                      # 启动开发服务器 :5173
```

## 项目结构

```
virs/
├── crates/
│   ├── foundation/           # L0 — 零依赖基础层
│   │   ├── virs-error/       #   统一错误定义（thiserror + anyhow）
│   │   └── virs-task/        #   异步任务管理（禁止直接 tokio::spawn）
│   ├── infra/                # L1 — 基础设施层
│   │   ├── virs-type/        #   共享类型（CcxtOrder, Position, OrderBook...）
│   │   ├── virs-utils/       #   加密 / JWT 工具
│   │   ├── virs-config/       #   环境变量配置
│   │   ├── virs-ws/          #   WebSocket 连接管理（重连/熔断/心跳）
│   │   └── virs-llm/         #   LLM API 客户端
│   ├── core/                 # L2 — 核心业务层
│   │   ├── virs-ccxt/        #   币安 API 适配器（REST + WS）
│   │   ├── virs-exchange/    #   交易所抽象 + PaperMode
│   │   ├── virs-indicator/   #   技术指标计算（25+ 指标）
│   │   └── virs-prompt/      #   策略提示词加载/渲染/校验
│   ├── services/             # L3 — 服务层
│   │   ├── virs-market/      #   行情引擎（K线聚合/深度同步/缺口回填）
│   │   ├── virs-position/    #   仓位引擎（成交驱动/状态恢复）
│   │   ├── virs-trading-bot/ #   自动交易机器人（AI决策/风控/执行）
│   │   └── virs-tactical-bot/#   战术机器人（策略评估/LLM优化）
│   └── api/
│       └── virs-api/         # L4 — REST API + WebSocket
├── apps/
│   ├── web/                  # 前端应用（React 19 + Vite）
│   └── server/               # L5 — 应用入口（trait 装配/引擎管理）
├── packages/                 # 前端公共包（预留）
├── strategies/               # 策略提示词模板
├── Dockerfile                # 多阶段构建（前端+后端→distroless）
├── docker-compose.yml        # PostgreSQL + Backend
└── .env.example              # 环境变量模板
```

## 配置

### 必填项

| 变量 | 说明 | 示例 |
|------|------|------|
| `ENCRYPTION_KEY` | 交易所凭据加密密钥（64字符，与 LLM_KEY 不同） | `openssl rand -hex 32` |
| `LLM_KEY` | LLM 凭据加密密钥（64字符，与 ENCRYPTION_KEY 不同） | `openssl rand -hex 32` |
| `JWT_SECRET` | JWT 签名密钥（至少 32 字符） | `openssl rand -hex 32` |
| `ADMIN_PASSWORD` | 管理员密码（至少 12 字符） | — |
| `DATABASE_URL` | PostgreSQL 连接字符串 | `postgresql://virs:virs123@localhost:5432/virs` |

### 可选配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `8080` | 后端服务端口 |
| `DB_POOL_MAX` | `50` | 数据库连接池上限 |
| `TIME_LLM_TIMEOUT_SECS` | `120` | LLM API 超时（秒） |
| `TIME_MAX_POSITION_DURATION_SECS` | `172800` | 最大持仓时长（48 小时） |
| `LISTENKEY_KEEPALIVE_FUTURES_SECS` | `1800` | 币安 listenKey 保活间隔 |
| `PROXY_URL` | — | 代理地址（访问交易所 API） |

完整配置见 [`.env.example`](.env.example)。

## 测试

```bash
# 运行全部测试
cargo test

# 运行 clippy 检查
cargo clippy --lib

# 前端类型检查
pnpm --filter @virs/web exec tsc --noEmit
```

| 指标 | 数值 |
|------|------|
| 测试总数 | 572 |
| 单元测试 | 503 |
| 集成测试 | 69 |
| Clippy 警告（lib） | 0 |
| 前端编译 | 通过 |

各 crate 测试用例详情见各自目录下的 `test_plan.md`。

## 支持的交易所

| 交易所 | 合约 | WebSocket | REST |
|--------|------|-----------|------|
| Binance | USDT-M 永续 | K线/深度/用户数据 | 下单/查仓/余额/杠杆 |

> 仅支持 **Hedge（双向持仓）** 模式，OneWay 模式会被拒绝。

## 许证

MIT
