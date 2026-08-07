

pub mod atomic;
pub mod derived;
pub mod primitive;

#[cfg(test)]
pub mod test_utils;

use virs_type::Kline;


/* K 线数据提取辅助函数：从 Kline 切片中提取对应价格序列供 TA-Lib 使用 */

pub fn closes(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.close).collect()
}


pub fn highs(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.high).collect()
}


pub fn lows(klines: &[Kline]) -> Vec<f64> {
    klines.iter().map(|k| k.low).collect()
}
