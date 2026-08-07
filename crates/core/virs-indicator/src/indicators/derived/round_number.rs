

pub fn compute_up(price: f64) -> f64 {
    find_round_number(price, true)
}


pub fn compute_down(price: f64) -> f64 {
    find_round_number(price, false)
}


/* 根据价格量级动态选择步长，计算上方/下方最近的整数关口（支撑/阻力位） */
fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    /* 取价格的数量级（如 50000 -> 10000），按量级选择合适的整数关口步长 */
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
    /* upward=true 向上取整得到上方关口，false 向下取整得到下方关口 */
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}
