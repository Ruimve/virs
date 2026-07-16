-- VIRS - Database Schema
-- PostgreSQL 16+
-- Crypto-only trading platform
--
-- 注意：本文件为全量初始化脚本，会重新创建整个数据库。
-- 所有字段及约束已聚合到初始 CREATE TABLE 中，无增量 ALTER 语句。

-- ============================================================
-- Users & Authentication
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'manager', 'user', 'viewer')),
    email TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_qd_users_email ON qd_users(email) WHERE email IS NOT NULL;

-- ============================================================
-- Exchange Credentials (encrypted)
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_exchange_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    exchange TEXT NOT NULL,
    encrypted_api_key TEXT NOT NULL,
    encrypted_api_secret TEXT NOT NULL,
    encrypted_passphrase TEXT,
    label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, exchange)
);

CREATE INDEX IF NOT EXISTS idx_credentials_user ON qd_exchange_credentials(user_id);

-- ============================================================
-- AI/LLM Credentials (per-user, encrypted)
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_ai_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    encrypted_api_key TEXT NOT NULL,
    model TEXT,
    label TEXT,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_ai_credentials_user ON qd_ai_credentials(user_id);

-- ============================================================
-- Grid Trading Bots
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_grid_bots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL DEFAULT 'binance',
    paper_mode BOOLEAN NOT NULL DEFAULT true,
    status TEXT NOT NULL DEFAULT 'stopped' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    -- Grid 配置参数
    upper_price DOUBLE PRECISION NOT NULL,
    lower_price DOUBLE PRECISION NOT NULL,
    grid_count INT NOT NULL,
    grid_profit_pct DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    quantity_per_grid DOUBLE PRECISION NOT NULL,
    leverage INT NOT NULL DEFAULT 1,
    initial_capital DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- AI 分析相关（内部字段，API 不返回）
    market_regime TEXT,
    ai_analysis TEXT,
    grid_levels_json JSONB,
    system_prompt TEXT,
    user_prompt TEXT,

    -- 动态调整（内部字段，API 不返回）
    dynamic_adjust BOOLEAN NOT NULL DEFAULT true,
    adjust_interval_secs INT NOT NULL DEFAULT 300,
    last_adjusted_at TIMESTAMPTZ,

    -- 统计缓存（denormalized，由 worker 定期同步）
    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    grid_filled_count INT NOT NULL DEFAULT 0,

    -- 生命周期时间戳（内部字段，API 不返回）
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_grid_bots_user ON qd_grid_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_grid_bots_status ON qd_grid_bots(status);

