-- VIRS - Database Schema
-- PostgreSQL 16+
-- Crypto-only trading platform

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
    credits BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_qd_users_email ON qd_users(email) WHERE email IS NOT NULL;

-- ============================================================
-- Strategies
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_strategies_trading (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    strategy_type TEXT NOT NULL DEFAULT 'indicator',
    market_type TEXT NOT NULL DEFAULT 'perpetual',
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    timeframe TEXT NOT NULL DEFAULT '1h',
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
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('long', 'short')),
    size DOUBLE PRECISION NOT NULL DEFAULT 0,
    entry_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    current_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    leverage INT NOT NULL DEFAULT 1,
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
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    trade_type TEXT NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    fee DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    exchange_order_id TEXT,
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
    symbol TEXT NOT NULL,
    signal_type TEXT NOT NULL CHECK (signal_type IN ('open_long', 'close_long', 'open_short', 'close_short')),
    order_type TEXT NOT NULL DEFAULT 'market' CHECK (order_type IN ('market', 'limit', 'stop_market', 'stop_limit')),
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    price DOUBLE PRECISION,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'dispatched', 'filled', 'failed', 'canceled')),
    priority INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 10,
    exchange_order_id TEXT,
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
    strategy_name TEXT NOT NULL DEFAULT '',
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    timeframe TEXT NOT NULL,
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
    identifier TEXT NOT NULL,
    identifier_type TEXT NOT NULL DEFAULT 'username' CHECK (identifier_type IN ('username', 'ip')),
    ip_address TEXT,
    attempt_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    success BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_login_attempts_ip_time ON qd_login_attempts(ip_address, attempt_time DESC) WHERE ip_address IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_login_attempts_identifier_time ON qd_login_attempts(identifier, attempt_time DESC);

CREATE TABLE IF NOT EXISTS qd_security_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES qd_users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    ip_address TEXT,
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
    action TEXT NOT NULL,
    amount BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    feature TEXT,
    reference_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credits_user ON qd_credits_log(user_id);
CREATE INDEX IF NOT EXISTS idx_credits_created ON qd_credits_log(created_at DESC);

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

