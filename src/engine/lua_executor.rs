use crate::models::Kline;
use mlua::{HookTriggers, Lua, Result as LuaResult, Table, Value, VmState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use talib_rs::ma_type::MaType;

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

    fn create_sandbox(&self, klines: &[Kline], idx: usize, params: &HashMap<String, f64>) -> LuaResult<Lua> {
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

        Ok(lua)
    }

    pub fn execute(
        &self,
        code: &str,
        klines: &[Kline],
        idx: usize,
        params: &HashMap<String, f64>,
    ) -> anyhow::Result<i8> {
        if idx >= klines.len() {
            return Ok(0);
        }

        let lua = self.create_sandbox(klines, idx, params)
            .map_err(|e| anyhow::anyhow!("Failed to create Lua sandbox: {}", e))?;

        lua.load(code)
            .exec()
            .map_err(|e| anyhow::anyhow!("Lua syntax error: {}", e))?;

        let signal_func: mlua::Function = lua
            .globals()
            .get("signal")
            .map_err(|e| anyhow::anyhow!("Failed to get signal() function: {}", e))?;

        let signal: i8 = signal_func
            .call(())
            .map_err(|e| anyhow::anyhow!("Lua runtime error in signal(): {}", e))?;

        Ok(signal.clamp(-1, 1))
    }

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

        let lua = self.create_sandbox(klines, 0, params)
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

        for idx in 0..klines.len() {
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

            let signal: i8 = signal_func.call(())
                .map_err(|e| anyhow::anyhow!("Lua runtime error at kline {}: {}", idx, e))?;

            on_signal(signal.clamp(-1, 1));
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
