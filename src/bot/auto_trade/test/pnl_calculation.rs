/**
 * 测试盈亏统计更新逻辑（镜像 worker::decide::apply_pending_close 中的统计逻辑）
 * - realized_pnl >= 0: win_trades++, consecutive_losses=0
 * - realized_pnl < 0: loss_trades++, consecutive_losses++
 * - breakeven (pnl=0) 计为 win
 * - total_pnl 累加 realized_pnl
 * - total_trades 每次递增
 */
#[test]
fn stats_update_on_win() {
    let mut total_pnl = 0.0_f64;
    let mut total_trades = 0_i32;
    let mut win_trades = 0_i32;
    let mut loss_trades = 0_i32;
    let mut consecutive_losses = 0_i32;

    let realized_pnl = 5.0_f64;
    total_pnl += realized_pnl;
    total_trades += 1;
    if realized_pnl >= 0.0 {
        win_trades += 1;
        consecutive_losses = 0;
    } else {
        loss_trades += 1;
        consecutive_losses += 1;
    }

    assert!((total_pnl - 5.0_f64).abs() < 0.001);
    assert_eq!(total_trades, 1);
    assert_eq!(win_trades, 1);
    assert_eq!(loss_trades, 0);
    assert_eq!(consecutive_losses, 0);
}

#[test]
fn stats_update_on_loss() {
    let mut total_pnl = 0.0_f64;
    let mut total_trades = 0_i32;
    let mut win_trades = 0_i32;
    let mut loss_trades = 0_i32;
    let mut consecutive_losses = 0_i32;

    let realized_pnl = -3.0_f64;
    total_pnl += realized_pnl;
    total_trades += 1;
    if realized_pnl >= 0.0 {
        win_trades += 1;
        consecutive_losses = 0;
    } else {
        loss_trades += 1;
        consecutive_losses += 1;
    }

    assert!((total_pnl - (-3.0_f64)).abs() < 0.001);
    assert_eq!(total_trades, 1);
    assert_eq!(win_trades, 0);
    assert_eq!(loss_trades, 1);
    assert_eq!(consecutive_losses, 1);
}

#[test]
fn consecutive_losses_resets_on_win() {
    let mut consecutive_losses = 2_i32;
    let realized_pnl = 1.0_f64;
    if realized_pnl >= 0.0 {
        consecutive_losses = 0;
    } else {
        consecutive_losses += 1;
    }
    assert_eq!(consecutive_losses, 0);
}

#[test]
fn consecutive_losses_increments_on_loss() {
    let mut consecutive_losses = 2_i32;
    let realized_pnl = -1.0_f64;
    if realized_pnl >= 0.0 {
        consecutive_losses = 0;
    } else {
        consecutive_losses += 1;
    }
    assert_eq!(consecutive_losses, 3);
}

#[test]
fn breakeven_counts_as_win() {
    let realized_pnl = 0.0_f64;
    assert!(realized_pnl >= 0.0);
}

#[test]
fn total_pnl_accumulates() {
    let mut total_pnl = 0.0_f64;
    total_pnl += 5.0;
    total_pnl += -3.0;
    total_pnl += 2.0;
    assert!((total_pnl - 4.0).abs() < 0.001);
}

#[test]
fn consecutive_losses_tracks_streak() {
    let mut consecutive_losses = 0_i32;
    let trades = [-1.0_f64, -2.0, -3.0, 1.0, -1.0];
    for pnl in trades {
        if pnl >= 0.0 {
            consecutive_losses = 0;
        } else {
            consecutive_losses += 1;
        }
    }
    assert_eq!(consecutive_losses, 1);
}

#[test]
fn win_loss_count_after_multiple_trades() {
    let mut win_trades = 0_i32;
    let mut loss_trades = 0_i32;
    let trades = [5.0_f64, -2.0, 3.0, -1.0, 0.0];
    for pnl in trades {
        if pnl >= 0.0 {
            win_trades += 1;
        } else {
            loss_trades += 1;
        }
    }
    assert_eq!(win_trades, 3);
    assert_eq!(loss_trades, 2);
}
