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

    -- 策略 prompt 文件名（STRATEGIES_DIR/grid/{strategy_file}.json）；NULL 时回退到 DEFAULT_* 常量
    strategy_file TEXT,

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

-- ============================================================
-- Position Engine: 订单事件表 + 成交表 (分表方案)
-- pe_order_events: 每笔 WS 事件独立一行 (ALL 事件类型), 全部 41 字段
-- pe_trades: 每笔成交独立一行 (仅 TRADE 事件), 全部 41 字段
-- 两表均无 DEFAULT, 所有值由 INSERT 提供 (来自币安原生推送)
-- ============================================================

-- pe_order_events: 订单事件表 (每笔 WS 事件一行, 含 NEW/TRADE/CANCELED 等)
CREATE TABLE IF NOT EXISTS pe_order_events (
    -- 订单标识
    client_order_id     TEXT NOT NULL,                -- c  客户端自定义订单ID
    order_id            BIGINT NOT NULL,              -- i  订单ID

    -- 订单基本信息
    symbol              TEXT NOT NULL,                -- s  交易对
    side                TEXT NOT NULL,                -- S  买卖方向 (BUY/SELL)
    order_type          TEXT NOT NULL,                -- o  订单类型
    position_side       TEXT NOT NULL,                -- ps 持仓方向 (LONG/SHORT)
    original_order_type TEXT,                         -- ot 原始订单类型 (可能缺失)
    status              TEXT NOT NULL,                -- X  订单当前状态
    execution_type      TEXT NOT NULL,                -- x  本次事件执行类型

    -- 价格与数量 (累计语义, 所有事件有意义)
    orig_qty            TEXT NOT NULL,                -- q  原始数量
    original_price      TEXT NOT NULL,                -- p  原始价格
    avg_fill_price      TEXT,                         -- ap 平均成交价 (NEW 状态时缺失)
    filled_qty          TEXT NOT NULL,                -- z  累计已成交量
    last_fill_qty       TEXT NOT NULL,                -- l  末次成交量 (非 TRADE 事件为 "0", 币安原值)
    last_fill_price     TEXT NOT NULL,                -- L  末次成交价 (非 TRADE 事件为 "0", 币安原值)
    stop_price          TEXT,                         -- sp 条件订单触发价格

    -- 手续费与盈亏 (本笔增量语义)
    commission          TEXT NOT NULL,                -- n  手续费数量 (非 TRADE 事件为 "0", 币安原值)
    commission_asset    TEXT NOT NULL,                -- N  手续费资产类型
    realized_pnl        TEXT,                         -- rp 该交易实现盈亏 (NEW 状态时缺失)

    -- 订单属性
    reduce_only         BOOLEAN NOT NULL,             -- R  是否仅减仓
    is_maker            BOOLEAN NOT NULL,             -- m  是否为挂单成交 (非 TRADE 事件为 false, 币安原值)
    close_position      BOOLEAN,                      -- cp 是否为触发平仓单
    time_in_force       TEXT NOT NULL,                -- f  有效方式
    working_type        TEXT,                         -- wt 触发价类型 (可能缺失)

    -- 名义价值
    bids_notional       TEXT,                         -- b  买单净值
    ask_notional        TEXT,                         -- a  卖单净值

    -- 追踪止损
    activation_price    TEXT,                         -- AP 追踪止损激活价格
    callback_rate       TEXT,                         -- cr 追踪止损回调比例

    -- 价格保护与模式
    price_protection    BOOLEAN,                      -- pP 是否开启条件单触发保护
    stp_mode            TEXT,                         -- V  自成交防止模式
    price_match_mode    TEXT,                         -- pm 价格匹配模式
    gtd_auto_cancel_time BIGINT,                      -- gtd TIF为GTD的订单自动取消时间
    expiry_reason       TEXT,                         -- er 过期原因

    -- 忽略字段
    si                  BIGINT,                       -- si 忽略 (可能缺失)
    ss                  BIGINT,                       -- ss 忽略 (可能缺失)

    -- 时间与成交ID
    trade_time          BIGINT NOT NULL,              -- T  成交时间(ms)
    trade_id            BIGINT NOT NULL,              -- t  成交ID (非 TRADE 事件为 0, 币安原值)

    -- 改单标识 (内层 o.M 字段)
    modify_id           TEXT,                         -- M  改单标识, 仅 AMENDMENT 事件推送

    -- 信封字段 (外层 e/E/T, 非内层 o 对象)
    envelope_event_type      TEXT NOT NULL,           -- e  事件类型 (固定 "ORDER_TRADE_UPDATE")
    envelope_event_time      BIGINT NOT NULL,         -- E  事件时间(ms)
    envelope_transaction_time BIGINT NOT NULL,        -- T  撮合时间(ms)

    -- 复合主键: 同一订单的每个 (execution_type, trade_id) 组合唯一
    PRIMARY KEY (client_order_id, execution_type, trade_id)
);

