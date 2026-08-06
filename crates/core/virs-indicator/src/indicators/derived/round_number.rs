//! 整数关口（基于价格量级的取整）。

/// 计算向上取整的整数关口。
pub fn compute_up(price: f64) -> f64 {
    find_round_number(price, true)
}

/// 计算向下取整的整数关口。
pub fn compute_down(price: f64) -> f64 {
    find_round_number(price, false)
}

/// 根据价格量级选择步长并取整。
fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    let magnitude = 10_f64.powf(price.log10().floor());
    let step = if magnitude >= 10000.0 {
        1000.0
    } else if magnitude >= 1000.0 {
        100.0
    } else if magnitude >= 100.0 {
        10.0
    } else if magnitude >= 10.0 {
        5.0
    } else {
        1.0
    };
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}
