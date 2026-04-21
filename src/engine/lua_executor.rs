use crate::models::Kline;
use mlua::{HookTriggers, Lua, Result as LuaResult, Table, Value, VmState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use talib_rs::ma_type::MaType;

// ---------------------------------------------------------------------------
// Lua state management types
// ---------------------------------------------------------------------------

/// Position state exposed to Lua scripts via `ctx.position`.
#[derive(Debug, Clone, Default)]
pub struct LuaPosition {
    pub side: String,       // "long", "short", or "flat"
    pub entry_price: f64,
    pub size: f64,
}

/// Context injected into Lua sandbox on each bar.
/// Tracks position state and allows explicit order placement.
#[derive(Debug, Clone, Default)]
pub struct LuaContext {
    pub position: LuaPosition,
    pub last_exit_bar: i64,  // bar index when position was last closed
    pub bar_index: i64,      // current bar index
}

/// Result from a Lua script execution.
#[derive(Debug, Clone)]
pub struct LuaScriptResult {
    /// Signal from return value (-1, 0, 1). None if script used explicit orders.
    pub signal: Option<i8>,
    /// Explicit orders placed via buy()/sell()/close().
    pub orders: Vec<LuaOrder>,
}

/// An explicit order placed from Lua.
#[derive(Debug, Clone)]
pub enum LuaOrder {
    Buy { size: f64 },
    Sell { size: f64 },
    Close,
}

// ---------------------------------------------------------------------------
// LuaExecutor
// ---------------------------------------------------------------------------

pub struct LuaExecutorConfig {
    pub instruction_limit: u64,
    pub memory_limit: usize,
}

impl Default for LuaExecutorConfig {
    fn default() -> Self {
        Self {
            instruction_limit: 1_000_000,
            memory_limit: 10 * 1024 * 1024,
        }
    }
}

pub struct LuaExecutor {
    config: LuaExecutorConfig,
}

impl LuaExecutor {
    pub fn new(config: LuaExecutorConfig) -> Self {
        Self { config }
    }

    fn create_sandbox(
        &self,
        klines: &[Kline],
        idx: usize,
        params: &HashMap<String, f64>,
        ctx: &LuaContext,
        orders: Arc<Mutex<Vec<LuaOrder>>>,
    ) -> LuaResult<Lua> {
        let lua = Lua::new();

        lua.set_memory_limit(self.config.memory_limit)
            .map_err(|e| mlua::Error::runtime(format!("Failed to set memory limit: {}", e)))?;

        let instruction_limit = self.config.instruction_limit;
        let count = Arc::new(AtomicU64::new(0));
        lua.set_hook(HookTriggers::new().every_nth_instruction(1000), move |_, _| {
            let n = count.fetch_add(1000, Ordering::Relaxed) + 1000;
            if n > instruction_limit {
                return Err(mlua::Error::runtime("Instruction limit exceeded"));
            }
            Ok(VmState::Continue)
        });

        lua.globals().set("io", Value::Nil)?;
        lua.globals().set("os", Value::Nil)?;
        lua.globals().set("debug", Value::Nil)?;
        lua.globals().set("require", Value::Nil)?;
        lua.globals().set("load", Value::Nil)?;
        lua.globals().set("loadfile", Value::Nil)?;
        lua.globals().set("dofile", Value::Nil)?;
        lua.globals().set("package", Value::Nil)?;
        lua.globals().set("coroutine", Value::Nil)?;

        let kline_table = lua.create_table()?;
        let start = if klines.len() > 200 { klines.len() - 200 } else { 0 };
        let mut row = 1;
        for i in start..=idx {
            let k = &klines[i];
            let entry = lua.create_table()?;
            entry.set("open", k.open)?;
            entry.set("high", k.high)?;
            entry.set("low", k.low)?;
            entry.set("close", k.close)?;
            entry.set("volume", k.volume)?;
            entry.set("time", k.open_time as f64)?;
            kline_table.set(row, entry)?;
            row += 1;
        }
        lua.globals().set("klines", kline_table)?;

        let current_idx = (idx - start + 1) as i64;
        lua.globals().set("current_idx", current_idx)?;

        let params_table = lua.create_table()?;
        for (key, value) in params {
            params_table.set(key.as_str(), *value)?;
        }
        lua.globals().set("params", params_table)?;

        // ---- Inject ctx table ----
        let ctx_table = lua.create_table()?;
        let pos_table = lua.create_table()?;
        pos_table.set("side", ctx.position.side.as_str())?;
        pos_table.set("entry_price", ctx.position.entry_price)?;
        pos_table.set("size", ctx.position.size)?;
        ctx_table.set("position", pos_table)?;
        ctx_table.set("last_exit_bar", ctx.last_exit_bar)?;
        ctx_table.set("bar_index", ctx.bar_index)?;
        lua.globals().set("ctx", ctx_table)?;

        // ---- Technical indicator functions ----

        let sma_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 1 { return Ok(0.0_f64); }
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }
            match talib_rs::overlap::sma(&close, period as usize) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("sma", sma_fn)?;

        let ema_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 1 { return Ok(0.0_f64); }
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }
            match talib_rs::overlap::ema(&close, period as usize) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("ema", ema_fn)?;

        let rsi_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok(50.0_f64); }
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }
            match talib_rs::momentum::rsi(&close, period as usize) {
                Ok(result) => Ok(result.last().copied().unwrap_or(50.0)),
                Err(_) => Ok(50.0),
            }
        })?;
        lua.globals().set("rsi", rsi_fn)?;

        let atr_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            if current_idx < 2 { return Ok(0.0_f64); }

            let n = current_idx as usize;
            let mut high = Vec::with_capacity(n);
            let mut low = Vec::with_capacity(n);
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                high.push(k.get::<f64>("high")?);
                low.push(k.get::<f64>("low")?);
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::volatility::atr(&high, &low, &close, period) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("atr", atr_fn)?;

        let bbands_fn = lua.create_function(|lua, (period, std_dev): (i64, f64)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            let n = current_idx as usize;
            if n < period { return Ok((0.0_f64, 0.0_f64, 0.0_f64)); }

            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::overlap::bbands(&close, period, std_dev, std_dev, MaType::Sma) {
                Ok((upper, mid, lower)) => Ok((
                    upper.last().copied().unwrap_or(0.0),
                    mid.last().copied().unwrap_or(0.0),
                    lower.last().copied().unwrap_or(0.0),
                )),
                Err(_) => Ok((0.0, 0.0, 0.0)),
            }
        })?;
        lua.globals().set("bbands", bbands_fn)?;

        let macd_fn = lua.create_function(|lua, (fast, slow, signal): (i64, i64, i64)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok((0.0_f64, 0.0_f64, 0.0_f64)); }

            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::momentum::macd(&close, fast as usize, slow as usize, signal as usize) {
                Ok((macd, sig, hist)) => Ok((
                    macd.last().copied().unwrap_or(0.0),
                    sig.last().copied().unwrap_or(0.0),
                    hist.last().copied().unwrap_or(0.0),
                )),
                Err(_) => Ok((0.0, 0.0, 0.0)),
            }
        })?;
        lua.globals().set("macd", macd_fn)?;

        let stoch_fn = lua.create_function(|lua, (k_period, d_period): (i64, i64)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok((50.0_f64, 50.0_f64)); }

            let mut high = Vec::with_capacity(n);
            let mut low = Vec::with_capacity(n);
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                high.push(k.get::<f64>("high")?);
                low.push(k.get::<f64>("low")?);
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::momentum::stoch(&high, &low, &close, k_period as usize, d_period as usize, MaType::Sma, d_period as usize, MaType::Sma) {
                Ok((slowk, slowd)) => Ok((
                    slowk.last().copied().unwrap_or(50.0),
                    slowd.last().copied().unwrap_or(50.0),
                )),
                Err(_) => Ok((50.0, 50.0)),
            }
        })?;
        lua.globals().set("stoch", stoch_fn)?;

        let highest_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            let n = current_idx as usize;
            if n < period { return Ok(0.0_f64); }

            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::math_operator::max(&close, period) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("highest", highest_fn)?;

        let lowest_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            let n = current_idx as usize;
            if n < period { return Ok(0.0_f64); }

            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::math_operator::min(&close, period) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("lowest", lowest_fn)?;

        let adx_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok(0.0_f64); }

            let mut high = Vec::with_capacity(n);
            let mut low = Vec::with_capacity(n);
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                high.push(k.get::<f64>("high")?);
                low.push(k.get::<f64>("low")?);
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::momentum::adx(&high, &low, &close, period as usize) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("adx", adx_fn)?;

        let cci_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok(0.0_f64); }

            let mut high = Vec::with_capacity(n);
            let mut low = Vec::with_capacity(n);
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                high.push(k.get::<f64>("high")?);
                low.push(k.get::<f64>("low")?);
                close.push(k.get::<f64>("close")?);
            }

            match talib_rs::momentum::cci(&high, &low, &close, period as usize) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("cci", cci_fn)?;

        let obv_fn = lua.create_function(|lua, (): ()| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok(0.0_f64); }

            let mut close = Vec::with_capacity(n);
            let mut vol = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                close.push(k.get::<f64>("close")?);
                vol.push(k.get::<f64>("volume")?);
            }

            match talib_rs::volume::obv(&close, &vol) {
                Ok(result) => Ok(result.last().copied().unwrap_or(0.0)),
                Err(_) => Ok(0.0),
            }
        })?;
        lua.globals().set("obv", obv_fn)?;

        // ---- Explicit order functions ----

        // buy(size) — size is optional, 0 means use default position sizing
        let orders_buy = orders.clone();
        let buy_fn = lua.create_function(move |_, (size,): (Option<f64>,)| {
            orders_buy.lock().unwrap().push(LuaOrder::Buy { size: size.unwrap_or(0.0) });
            Ok(())
        })?;
        lua.globals().set("buy", buy_fn)?;

        // sell(size) — size is optional, 0 means use default position sizing
        let orders_sell = orders.clone();
        let sell_fn = lua.create_function(move |_, (size,): (Option<f64>,)| {
            orders_sell.lock().unwrap().push(LuaOrder::Sell { size: size.unwrap_or(0.0) });
            Ok(())
        })?;
        lua.globals().set("sell", sell_fn)?;

        // close() — close current position
        let orders_close = orders.clone();
        let close_fn = lua.create_function(move |_, (): ()| {
            orders_close.lock().unwrap().push(LuaOrder::Close);
            Ok(())
        })?;
        lua.globals().set("close", close_fn)?;

        Ok(lua)
    }

    /// Execute a Lua script for a single bar (live trading mode).
    ///
    /// Returns `LuaScriptResult` which may contain either a signal from the
    /// `signal()` function return value, or explicit orders placed via
    /// `buy()`/`sell()`/`close()`.
    pub fn execute(
        &self,
        code: &str,
        klines: &[Kline],
        idx: usize,
        params: &HashMap<String, f64>,
        ctx: &LuaContext,
    ) -> anyhow::Result<LuaScriptResult> {
        if idx >= klines.len() {
            return Ok(LuaScriptResult { signal: Some(0), orders: vec![] });
        }

        let orders = Arc::new(Mutex::new(Vec::new()));
        let lua = self.create_sandbox(klines, idx, params, ctx, orders.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create Lua sandbox: {}", e))?;

        lua.load(code)
            .exec()
            .map_err(|e| anyhow::anyhow!("Lua syntax error: {}", e))?;

        // Try to call signal() — it's optional if using explicit orders
        let signal = match lua.globals().get::<mlua::Function>("signal") {
            Ok(signal_func) => {
                let sig: i8 = signal_func
                    .call(())
                    .map_err(|e| anyhow::anyhow!("Lua runtime error in signal(): {}", e))?;
                Some(sig.clamp(-1, 1))
            }
            Err(_) => None,
        };

        drop(lua);
        let explicit_orders: Vec<LuaOrder> = match Arc::try_unwrap(orders) {
            Ok(mutex) => mutex.into_inner().unwrap_or_default(),
            Err(arc) => arc.lock().unwrap().clone(),
        };

        Ok(LuaScriptResult { signal, orders: explicit_orders })
    }

    /// Execute a Lua script across all klines (backtest mode).
    ///
    /// The `on_signal` callback receives the raw signal (-1, 0, 1) for each bar.
    /// Position state (`ctx`) is tracked internally across bars.
    /// The signature remains backward-compatible.
    pub fn execute_backtest<F>(
        &self,
        code: &str,
        klines: &[Kline],
        params: &HashMap<String, f64>,
        mut on_signal: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(i8),
    {
        if klines.is_empty() {
            return Ok(());
        }

        let orders = Arc::new(Mutex::new(Vec::new()));
        let lua = self.create_sandbox(klines, 0, params, &LuaContext::default(), orders.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create Lua sandbox: {}", e))?;

        lua.load(code)
            .exec()
            .map_err(|e| anyhow::anyhow!("Lua syntax error: {}", e))?;

        let signal_func: mlua::Function = lua
            .globals()
            .get("signal")
            .map_err(|e| anyhow::anyhow!("Failed to get signal() function: {}", e))?;

        let kline_table: Table = lua.globals().get("klines")
            .map_err(|e| anyhow::anyhow!("Failed to get klines table: {}", e))?;

        // Track position state for ctx injection
        let mut ctx = LuaContext::default();

        for idx in 0..klines.len() {
            ctx.bar_index = idx as i64;

            let k = &klines[idx];
            let entry = lua.create_table()
                .map_err(|e| anyhow::anyhow!("Failed to create table: {}", e))?;
            entry.set("open", k.open).map_err(|e| anyhow::anyhow!("{}", e))?;
            entry.set("high", k.high).map_err(|e| anyhow::anyhow!("{}", e))?;
            entry.set("low", k.low).map_err(|e| anyhow::anyhow!("{}", e))?;
            entry.set("close", k.close).map_err(|e| anyhow::anyhow!("{}", e))?;
            entry.set("volume", k.volume).map_err(|e| anyhow::anyhow!("{}", e))?;
            entry.set("time", k.open_time as f64).map_err(|e| anyhow::anyhow!("{}", e))?;

            let row_idx = kline_table.raw_len() + 1;
            kline_table.set(row_idx, entry).map_err(|e| anyhow::anyhow!("{}", e))?;

            lua.globals().set("current_idx", row_idx as i64)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            // Update ctx table in Lua
            let ctx_table: Table = lua.globals().get("ctx").map_err(|e| anyhow::anyhow!("{}", e))?;
            let pos_table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
            pos_table.set("side", ctx.position.side.as_str()).map_err(|e| anyhow::anyhow!("{}", e))?;
            pos_table.set("entry_price", ctx.position.entry_price).map_err(|e| anyhow::anyhow!("{}", e))?;
            pos_table.set("size", ctx.position.size).map_err(|e| anyhow::anyhow!("{}", e))?;
            ctx_table.set("position", pos_table).map_err(|e| anyhow::anyhow!("{}", e))?;
            ctx_table.set("last_exit_bar", ctx.last_exit_bar).map_err(|e| anyhow::anyhow!("{}", e))?;
            ctx_table.set("bar_index", ctx.bar_index).map_err(|e| anyhow::anyhow!("{}", e))?;

            // Clear any explicit orders from previous bar
            orders.lock().unwrap().clear();

            let signal: i8 = signal_func.call(())
                .map_err(|e| anyhow::anyhow!("Lua runtime error at kline {}: {}", idx, e))?;

            let clamped = signal.clamp(-1, 1);

            // Check for explicit orders first
            let explicit_orders: Vec<LuaOrder> = orders.lock().unwrap().clone();
            if !explicit_orders.is_empty() {
                // Process explicit orders — for backtest, convert to signal
                for order in &explicit_orders {
                    match order {
                        LuaOrder::Buy { .. } => {
                            if ctx.position.side == "flat" {
                                ctx.position.side = "long".to_string();
                                ctx.position.entry_price = k.close;
                            }
                            on_signal(1);
                        }
                        LuaOrder::Sell { .. } => {
                            if ctx.position.side == "flat" {
                                ctx.position.side = "short".to_string();
                                ctx.position.entry_price = k.close;
                            }
                            on_signal(-1);
                        }
                        LuaOrder::Close => {
                            if ctx.position.side != "flat" {
                                ctx.position.side = "flat".to_string();
                                ctx.position.entry_price = 0.0;
                                ctx.position.size = 0.0;
                                ctx.last_exit_bar = idx as i64;
                            }
                            // Signal depends on which side we were in
                            // (backtest engine handles direction via trade_direction)
                            on_signal(0);
                        }
                    }
                }
            } else {
                // No explicit orders — use signal() return value
                // Update ctx based on signal (simplified: track position for next bar)
                if clamped == 1 && ctx.position.side == "flat" {
                    ctx.position.side = "long".to_string();
                    ctx.position.entry_price = k.close;
                } else if clamped == -1 && ctx.position.side == "flat" {
                    ctx.position.side = "short".to_string();
                    ctx.position.entry_price = k.close;
                } else if clamped == -1 && ctx.position.side == "long" {
                    ctx.position.side = "flat".to_string();
                    ctx.position.entry_price = 0.0;
                    ctx.position.size = 0.0;
                    ctx.last_exit_bar = idx as i64;
                } else if clamped == 1 && ctx.position.side == "short" {
                    ctx.position.side = "flat".to_string();
                    ctx.position.entry_price = 0.0;
                    ctx.position.size = 0.0;
                    ctx.last_exit_bar = idx as i64;
                }

                on_signal(clamped);
            }
        }

        Ok(())
    }

    pub fn validate(&self, code: &str) -> Result<(), String> {
        let lua = Lua::new();
        lua.globals().set("io", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("os", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("debug", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("require", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("load", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("loadfile", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("dofile", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("package", Value::Nil).map_err(|e| e.to_string())?;
        lua.globals().set("coroutine", Value::Nil).map_err(|e| e.to_string())?;

        lua.load(code)
            .exec()
            .map_err(|e| format!("Lua error: {}", e))?;
        Ok(())
    }
}