-- AI/LLM credentials (per-user, encrypted)
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
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    upper_price DOUBLE PRECISION NOT NULL,
    lower_price DOUBLE PRECISION NOT NULL,
    grid_count INT NOT NULL,
    grid_profit_pct DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    quantity_per_grid DOUBLE PRECISION NOT NULL,
    leverage INT NOT NULL DEFAULT 1,

    market_regime TEXT,
    ai_analysis TEXT,
    grid_levels_json JSONB,
    system_prompt TEXT,
    user_prompt TEXT,

    dynamic_adjust BOOLEAN NOT NULL DEFAULT true,
    adjust_interval_secs INT NOT NULL DEFAULT 300,
    last_adjusted_at TIMESTAMPTZ,

    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    grid_filled_count INT NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_grid_bots_user ON qd_grid_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_grid_bots_status ON qd_grid_bots(status);

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'qd_grid_trades' AND column_name = 'side') THEN
        DROP TABLE IF EXISTS qd_grid_trades CASCADE;
    END IF;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS qd_grid_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    grid_level INT NOT NULL,
    open_side TEXT NOT NULL CHECK (open_side IN ('buy', 'sell')),
    open_price DOUBLE PRECISION NOT NULL,
    open_quantity DOUBLE PRECISION NOT NULL,
    open_order_id TEXT,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    close_side TEXT CHECK (close_side IN ('buy', 'sell')),
    close_price DOUBLE PRECISION,
    close_quantity DOUBLE PRECISION,
    close_order_id TEXT,
    closed_at TIMESTAMPTZ,
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'orphaned')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_grid_trades_bot ON qd_grid_trades(bot_id);
CREATE INDEX IF NOT EXISTS idx_grid_trades_user ON qd_grid_trades(user_id);
CREATE INDEX IF NOT EXISTS idx_grid_trades_status ON qd_grid_trades(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_grid_trades_created ON qd_grid_trades(created_at DESC);

CREATE TABLE IF NOT EXISTS qd_grid_analysis_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    analysis_type TEXT NOT NULL DEFAULT 'periodic',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    system_prompt TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_grid_analysis_logs_bot ON qd_grid_analysis_logs(bot_id, created_at DESC);

-- ============================================================
-- Migrations for existing databases
-- ============================================================

DO $$ BEGIN
    ALTER TABLE qd_login_attempts ADD COLUMN IF NOT EXISTS ip_address TEXT;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_grid_bots ADD COLUMN IF NOT EXISTS grid_levels_json JSONB;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_grid_bots ALTER COLUMN status TYPE TEXT;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_grid_analysis_logs ADD CONSTRAINT chk_analysis_status CHECK (status IN ('pending', 'completed', 'failed'));
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_grid_bots ADD COLUMN IF NOT EXISTS unrealized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS system_prompt TEXT;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS user_prompt TEXT;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_analysis_logs ADD COLUMN IF NOT EXISTS system_prompt TEXT NOT NULL DEFAULT '';
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_analysis_logs ADD COLUMN IF NOT EXISTS user_prompt TEXT NOT NULL DEFAULT '';
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS liquidation_price DOUBLE PRECISION;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_trades ADD COLUMN IF NOT EXISTS trigger_source TEXT NOT NULL DEFAULT 'llm';
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_ai_credentials ADD COLUMN IF NOT EXISTS model TEXT;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

-- ============================================================
-- Auto Trade Bot (全自动交易机器人)
-- ============================================================

CREATE TABLE IF NOT EXISTS qd_auto_bots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL DEFAULT 'binance',
    market_type TEXT NOT NULL DEFAULT 'perpetual' CHECK (market_type IN ('perpetual', 'spot')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'running', 'paused', 'stopped', 'error')),

    leverage INT NOT NULL DEFAULT 1,
    max_position_pct DOUBLE PRECISION NOT NULL DEFAULT 80.0,
    decide_interval_secs INT NOT NULL DEFAULT 300,

    current_side TEXT CHECK (current_side IN ('long', 'short', 'none')),
    entry_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    position_size DOUBLE PRECISION NOT NULL DEFAULT 0,
    stop_loss DOUBLE PRECISION NOT NULL DEFAULT 0,
    take_profit DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    liquidation_price DOUBLE PRECISION,

    market_regime TEXT,
    ai_analysis TEXT,
    system_prompt TEXT,
    user_prompt TEXT,

    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    win_trades INT NOT NULL DEFAULT 0,
    loss_trades INT NOT NULL DEFAULT 0,

    last_decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_bots_user ON qd_auto_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_auto_bots_status ON qd_auto_bots(status);

CREATE TABLE IF NOT EXISTS qd_auto_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_auto_bots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    trade_type TEXT NOT NULL CHECK (trade_type IN ('open_long', 'close_long', 'open_short', 'close_short', 'stop_loss', 'take_profit')),
    trigger_source TEXT NOT NULL DEFAULT 'llm' CHECK (trigger_source IN ('llm', 'risk_control')),
    price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    exchange_order_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auto_trades_bot ON qd_auto_trades(bot_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_auto_trades_user ON qd_auto_trades(user_id);

CREATE TABLE IF NOT EXISTS qd_auto_analysis_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bot_id UUID NOT NULL REFERENCES qd_auto_bots(id) ON DELETE CASCADE,
    analysis_type TEXT NOT NULL DEFAULT 'periodic',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'failed')),
    system_prompt TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}',
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_analysis_logs_bot ON qd_auto_analysis_logs(bot_id, created_at DESC);

-- ============================================================
-- Migrations: Add paper_mode and grid market_type
-- ============================================================

DO $$ BEGIN
    ALTER TABLE qd_grid_bots ADD COLUMN IF NOT EXISTS market_type TEXT NOT NULL DEFAULT 'perpetual' CHECK (market_type IN ('perpetual', 'spot'));
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_grid_bots ADD COLUMN IF NOT EXISTS paper_mode BOOLEAN NOT NULL DEFAULT true;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS paper_mode BOOLEAN NOT NULL DEFAULT true;
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

-- Add llm_model to analysis logs
DO $$ BEGIN
    ALTER TABLE qd_grid_analysis_logs ADD COLUMN IF NOT EXISTS llm_model TEXT NOT NULL DEFAULT '';
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE qd_auto_analysis_logs ADD COLUMN IF NOT EXISTS llm_model TEXT NOT NULL DEFAULT '';
EXCEPTION WHEN OTHERS THEN NULL;
END $$;