CREATE INDEX IF NOT EXISTS idx_pe_order_events_status ON pe_order_events (status);
CREATE INDEX IF NOT EXISTS idx_pe_order_events_order_id ON pe_order_events (order_id);
CREATE INDEX IF NOT EXISTS idx_pe_order_events_cid ON pe_order_events (client_order_id);

-- pe_trades: 成交表 (每笔成交一行, 仅 TRADE 事件), 全部 41 字段
CREATE TABLE IF NOT EXISTS pe_trades (
    -- 订单标识
    client_order_id     TEXT NOT NULL,                -- c  客户端自定义订单ID
    order_id            BIGINT NOT NULL,              -- i  订单ID

    -- 订单基本信息
    symbol              TEXT NOT NULL,                -- s  交易对
    side                TEXT NOT NULL,                -- S  买卖方向 (BUY/SELL)
    order_type          TEXT NOT NULL,                -- o  订单类型
    position_side       TEXT NOT NULL,                -- ps 持仓方向 (LONG/SHORT)
    original_order_type TEXT,                         -- ot 原始订单类型 (可能缺失)
    status              TEXT NOT NULL,                -- X  订单当前状态
    execution_type      TEXT NOT NULL,                -- x  始终为 'TRADE'

    -- 价格与数量 (累计语义)
    orig_qty            TEXT NOT NULL,                -- q  原始数量
    original_price      TEXT NOT NULL,                -- p  原始价格
    avg_fill_price      TEXT,                         -- ap 平均成交价
    filled_qty          TEXT NOT NULL,                -- z  累计已成交量
    last_fill_qty       TEXT NOT NULL,                -- l  本笔成交量 (必有实际值)
    last_fill_price     TEXT NOT NULL,                -- L  本笔成交价 (必有实际值)
    stop_price          TEXT,                         -- sp 条件订单触发价格

    -- 手续费与盈亏 (本笔增量, 必有实际值)
    commission          TEXT NOT NULL,                -- n  本笔手续费
    commission_asset    TEXT NOT NULL,                -- N  手续费资产类型
    realized_pnl        TEXT,                         -- rp 本笔实现盈亏 (开仓时可能为 NULL/"0")

    -- 订单属性
    reduce_only         BOOLEAN NOT NULL,             -- R  是否仅减仓
    is_maker            BOOLEAN NOT NULL,             -- m  是否为挂单成交
    close_position      BOOLEAN,                      -- cp 是否为触发平仓单
    time_in_force       TEXT NOT NULL,                -- f  有效方式
    working_type        TEXT,                         -- wt 触发价类型

    -- 名义价值
    bids_notional       TEXT,                         -- b  买单净值
    ask_notional        TEXT,                         -- a  卖单净值

    -- 追踪止损
    activation_price    TEXT,                         -- AP 追踪止损激活价格
    callback_rate       TEXT,                         -- cr 追踪止损回调比例

    -- 价格保护与模式
    price_protection    BOOLEAN,                      -- pP 是否开启条件单触发保护
    stp_mode            TEXT,                         -- V  自成交防止模式
    price_match_mode    TEXT,                         -- pm 价格匹配模式
    gtd_auto_cancel_time BIGINT,                      -- gtd TIF为GTD的订单自动取消时间
    expiry_reason       TEXT,                         -- er 过期原因

    -- 忽略字段
    si                  BIGINT,                       -- si 忽略
    ss                  BIGINT,                       -- ss 忽略

    -- 时间与成交ID
    trade_time          BIGINT NOT NULL,              -- T  成交时间(ms)
    trade_id            BIGINT NOT NULL,              -- t  成交ID (必有实际值 >0)

    -- 改单标识 (内层 o.M 字段)
    modify_id           TEXT,                         -- M  改单标识

    -- 信封字段 (外层 e/E/T)
    envelope_event_type      TEXT NOT NULL,           -- e  事件类型
    envelope_event_time      BIGINT NOT NULL,         -- E  事件时间(ms)
    envelope_transaction_time BIGINT NOT NULL,        -- T  撮合时间(ms)

    -- 复合主键: 同一订单的每笔成交唯一
    PRIMARY KEY (client_order_id, trade_id)
);

