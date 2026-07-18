use crate::market::Balance;


#[test]
fn m1_1_normal_total() {
    let balance = Balance { asset: "USDT".into(), free: 100.0, used: 50.0, total: 150.0 };
    assert!((balance.compute_total() - 150.0).abs() < 0.01);
}

#[test]
fn m1_2_zero_total() {
    let balance = Balance { asset: "USDT".into(), free: 0.0, used: 0.0, total: 0.0 };
    assert!((balance.compute_total() - 0.0).abs() < 0.01);
}
