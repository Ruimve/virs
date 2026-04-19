use crate::models::Kline;
use mlua::{HookTriggers, Lua, Result as LuaResult, Table, Value, VmState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
            let period = period as usize;
            if current_idx < period as i64 {
                return Ok(0.0_f64);
            }
            let mut sum = 0.0;
            for i in (current_idx - period as i64 + 1)..=current_idx {
                let k: Table = klines.get(i)?;
                let close: f64 = k.get("close")?;
                sum += close;
            }
            Ok(sum / period as f64)
        })?;
        lua.globals().set("sma", sma_fn)?;

        let ema_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            if current_idx < 1 {
                return Ok(0.0_f64);
            }
            let k: Table = klines.get(1)?;
            let first_close: f64 = k.get("close")?;
            let mut ema = first_close;
            let multiplier = 2.0 / (period as f64 + 1.0);
            for i in 2..=current_idx {
                let k: Table = klines.get(i)?;
                let close: f64 = k.get("close")?;
                ema = close * multiplier + ema * (1.0 - multiplier);
            }
            Ok(ema)
        })?;
        lua.globals().set("ema", ema_fn)?;

        let rsi_fn = lua.create_function(|lua, (period,): (i64,)| {
            let klines: Table = lua.globals().get("klines")?;
            let current_idx: i64 = lua.globals().get("current_idx")?;
            let period = period as usize;
            if current_idx < (period as i64 + 1) {
                return Ok(50.0_f64);
            }
            let mut gain_sum = 0.0;
            let mut loss_sum = 0.0;
            for i in (current_idx - period as i64)..current_idx {
                let k_prev: Table = klines.get(i)?;
                let k_curr: Table = klines.get(i + 1)?;
                let prev_close: f64 = k_prev.get("close")?;
                let curr_close: f64 = k_curr.get("close")?;
                let change = curr_close - prev_close;
                if change > 0.0 { gain_sum += change; } else { loss_sum += change.abs(); }
            }
            let mut avg_gain = gain_sum / period as f64;
            let mut avg_loss = loss_sum / period as f64;
            let k_prev: Table = klines.get(current_idx - 1)?;
            let k_curr: Table = klines.get(current_idx)?;
            let prev_close: f64 = k_prev.get("close")?;
            let curr_close: f64 = k_curr.get("close")?;
            let change = curr_close - prev_close;
            let gain = if change > 0.0 { change } else { 0.0 };
            let loss = if change < 0.0 { change.abs() } else { 0.0 };
            avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;
            if avg_loss == 0.0 { return Ok(100.0); }
            let rs = avg_gain / avg_loss;
            Ok(100.0 - 100.0 / (1.0 + rs))
        })?;
        lua.globals().set("rsi", rsi_fn)?;

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