CREATE INDEX IF NOT EXISTS idx_pe_trades_symbol_ps ON pe_trades (symbol, position_side, trade_time);
CREATE INDEX IF NOT EXISTS idx_pe_trades_cid ON pe_trades (client_order_id);

-- pe_rejected_orders: 被 validate 拦截的非法订单 (side/position_side/status 非法)
-- 41 字段与 pe_order_events/pe_trades 一致 + rejection_reason 拒绝原因
CREATE TABLE IF NOT EXISTS pe_rejected_orders (
    -- 订单标识
    client_order_id     TEXT NOT NULL,
    order_id            BIGINT NOT NULL,

    -- 订单基本信息
    symbol              TEXT NOT NULL,
    side                TEXT NOT NULL,
    order_type          TEXT NOT NULL,
    position_side       TEXT NOT NULL,
    original_order_type TEXT,
    status              TEXT NOT NULL,
    execution_type      TEXT NOT NULL,

    -- 价格与数量
    orig_qty            TEXT NOT NULL,
    original_price      TEXT NOT NULL,
    avg_fill_price      TEXT,
    filled_qty          TEXT NOT NULL,
    last_fill_qty       TEXT NOT NULL,
    last_fill_price     TEXT NOT NULL,
    stop_price          TEXT,

    -- 手续费与盈亏
    commission          TEXT NOT NULL,
    commission_asset    TEXT NOT NULL,
    realized_pnl        TEXT,

    -- 订单属性
    reduce_only         BOOLEAN NOT NULL,
    is_maker            BOOLEAN NOT NULL,
    close_position      BOOLEAN,
    time_in_force       TEXT NOT NULL,
    working_type        TEXT,

    -- 名义价值
    bids_notional       TEXT,
    ask_notional        TEXT,

    -- 追踪止损
    activation_price    TEXT,
    callback_rate       TEXT,

    -- 价格保护与模式
    price_protection    BOOLEAN,
    stp_mode            TEXT,
    price_match_mode    TEXT,
    gtd_auto_cancel_time BIGINT,
    expiry_reason       TEXT,

    -- 忽略字段
    si                  BIGINT,
    ss                  BIGINT,

    -- 时间与成交ID
    trade_time          BIGINT NOT NULL,
    trade_id            BIGINT NOT NULL,

    -- 改单标识
    modify_id           TEXT,

    -- 信封字段
    envelope_event_type      TEXT NOT NULL,
    envelope_event_time      BIGINT NOT NULL,
    envelope_transaction_time BIGINT NOT NULL,

    -- 拒绝原因
    rejection_reason    TEXT NOT NULL,

    -- 复合主键: 同一订单同一事件只存一条
    PRIMARY KEY (client_order_id, execution_type, trade_id)
);

CREATE INDEX IF NOT EXISTS idx_pe_rejected_orders_cid ON pe_rejected_orders (client_order_id);

-- pe_order_latest 视图: UNION ALL 合并两表取最新行, commission/realized_pnl 从 pe_trades 聚合
-- pe_order_events 存非 TRADE 事件, pe_trades 存 TRADE 事件, 两表数据互补无冗余
CREATE OR REPLACE VIEW pe_order_latest AS
SELECT
    latest.client_order_id,
    latest.order_id,
    latest.symbol,
    latest.side,
    latest.order_type,
    latest.position_side,
    latest.original_order_type,
    latest.status,
    latest.execution_type,
    latest.orig_qty,
    latest.original_price,
    latest.avg_fill_price,
    latest.filled_qty,
    latest.last_fill_qty,
    latest.last_fill_price,
    latest.stop_price,
    COALESCE(agg.commission_sum, 0)::TEXT AS commission,
    latest.commission_asset,
    COALESCE(agg.rp_sum, 0)::TEXT   AS realized_pnl,
    latest.reduce_only,
    latest.is_maker,
    latest.close_position,
    latest.time_in_force,
    latest.working_type,
    latest.bids_notional,
    latest.ask_notional,
    latest.activation_price,
    latest.callback_rate,
    latest.price_protection,
    latest.stp_mode,
    latest.price_match_mode,
    latest.gtd_auto_cancel_time,
    latest.expiry_reason,
    latest.si,
    latest.ss,
    latest.trade_time,
    latest.trade_id,
    latest.modify_id,
    latest.envelope_event_type,
    latest.envelope_event_time,
    latest.envelope_transaction_time
