-- VIRS - Database Schema
-- PostgreSQL 16+
-- Crypto-only trading platform
--
-- 注意：本文件为全量初始化脚本，会重新创建整个数据库。
-- 所有字段已聚合到初始 CREATE TABLE 中，无增量 ALTER 语句。

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
    market_type TEXT NOT NULL DEFAULT 'perpetual',
    encrypted_api_key TEXT NOT NULL,
    encrypted_api_secret TEXT NOT NULL,
    encrypted_passphrase TEXT,
    label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, exchange, market_type)
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
    market_type TEXT NOT NULL DEFAULT 'perpetual' CHECK (market_type IN ('perpetual', 'spot')),
    paper_mode BOOLEAN NOT NULL DEFAULT true,
    status TEXT NOT NULL DEFAULT 'stopped' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    -- Grid 配置参数
    upper_price DOUBLE PRECISION NOT NULL,
    lower_price DOUBLE PRECISION NOT NULL,
    grid_count INT NOT NULL,
    grid_profit_pct DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    quantity_per_grid DOUBLE PRECISION NOT NULL,
    leverage INT NOT NULL DEFAULT 1,

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
    market_type TEXT NOT NULL DEFAULT 'perpetual' CHECK (market_type IN ('perpetual', 'spot')),
    paper_mode BOOLEAN NOT NULL DEFAULT true,
    status TEXT NOT NULL DEFAULT 'stopped' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    -- 交易参数
    leverage INT NOT NULL DEFAULT 1,
    max_position_pct DOUBLE PRECISION NOT NULL DEFAULT 80.0,
    decide_interval_secs INT NOT NULL DEFAULT 300,

    -- 风控参数
    stop_loss DOUBLE PRECISION NOT NULL DEFAULT 0,
    take_profit DOUBLE PRECISION NOT NULL DEFAULT 0,

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

    -- 触发源与平仓原因
    trigger_source TEXT NOT NULL DEFAULT 'llm' CHECK (trigger_source IN ('llm', 'risk_control')),
    close_reason TEXT CHECK (close_reason IN ('stop_loss', 'take_profit', 'position_timeout', 'llm_decision')),

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
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    system_prompt TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    llm_model TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_analysis_logs_bot ON qd_auto_analysis_logs(bot_id, created_at DESC);
