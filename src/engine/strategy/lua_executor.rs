use crate::models::Kline;
use mlua::{HookTriggers, Lua, Result as LuaResult, Table, Value, VmState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use talib_rs::ma_type::MaType;
use talib_rs::TaResult;

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

        // Memory limit may not be available in some environments (e.g. containers)
        if let Err(e) = lua.set_memory_limit(self.config.memory_limit) {
            tracing::warn!("Lua memory limit not available (sandbox still works): {}", e);
        }

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

        // Candlestick pattern recognition
        // Usage: pattern("engulfing") -> number (-100, 0, or 100)
        let pattern_fn = lua.create_function(|lua, (name,): (String,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let n = current_idx as usize;
            if n < 2 { return Ok(0_i32); }

            let mut open = Vec::with_capacity(n);
            let mut high = Vec::with_capacity(n);
            let mut low = Vec::with_capacity(n);
            let mut close = Vec::with_capacity(n);
            for i in 1..=n {
                let k: Table = klines.get(i)?;
                open.push(k.get::<f64>("open")?);
                high.push(k.get::<f64>("high")?);
                low.push(k.get::<f64>("low")?);
                close.push(k.get::<f64>("close")?);
            }

            let result: TaResult<Vec<i32>> = match name.to_lowercase().as_str() {
                "2crows" | "two_crows" => talib_rs::pattern::cdl_2crows(&open, &high, &low, &close),
                "3blackcrows" | "three_black_crows" => talib_rs::pattern::cdl_3blackcrows(&open, &high, &low, &close),
                "3inside" | "three_inside" => talib_rs::pattern::cdl_3inside(&open, &high, &low, &close),
                "3linestrike" | "three_line_strike" => talib_rs::pattern::cdl_3linestrike(&open, &high, &low, &close),
                "3outside" | "three_outside" => talib_rs::pattern::cdl_3outside(&open, &high, &low, &close),
                "3starsinsouth" | "three_stars_in_south" => talib_rs::pattern::cdl_3starsinsouth(&open, &high, &low, &close),
                "3whitesoldiers" | "three_white_soldiers" => talib_rs::pattern::cdl_3whitesoldiers(&open, &high, &low, &close),
                "abandonedbaby" => talib_rs::pattern::cdl_abandonedbaby(&open, &high, &low, &close),
                "advanceblock" | "advance_block" => talib_rs::pattern::cdl_advanceblock(&open, &high, &low, &close),
                "belthold" => talib_rs::pattern::cdl_belthold(&open, &high, &low, &close),
                "breakaway" => talib_rs::pattern::cdl_breakaway(&open, &high, &low, &close),
                "closingmarubozu" => talib_rs::pattern::cdl_closingmarubozu(&open, &high, &low, &close),
                "concealbabyswall" => talib_rs::pattern::cdl_concealbabyswall(&open, &high, &low, &close),
                "counterattack" => talib_rs::pattern::cdl_counterattack(&open, &high, &low, &close),
                "darkcloudcover" | "dark_cloud_cover" => talib_rs::pattern::cdl_darkcloudcover(&open, &high, &low, &close),
                "doji" => talib_rs::pattern::cdl_doji(&open, &high, &low, &close),
                "dojistar" | "doji_star" => talib_rs::pattern::cdl_dojistar(&open, &high, &low, &close),
                "dragonflydoji" | "dragonfly_doji" => talib_rs::pattern::cdl_dragonflydoji(&open, &high, &low, &close),
                "engulfing" => talib_rs::pattern::cdl_engulfing(&open, &high, &low, &close),
                "eveningdojistar" | "evening_doji_star" => talib_rs::pattern::cdl_eveningdojistar(&open, &high, &low, &close),
                "eveningstar" | "evening_star" => talib_rs::pattern::cdl_eveningstar(&open, &high, &low, &close),
                "gapsidesidewhite" => talib_rs::pattern::cdl_gapsidesidewhite(&open, &high, &low, &close),
                "gravestonedoji" | "gravestone_doji" => talib_rs::pattern::cdl_gravestonedoji(&open, &high, &low, &close),
                "hammer" => talib_rs::pattern::cdl_hammer(&open, &high, &low, &close),
                "hangingman" | "hanging_man" => talib_rs::pattern::cdl_hangingman(&open, &high, &low, &close),
                "harami" => talib_rs::pattern::cdl_harami(&open, &high, &low, &close),
                "haramicross" | "harami_cross" => talib_rs::pattern::cdl_haramicross(&open, &high, &low, &close),
                "highwave" | "high_wave" => talib_rs::pattern::cdl_highwave(&open, &high, &low, &close),
                "hikkake" => talib_rs::pattern::cdl_hikkake(&open, &high, &low, &close),
                "hikkakemod" | "hikkake_mod" => talib_rs::pattern::cdl_hikkakemod(&open, &high, &low, &close),
                "homingpigeon" | "homing_pigeon" => talib_rs::pattern::cdl_homingpigeon(&open, &high, &low, &close),
                "identical3crows" | "identical_three_crows" => talib_rs::pattern::cdl_identical3crows(&open, &high, &low, &close),
                "inneck" => talib_rs::pattern::cdl_inneck(&open, &high, &low, &close),
                "invertedhammer" | "inverted_hammer" => talib_rs::pattern::cdl_invertedhammer(&open, &high, &low, &close),
                "kicking" => talib_rs::pattern::cdl_kicking(&open, &high, &low, &close),
                "kickingbylength" => talib_rs::pattern::cdl_kickingbylength(&open, &high, &low, &close),
                "ladderbottom" | "ladder_bottom" => talib_rs::pattern::cdl_ladderbottom(&open, &high, &low, &close),
                "longleggeddoji" | "long_legged_doji" => talib_rs::pattern::cdl_longleggeddoji(&open, &high, &low, &close),
                "longline" | "long_line" => talib_rs::pattern::cdl_longline(&open, &high, &low, &close),
                "marubozu" => talib_rs::pattern::cdl_marubozu(&open, &high, &low, &close),
                "matchinglow" | "matching_low" => talib_rs::pattern::cdl_matchinglow(&open, &high, &low, &close),
                "mathold" => talib_rs::pattern::cdl_mathold(&open, &high, &low, &close),
                "morningdojistar" | "morning_doji_star" => talib_rs::pattern::cdl_morningdojistar(&open, &high, &low, &close),
                "morningstar" | "morning_star" => talib_rs::pattern::cdl_morningstar(&open, &high, &low, &close),
                "onneck" => talib_rs::pattern::cdl_onneck(&open, &high, &low, &close),
                "piercing" => talib_rs::pattern::cdl_piercing(&open, &high, &low, &close),
                "rickshawman" | "rickshaw_man" => talib_rs::pattern::cdl_rickshawman(&open, &high, &low, &close),
                "risefall3methods" | "rise_fall_3_methods" => talib_rs::pattern::cdl_risefall3methods(&open, &high, &low, &close),
                "separatinglines" | "separating_lines" => talib_rs::pattern::cdl_separatinglines(&open, &high, &low, &close),
                "shootingstar" | "shooting_star" => talib_rs::pattern::cdl_shootingstar(&open, &high, &low, &close),
                "shortline" | "short_line" => talib_rs::pattern::cdl_shortline(&open, &high, &low, &close),
                "spinningtop" | "spinning_top" => talib_rs::pattern::cdl_spinningtop(&open, &high, &low, &close),
                "stalledpattern" | "stalled_pattern" => talib_rs::pattern::cdl_stalledpattern(&open, &high, &low, &close),
                "sticksandwich" | "stick_sandwich" => talib_rs::pattern::cdl_sticksandwich(&open, &high, &low, &close),
                "takuri" => talib_rs::pattern::cdl_takuri(&open, &high, &low, &close),
                "tasukigap" => talib_rs::pattern::cdl_tasukigap(&open, &high, &low, &close),
                "thrusting" => talib_rs::pattern::cdl_thrusting(&open, &high, &low, &close),
                "tristar" | "tri_star" => talib_rs::pattern::cdl_tristar(&open, &high, &low, &close),
                "unique3river" | "unique_3_river" => talib_rs::pattern::cdl_unique3river(&open, &high, &low, &close),
                "upsidegap2crows" | "upside_gap_2_crows" => talib_rs::pattern::cdl_upsidegap2crows(&open, &high, &low, &close),
                "xsidegap3methods" | "xside_gap_3_methods" => talib_rs::pattern::cdl_xsidegap3methods(&open, &high, &low, &close),
                _ => {
                    return Ok(0_i32); // Unknown pattern
                }
            };

            match result {
                Ok(values) => Ok(values.last().copied().unwrap_or(0)),
                Err(_) => Ok(0),
            }
        })?;
        lua.globals().set("pattern", pattern_fn)?;

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

    // -----------------------------------------------------------------------
    // Multi-timeframe support
    // -----------------------------------------------------------------------

    /// Create indicator functions for an auxiliary timeframe.
    ///
    /// Each function closure captures only a `String` (the registry key for
    /// the klines table and the idx), which satisfies `Send + 'static`.
    fn create_indicator_functions_for_tf(
        lua: &Lua,
        klines_registry_key: String,
        idx_registry_key: String,
    ) -> LuaResult<Table> {
        let result = lua.create_table()?;

        // ---- sma ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 1 { return Ok(0.0_f64); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::overlap::sma(&close, period as usize) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("sma", f)?;
        }

        // ---- ema ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 1 { return Ok(0.0_f64); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::overlap::ema(&close, period as usize) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("ema", f)?;
        }

        // ---- rsi ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok(50.0_f64); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::momentum::rsi(&close, period as usize) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(50.0)),
                    Err(_) => Ok(50.0),
                }
            })?;
            result.set("rsi", f)?;
        }

        // ---- atr ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let period = period as usize;
                if idx < 2 { return Ok(0.0_f64); }
                let n = idx as usize;
                let mut high = Vec::with_capacity(n);
                let mut low = Vec::with_capacity(n);
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    high.push(k.get::<f64>("high").map_err(|e| mlua::Error::external(e.to_string()))?);
                    low.push(k.get::<f64>("low").map_err(|e| mlua::Error::external(e.to_string()))?);
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::volatility::atr(&high, &low, &close, period) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("atr", f)?;
        }

        // ---- bbands ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period, std_dev): (i64, f64)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let period = period as usize;
                let n = idx as usize;
                if n < period { return Ok((0.0_f64, 0.0_f64, 0.0_f64)); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
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
            result.set("bbands", f)?;
        }

        // ---- macd ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (fast, slow, signal): (i64, i64, i64)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok((0.0_f64, 0.0_f64, 0.0_f64)); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::momentum::macd(&close, fast as usize, slow as usize, signal as usize) {
                    Ok((m, s, h)) => Ok((
                        m.last().copied().unwrap_or(0.0),
                        s.last().copied().unwrap_or(0.0),
                        h.last().copied().unwrap_or(0.0),
                    )),
                    Err(_) => Ok((0.0, 0.0, 0.0)),
                }
            })?;
            result.set("macd", f)?;
        }

        // ---- stoch ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (k_period, d_period): (i64, i64)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok((50.0_f64, 50.0_f64)); }
                let mut high = Vec::with_capacity(n);
                let mut low = Vec::with_capacity(n);
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    high.push(k.get::<f64>("high").map_err(|e| mlua::Error::external(e.to_string()))?);
                    low.push(k.get::<f64>("low").map_err(|e| mlua::Error::external(e.to_string()))?);
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::momentum::stoch(&high, &low, &close, k_period as usize, d_period as usize, MaType::Sma, d_period as usize, MaType::Sma) {
                    Ok((slowk, slowd)) => Ok((
                        slowk.last().copied().unwrap_or(50.0),
                        slowd.last().copied().unwrap_or(50.0),
                    )),
                    Err(_) => Ok((50.0, 50.0)),
                }
            })?;
            result.set("stoch", f)?;
        }

        // ---- highest ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let period = period as usize;
                let n = idx as usize;
                if n < period { return Ok(0.0_f64); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::math_operator::max(&close, period) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("highest", f)?;
        }

        // ---- lowest ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let period = period as usize;
                let n = idx as usize;
                if n < period { return Ok(0.0_f64); }
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::math_operator::min(&close, period) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("lowest", f)?;
        }

        // ---- adx ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok(0.0_f64); }
                let mut high = Vec::with_capacity(n);
                let mut low = Vec::with_capacity(n);
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    high.push(k.get::<f64>("high").map_err(|e| mlua::Error::external(e.to_string()))?);
                    low.push(k.get::<f64>("low").map_err(|e| mlua::Error::external(e.to_string()))?);
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::momentum::adx(&high, &low, &close, period as usize) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("adx", f)?;
        }

        // ---- cci ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (period,): (i64,)| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok(0.0_f64); }
                let mut high = Vec::with_capacity(n);
                let mut low = Vec::with_capacity(n);
                let mut close = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    high.push(k.get::<f64>("high").map_err(|e| mlua::Error::external(e.to_string()))?);
                    low.push(k.get::<f64>("low").map_err(|e| mlua::Error::external(e.to_string()))?);
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::momentum::cci(&high, &low, &close, period as usize) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("cci", f)?;
        }

        // ---- obv ----
        {
            let key = klines_registry_key.clone();
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (): ()| {
                let extra: Table = lua.named_registry_value(&key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                let n = idx as usize;
                if n < 2 { return Ok(0.0_f64); }
                let mut close = Vec::with_capacity(n);
                let mut vol = Vec::with_capacity(n);
                for i in 1..=n {
                    let k: Table = extra.get(i).map_err(|e| mlua::Error::external(e.to_string()))?;
                    close.push(k.get::<f64>("close").map_err(|e| mlua::Error::external(e.to_string()))?);
                    vol.push(k.get::<f64>("volume").map_err(|e| mlua::Error::external(e.to_string()))?);
                }
                match talib_rs::volume::obv(&close, &vol) {
                    Ok(r) => Ok(r.last().copied().unwrap_or(0.0)),
                    Err(_) => Ok(0.0),
                }
            })?;
            result.set("obv", f)?;
        }

        // ---- count (returns current visible bar count for this timeframe) ----
        {
            let idx_key = idx_registry_key.clone();
            let f = lua.create_function(move |lua, (): ()| {
                let idx: i64 = lua.named_registry_value(&idx_key)
                    .map_err(|e| mlua::Error::external(e.to_string()))?;
                Ok(idx)
            })?;
            result.set("count", f)?;
        }

        Ok(result)
    }

    /// Execute a Lua script across all klines with multi-timeframe support.
    ///
    /// The `extra_klines` map contains auxiliary timeframe data keyed by
    /// timeframe string (e.g. `"4h"`, `"1d"`).  For each main bar the
    /// auxiliary `current_idx` is updated so that only bars with
    /// `open_time <= current_main_bar.open_time` are visible.
    pub fn execute_backtest_with_multi_tf<F>(
        &self,
        code: &str,
        klines: &[Kline],
        params: &HashMap<String, f64>,
        extra_klines: &HashMap<String, Vec<Kline>>,
        mut on_signal: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(i8),
    {
        if klines.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "Multi-TF backtest: main bars={}, extra timeframes={:?}",
            klines.len(),
            extra_klines.keys().collect::<Vec<_>>(),
        );

        let orders = Arc::new(Mutex::new(Vec::new()));
        let lua = self.create_sandbox(klines, 0, params, &LuaContext::default(), orders.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create Lua sandbox: {}", e))?;

        // ---- Pre-fill auxiliary klines tables into registry ----
        for (tf_name, tf_klines) in extra_klines {
            let table = lua.create_table()
                .map_err(|e| anyhow::anyhow!("Failed to create table for tf {}: {}", tf_name, e))?;
            for k in tf_klines {
                let entry = lua.create_table()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("open", k.open).map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("high", k.high).map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("low", k.low).map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("close", k.close).map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("volume", k.volume).map_err(|e| anyhow::anyhow!("{}", e))?;
                entry.set("time", k.open_time as f64).map_err(|e| anyhow::anyhow!("{}", e))?;
                table.set(table.raw_len() + 1, entry)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
            let registry_key = format!("extra_klines_{}", tf_name);
            lua.set_named_registry_value(&registry_key, table)
                .map_err(|e| anyhow::anyhow!("Failed to store registry value: {}", e))?;
            // Initialize idx to 0 (no bars visible yet)
            let idx_key = format!("extra_idx_{}", tf_name);
            lua.set_named_registry_value(&idx_key, 0_i64)
                .map_err(|e| anyhow::anyhow!("Failed to store registry value: {}", e))?;
            tracing::info!("Pre-filled auxiliary timeframe '{}' with {} bars", tf_name, tf_klines.len());
        }

        // ---- Create the `tf` global function ----
        // Collect the list of known timeframes (cloned Strings for Send).
        let known_timeframes: Vec<String> = extra_klines.keys().cloned().collect();

        let tf_fn = lua.create_function(move |lua, (tf_name,): (String,)| {
            if !known_timeframes.contains(&tf_name) {
                return Err(mlua::Error::external(format!(
                    "Unknown timeframe: '{}'. Available: {:?}",
                    tf_name, known_timeframes
                )));
            }
            let klines_key = format!("extra_klines_{}", tf_name);
            let idx_key = format!("extra_idx_{}", tf_name);
            Self::create_indicator_functions_for_tf(lua, klines_key, idx_key)
                .map_err(|e| mlua::Error::external(e.to_string()))
        }).map_err(|e| anyhow::anyhow!("Failed to create tf function: {}", e))?;
        lua.globals().set("tf", tf_fn)
            .map_err(|e| anyhow::anyhow!("Failed to set tf function: {}", e))?;

        // ---- Load and validate script ----
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

            // ---- Update auxiliary timeframe indices ----
            let current_time = k.open_time;
            for (tf_name, tf_klines) in extra_klines {
                let mut tf_idx: i64 = 0;
                for (i, tk) in tf_klines.iter().enumerate() {
                    if tk.open_time <= current_time {
                        tf_idx = (i + 1) as i64; // Lua is 1-indexed
                    } else {
                        break;
                    }
                }
                let idx_key = format!("extra_idx_{}", tf_name);
                lua.set_named_registry_value(&idx_key, tf_idx)
                    .map_err(|e| anyhow::anyhow!("Failed to update extra idx: {}", e))?;
            }

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
                            on_signal(0);
                        }
                    }
                }
            } else {
                // No explicit orders — use signal() return value
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
}
