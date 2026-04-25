-- VIRS - Database Schema
-- PostgreSQL 16+
-- Crypto-only trading platform

-- ============================================================
-- Users & Authentication
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'manager', 'user', 'viewer')),
    email VARCHAR(255),
    is_active BOOLEAN NOT NULL DEFAULT true,
    credits BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_qd_users_username ON qd_users(username);
CREATE INDEX IF NOT EXISTS idx_qd_users_email ON qd_users(email) WHERE email IS NOT NULL;

-- ============================================================
-- Strategies
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_strategies_trading (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    strategy_type VARCHAR(100) NOT NULL DEFAULT 'indicator',
    market_type TEXT NOT NULL DEFAULT 'perpetual',
    symbol VARCHAR(50) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    timeframe VARCHAR(10) NOT NULL DEFAULT '1h',
    strategy_mode TEXT NOT NULL DEFAULT 'signal' CHECK (strategy_mode IN ('signal', 'script')),
    execution_mode TEXT NOT NULL DEFAULT 'signal_only' CHECK (execution_mode IN ('signal_only', 'live')),
    indicator_config JSONB NOT NULL DEFAULT '{}',
    trading_config JSONB NOT NULL DEFAULT '{}',
    exchange_config JSONB NOT NULL DEFAULT '{}',
    notification_config JSONB NOT NULL DEFAULT '{}',
    strategy_code TEXT,
    decide_interval_secs INTEGER NOT NULL DEFAULT 300,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_strategies_user ON qd_strategies_trading(user_id);
CREATE INDEX IF NOT EXISTS idx_strategies_status ON qd_strategies_trading(status);
CREATE INDEX IF NOT EXISTS idx_strategies_symbol ON qd_strategies_trading(symbol, exchange);

-- ============================================================
-- Positions
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_strategy_positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id UUID NOT NULL REFERENCES qd_strategies_trading(id) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('long', 'short')),
    size DOUBLE PRECISION NOT NULL DEFAULT 0,
    entry_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    current_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    leverage DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    stop_loss DOUBLE PRECISION,
    take_profit DOUBLE PRECISION,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_positions_strategy ON qd_strategy_positions(strategy_id);
CREATE INDEX IF NOT EXISTS idx_positions_open ON qd_strategy_positions(strategy_id) WHERE closed_at IS NULL;

-- ============================================================
-- Trades
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_strategy_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id UUID NOT NULL REFERENCES qd_strategies_trading(id) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    trade_type VARCHAR(50) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    fee DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    exchange_order_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trades_strategy ON qd_strategy_trades(strategy_id);
CREATE INDEX IF NOT EXISTS idx_trades_created ON qd_strategy_trades(created_at DESC);

-- ============================================================
-- Pending Orders
-- ============================================================

CREATE TABLE IF NOT EXISTS pending_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id UUID NOT NULL REFERENCES qd_strategies_trading(id) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    signal_type TEXT NOT NULL CHECK (signal_type IN ('open_long', 'close_long', 'open_short', 'close_short')),
    order_type TEXT NOT NULL DEFAULT 'market' CHECK (order_type IN ('market', 'limit', 'stop_market', 'stop_limit')),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    price DOUBLE PRECISION,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'dispatched', 'filled', 'failed', 'canceled')),
    priority INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 10,
    exchange_order_id VARCHAR(255),
    exchange_response JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pending_status ON pending_orders(status, created_at);
CREATE INDEX IF NOT EXISTS idx_pending_strategy ON pending_orders(strategy_id);

-- ============================================================
-- Backtest Results
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_backtest_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES qd_users(id) ON DELETE SET NULL,
    strategy_name VARCHAR(255) NOT NULL DEFAULT '',
    symbol VARCHAR(50) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    timeframe VARCHAR(10) NOT NULL,
    start_date TIMESTAMPTZ NOT NULL,
    end_date TIMESTAMPTZ NOT NULL,
    initial_balance DOUBLE PRECISION NOT NULL,
    final_balance DOUBLE PRECISION NOT NULL,
    total_return_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_drawdown_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    sharpe_ratio DOUBLE PRECISION NOT NULL DEFAULT 0,
    sortino_ratio DOUBLE PRECISION NOT NULL DEFAULT 0,
    win_rate DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades BIGINT NOT NULL DEFAULT 0,
    profit_trades BIGINT NOT NULL DEFAULT 0,
    loss_trades BIGINT NOT NULL DEFAULT 0,
    avg_profit DOUBLE PRECISION NOT NULL DEFAULT 0,
    avg_loss DOUBLE PRECISION NOT NULL DEFAULT 0,
    profit_factor DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_consecutive_wins BIGINT NOT NULL DEFAULT 0,
    max_consecutive_losses BIGINT NOT NULL DEFAULT 0,
    trades_json JSONB,
    equity_curve_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backtest_user ON qd_backtest_results(user_id);