-- Grid Trades（开/平仓对）
CREATE TABLE IF NOT EXISTS qd_grid_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    grid_level INT NOT NULL,

    -- 开仓
    open_side TEXT NOT NULL CHECK (open_side IN ('buy', 'sell')),
    open_price DOUBLE PRECISION NOT NULL,
    open_quantity DOUBLE PRECISION NOT NULL,
    open_order_id TEXT,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 平仓（未平仓时为 NULL）
    close_side TEXT CHECK (close_side IN ('buy', 'sell')),
    close_price DOUBLE PRECISION,
    close_quantity DOUBLE PRECISION,
    close_order_id TEXT,
    closed_at TIMESTAMPTZ,

    -- 盈亏
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_pct DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- 状态：open=持仓中, closed=已平仓, orphaned=孤儿记录（无对应开仓）
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'orphaned')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_grid_trades_bot ON qd_grid_trades(bot_id);
CREATE INDEX IF NOT EXISTS idx_grid_trades_user ON qd_grid_trades(user_id);
CREATE INDEX IF NOT EXISTS idx_grid_trades_status ON qd_grid_trades(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_grid_trades_created ON qd_grid_trades(created_at DESC);

-- Grid LLM 分析日志
CREATE TABLE IF NOT EXISTS qd_grid_analysis_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    analysis_type TEXT NOT NULL DEFAULT 'periodic',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    system_prompt TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    llm_model TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_grid_analysis_logs_bot ON qd_grid_analysis_logs(bot_id, created_at DESC);

-- ============================================================
-- Auto Trading Bots (全自动交易机器人)
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_auto_bots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL DEFAULT 'binance',
    paper_mode BOOLEAN NOT NULL DEFAULT true,
    status TEXT NOT NULL DEFAULT 'stopped' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    -- 交易参数
    leverage INT NOT NULL DEFAULT 1,
    max_position_pct DOUBLE PRECISION NOT NULL DEFAULT 80.0,
    decide_interval_secs INT NOT NULL DEFAULT 300,
    initial_capital DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- AI 分析相关（内部字段，API 不返回）
    market_regime TEXT,
    ai_analysis TEXT,
    system_prompt TEXT,
    user_prompt TEXT,

    -- 统计缓存（denormalized，由 worker 定期同步）
    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    win_trades INT NOT NULL DEFAULT 0,
    loss_trades INT NOT NULL DEFAULT 0,

    -- 仓位追踪（跨系统引用 PositionEngine 内存中的 Position.id）
    position_id UUID,

    -- 生命周期时间戳（内部字段，API 不返回）
    last_decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_bots_user ON qd_auto_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_auto_bots_status ON qd_auto_bots(status);

-- Auto Trades（开/平仓对模型，与 qd_grid_trades 语义一致）
CREATE TABLE IF NOT EXISTS qd_auto_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_auto_bots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,

    -- 开仓
    open_side TEXT NOT NULL CHECK (open_side IN ('buy', 'sell')),
    open_price DOUBLE PRECISION NOT NULL,
    open_quantity DOUBLE PRECISION NOT NULL,
    open_order_id TEXT,
    open_fee DOUBLE PRECISION NOT NULL DEFAULT 0,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 平仓（未平仓时为 NULL）
    close_side TEXT CHECK (close_side IN ('buy', 'sell')),
    close_price DOUBLE PRECISION,
    close_quantity DOUBLE PRECISION,
    close_order_id TEXT,
    close_fee DOUBLE PRECISION NOT NULL DEFAULT 0,
    closed_at TIMESTAMPTZ,

    -- 盈亏（平仓后填入：pnl = gross_pnl - open_fee - close_fee）
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_pct DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- 风控边界（开仓时记录，trailing stop 更新时覆盖，用于审计与前端展示）
    stop_loss DOUBLE PRECISION NOT NULL DEFAULT 0,
    take_profit DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- 平仓原因：stop_loss/take_profit/position_timeout/llm_decision
    -- 由代码逻辑决定（不由 LLM 决定），用于冷却期判断和前端展示
    close_reason TEXT CHECK (close_reason IS NULL OR close_reason IN ('stop_loss', 'take_profit', 'position_timeout', 'llm_decision')),

    -- 状态：open=持仓中, closed=已平仓, orphaned=孤儿记录（无对应开仓）
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'orphaned')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auto_trades_bot ON qd_auto_trades(bot_id, opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_auto_trades_user ON qd_auto_trades(user_id);
CREATE INDEX IF NOT EXISTS idx_auto_trades_status ON qd_auto_trades(bot_id, status);

-- Auto LLM 分析日志
CREATE TABLE IF NOT EXISTS qd_auto_analysis_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_auto_bots(id) ON DELETE CASCADE,
    analysis_type TEXT NOT NULL DEFAULT 'periodic',
    -- 执行状态：pending=待执行, completed=已执行, failed=执行失败, intercepted=被代码拦截
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed', 'intercepted')),
    system_prompt TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    llm_model TEXT NOT NULL DEFAULT '',
    -- 执行回填字段（在订单成交/拦截发生时回填）
    -- intercept_reason: LLM 决策被代码拦截时的原因（如冷却期/置信度不足）
    -- execution_status: open=开仓成功, open_failed=开仓失败, close=平仓成功, close_failed=平仓失败, hold=观望
    -- 注：close_reason 不在此表，已记录在 qd_auto_trades.close_reason
    intercept_reason TEXT,
    execution_status TEXT CHECK (execution_status IS NULL OR execution_status IN ('open', 'open_failed', 'close', 'close_failed', 'hold')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_analysis_logs_bot ON qd_auto_analysis_logs(bot_id, created_at DESC);

-- ============================================================
-- Position Engine (pe_positions)
-- ============================================================
-- pe_positions: 引擎重启后快速恢复内存仓位状态
-- pe_orders: 订单完整生命周期持久化，按 client_order_id 更新

CREATE TABLE IF NOT EXISTS pe_positions (
    id              UUID PRIMARY KEY,
    strategy_id     TEXT,
    exchange        TEXT NOT NULL,
    symbol          TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('Long', 'Short')),
    status          TEXT NOT NULL,
    size            DOUBLE PRECISION NOT NULL,
    entry_price     DOUBLE PRECISION NOT NULL,
    current_price   DOUBLE PRECISION NOT NULL,
    leverage        INT NOT NULL,
    margin          DOUBLE PRECISION NOT NULL,
    unrealized_pnl  DOUBLE PRECISION NOT NULL,
    realized_pnl    DOUBLE PRECISION NOT NULL,
    stop_loss       DOUBLE PRECISION,
    take_profit     DOUBLE PRECISION,
    liquidation_price DOUBLE PRECISION,
    opened_at       TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    closed_at       TIMESTAMPTZ,
    metadata        JSONB NOT NULL DEFAULT '{}',
    UNIQUE (exchange, symbol, side)
);

CREATE INDEX IF NOT EXISTS idx_pe_positions_status ON pe_positions (status);

-- ============================================================
-- Position Engine Orders (pe_orders)
-- 完整映射 CcxtOrder 37 字段，按 client_order_id UPSERT
-- ============================================================

CREATE TABLE IF NOT EXISTS pe_orders (
    -- 订单标识
    client_order_id     TEXT PRIMARY KEY,           -- c  客户端自定义订单ID
    order_id            BIGINT NOT NULL,             -- i  订单ID

    -- 订单基本信息
    symbol              TEXT NOT NULL,               -- s  交易对
    side                TEXT NOT NULL,               -- S  买卖方向 (BUY/SELL)
    order_type           TEXT NOT NULL,               -- o  订单类型
    position_side       TEXT NOT NULL,               -- ps 持仓方向 (LONG/SHORT)
    original_order_type TEXT NOT NULL DEFAULT '',     -- ot 原始订单类型
    status              TEXT NOT NULL,               -- X  订单当前状态
    execution_type      TEXT NOT NULL,               -- x  本次事件执行类型

    -- 价格与数量 (币安返回字符串，保持原样)
    orig_qty            TEXT NOT NULL,               -- q  原始数量
    original_price      TEXT NOT NULL,               -- p  原始价格
    avg_fill_price      TEXT NOT NULL,               -- ap 平均成交价
    filled_qty          TEXT NOT NULL,               -- z  累计已成交量
    last_fill_qty       TEXT NOT NULL,               -- l  末次成交量
    last_fill_price     TEXT NOT NULL,               -- L  末次成交价
    stop_price          TEXT,                        -- sp 条件订单触发价格

    -- 手续费与盈亏
    commission          TEXT NOT NULL DEFAULT '0',   -- n  手续费数量
    commission_asset    TEXT NOT NULL DEFAULT '',    -- N  手续费资产类型
    realized_pnl        TEXT NOT NULL DEFAULT '0',   -- rp 该交易实现盈亏

    -- 订单属性
    reduce_only         BOOLEAN NOT NULL DEFAULT FALSE, -- R 是否仅减仓
    is_maker            BOOLEAN NOT NULL DEFAULT FALSE, -- m 是否为挂单成交
    close_position      BOOLEAN,                     -- cp 是否为触发平仓单
    time_in_force       TEXT NOT NULL DEFAULT 'GTC', -- f  有效方式
    working_type        TEXT NOT NULL DEFAULT '',     -- wt 触发价类型

    -- 名义价值
    bids_notional       TEXT,                        -- b  买单净值
    ask_notional        TEXT,                        -- a  卖单净值

    -- 追踪止损
    activation_price    TEXT,                        -- AP 追踪止损激活价格
    callback_rate       TEXT,                        -- cr 追踪止损回调比例

    -- 价格保护与模式
    price_protection     BOOLEAN NOT NULL DEFAULT FALSE, -- pP 是否开启条件单触发保护
    stp_mode            TEXT,                        -- V  自成交防止模式
    price_match_mode    TEXT,                        -- pm 价格匹配模式
    gtd_auto_cancel_time BIGINT,                     -- gtd TIF为GTD的订单自动取消时间
    expiry_reason       TEXT,                        -- er 过期原因

    -- 忽略字段
    si                  BIGINT NOT NULL DEFAULT 0,   -- si 忽略
    ss                  BIGINT NOT NULL DEFAULT 0,   -- ss 忽略

    -- 时间与成交ID
    trade_time          BIGINT NOT NULL DEFAULT 0,   -- T  成交时间(ms)
    trade_id            BIGINT NOT NULL DEFAULT 0    -- t  成交ID
);

CREATE INDEX IF NOT EXISTS idx_pe_orders_status ON pe_orders (status);
CREATE INDEX IF NOT EXISTS idx_pe_orders_order_id ON pe_orders (order_id);
