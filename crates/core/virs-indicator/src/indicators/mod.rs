

pub mod atomic;
pub mod derived;
pub mod primitive;

#[cfg(test)]
pub mod test_utils;

use virs_type::Kline;


pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}


pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}


pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}