CREATE INDEX IF NOT EXISTS idx_backtest_created ON qd_backtest_results(created_at DESC);

-- ============================================================
-- Notifications
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_strategy_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id UUID NOT NULL REFERENCES qd_strategies_trading(id) ON DELETE CASCADE,
    signal_type TEXT NOT NULL,
    channels TEXT[] NOT NULL DEFAULT '{}',
    message TEXT NOT NULL,
    is_read BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notifications_strategy ON qd_strategy_notifications(strategy_id);
CREATE INDEX IF NOT EXISTS idx_notifications_unread ON qd_strategy_notifications(strategy_id) WHERE NOT is_read;

-- ============================================================
-- Security & Audit
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_login_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identifier VARCHAR(255) NOT NULL,
    identifier_type TEXT NOT NULL DEFAULT 'username' CHECK (identifier_type IN ('username', 'ip')),
    attempt_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    success BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS qd_security_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES qd_users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    ip_address VARCHAR(45),
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_security_logs_user ON qd_security_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_security_logs_created ON qd_security_logs(created_at DESC);

-- ============================================================
-- Credits
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_credits_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    action VARCHAR(100) NOT NULL,
    amount BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    feature VARCHAR(100),
    reference_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credits_user ON qd_credits_log(user_id);

-- ============================================================
-- Exchange Credentials (encrypted)
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_exchange_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    exchange VARCHAR(50) NOT NULL,
    market_type TEXT NOT NULL DEFAULT 'perpetual',
    encrypted_api_key TEXT NOT NULL,
    encrypted_api_secret TEXT NOT NULL,
    encrypted_passphrase TEXT,
    label VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, exchange, market_type)
);

CREATE INDEX IF NOT EXISTS idx_credentials_user ON qd_exchange_credentials(user_id);

-- AI/LLM credentials (per-user, encrypted)
CREATE TABLE IF NOT EXISTS qd_ai_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL,
    encrypted_api_key TEXT NOT NULL,
    label VARCHAR(255),
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_ai_credentials_user ON qd_ai_credentials(user_id);

-- ============================================================
-- Grid Trading Bots
-- ============================================================

-- 网格机器人
CREATE TABLE IF NOT EXISTS qd_grid_bots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    exchange VARCHAR(50) NOT NULL DEFAULT 'binance',
    status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    -- 网格参数
    upper_price DOUBLE PRECISION NOT NULL,
    lower_price DOUBLE PRECISION NOT NULL,
    grid_count INT NOT NULL,
    grid_profit_pct DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    quantity_per_grid DOUBLE PRECISION NOT NULL,
    leverage INT NOT NULL DEFAULT 1,

    -- AI 分析结果
    market_regime VARCHAR(20),
    ai_analysis TEXT,
    system_prompt TEXT,
    user_prompt TEXT,

    -- 动态调整配置
    dynamic_adjust BOOLEAN NOT NULL DEFAULT true,
    adjust_interval_secs INT NOT NULL DEFAULT 300,
    last_adjusted_at TIMESTAMPTZ,

    -- 统计
    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    grid_filled_count INT NOT NULL DEFAULT 0,

    -- 时间戳
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

-- 网格交易记录
CREATE TABLE IF NOT EXISTS qd_grid_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES qd_users(id),
    symbol VARCHAR(50) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL,
    grid_level INT NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    order_id VARCHAR(100),
    status VARCHAR(20) NOT NULL DEFAULT 'filled',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_grid_bots_user ON qd_grid_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_grid_trades_bot ON qd_grid_trades(bot_id);