FROM (
    SELECT DISTINCT ON (client_order_id) *
    FROM (
        SELECT * FROM pe_order_events
        UNION ALL
        SELECT * FROM pe_trades
    ) combined
    ORDER BY client_order_id, trade_time DESC, trade_id DESC
) latest
LEFT JOIN (
    SELECT client_order_id,
           SUM(commission::float8) AS commission_sum,
           SUM(COALESCE(realized_pnl::float8, 0)) AS rp_sum
    FROM pe_trades
    GROUP BY client_order_id
) agg ON agg.client_order_id = latest.client_order_id;

-- Grid Order Context（业务上下文，价格/数量/pnl 从 pe_order_latest 取）
CREATE TABLE IF NOT EXISTS pe_grid_order_context (
    client_order_id        TEXT PRIMARY KEY,
    bot_id                 UUID NOT NULL REFERENCES qd_grid_bots(id) ON DELETE CASCADE,
    user_id                UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol                 TEXT NOT NULL,
    exchange               TEXT NOT NULL,
    grid_level             INT NOT NULL,

    -- 订单角色: open=开仓, close=平仓
    order_role             TEXT NOT NULL CHECK (order_role IN ('open', 'close')),

    -- 配对状态: open=持仓中, closed=已平仓, orphaned=孤儿记录
    status                 TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'orphaned')),

    -- 配对关联: close 行指向 open 行的 client_order_id
    paired_client_order_id TEXT REFERENCES pe_grid_order_context(client_order_id),

    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_grid_ctx_bot ON pe_grid_order_context(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_grid_ctx_pair ON pe_grid_order_context(paired_client_order_id);
CREATE INDEX IF NOT EXISTS idx_grid_ctx_level ON pe_grid_order_context(bot_id, grid_level, status);

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

    -- 策略 prompt 文件名（STRATEGIES_DIR/auto/{strategy_file}.json）；NULL 时回退到 DEFAULT_* 常量
    strategy_file TEXT,

    -- 统计缓存（denormalized，由 worker 定期同步）
    total_pnl DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades INT NOT NULL DEFAULT 0,
    win_trades INT NOT NULL DEFAULT 0,
    loss_trades INT NOT NULL DEFAULT 0,

    -- 仓位追踪（per-side，Hedge 模式支持多空并存；跨系统引用 PositionEngine 内存中的 Position.id）
    position_id_long UUID,
    position_id_short UUID,

    -- 生命周期时间戳（内部字段，API 不返回）
    last_decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_bots_user ON qd_auto_bots(user_id);
CREATE INDEX IF NOT EXISTS idx_auto_bots_status ON qd_auto_bots(status);

-- Auto Order Context（业务上下文，价格/数量/pnl 从 pe_order_latest 取）
CREATE TABLE IF NOT EXISTS pe_auto_order_context (
    client_order_id        TEXT PRIMARY KEY,
    bot_id                 UUID NOT NULL REFERENCES qd_auto_bots(id) ON DELETE CASCADE,
    user_id                UUID NOT NULL REFERENCES qd_users(id) ON DELETE CASCADE,
    symbol                 TEXT NOT NULL,
    exchange               TEXT NOT NULL,

    -- 订单角色: open=开仓, close=平仓
    order_role             TEXT NOT NULL CHECK (order_role IN ('open', 'close')),

    -- 配对状态: open=持仓中, closed=已平仓, orphaned=孤儿记录
    status                 TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'orphaned')),

    -- 配对关联: close 行指向 open 行的 client_order_id
    paired_client_order_id TEXT REFERENCES pe_auto_order_context(client_order_id),

    -- Auto 专属字段
    stop_loss              DOUBLE PRECISION NOT NULL DEFAULT 0,
    take_profit            DOUBLE PRECISION NOT NULL DEFAULT 0,
    close_reason           TEXT CHECK (close_reason IS NULL OR close_reason IN ('stop_loss', 'take_profit', 'position_timeout', 'llm_decision')),

    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auto_ctx_bot ON pe_auto_order_context(bot_id, status);
CREATE INDEX IF NOT EXISTS idx_auto_ctx_pair ON pe_auto_order_context(paired_client_order_id);

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
    -- 注：close_reason 不在此表，已记录在 pe_auto_order_context.close_reason
    intercept_reason TEXT,
    execution_status TEXT CHECK (execution_status IS NULL OR execution_status IN ('open', 'open_failed', 'close', 'close_failed', 'hold')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auto_analysis_logs_bot ON qd_auto_analysis_logs(bot_id, created_at DESC);

