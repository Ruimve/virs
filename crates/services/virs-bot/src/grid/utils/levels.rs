//! Grid level generation utilities.

/// Generate grid levels using Gaussian distribution.
/// Returns a vector of (level_index, price) sorted by price ascending.
pub fn generate_gaussian_levels(
    upper_price: f64,
    lower_price: f64,
    grid_count: i32,
) -> Vec<(i32, f64)> {
    if grid_count <= 0 || upper_price <= lower_price {
        return vec![];
    }

    let n = grid_count as f64;
    let mu = (upper_price + lower_price) / 2.0;
    let sigma = (upper_price - lower_price) / 4.0;

    if sigma <= 0.0 {
        return vec![];
    }

    let mut levels = Vec::with_capacity(grid_count as usize);
    for i in 1..=grid_count {
        let p = (i as f64 - 0.5) / n;
        let z = norm_ppf(p);
        let mut price = mu + sigma * z;

        // Clamp to boundaries
        if price > upper_price {
            price = upper_price;
        } else if price < lower_price {
            price = lower_price;
        }

        levels.push((i, price));
    }

    // Deduplicate prices that were clamped to the same boundary value
    levels.dedup_by(|a, b| (a.1 - b.1).abs() < 1e-10);

    // If we lost levels due to dedup, interpolate to fill
    while (levels.len() as i32) < grid_count {
        let mut new_levels = Vec::with_capacity(levels.len() + 1);
        let mut inserted = false;
        for i in 0..levels.len().saturating_sub(1) {
            new_levels.push(levels[i]);
            if !inserted {
                let mid_price = (levels[i].1 + levels[i + 1].1) / 2.0;
                if mid_price > levels[i].1 + 1e-10 && mid_price < levels[i + 1].1 - 1e-10 {
                    new_levels.push((0, mid_price)); // temporary index
                    inserted = true;
                }
            }
        }
        if let Some(&last) = levels.last() {
            new_levels.push(last);
        }
        levels = new_levels;
        if !inserted { break; }
    }

    // Re-index
    levels.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let result: Vec<(i32, f64)> = levels.into_iter().enumerate()
        .map(|(idx, (_, price))| ((idx + 1) as i32, price))
        .collect();

    result
}

/// Approximate inverse normal CDF (Abramowitz and Stegun approximation).
fn norm_ppf(p: f64) -> f64 {
    if p <= 0.0 { return -8.0; }
    if p >= 1.0 { return 8.0; }
    if p < 0.5 {
        return -norm_ppf(1.0 - p);
    }

    // Rational approximation for upper half
    let t = (-2.0 * (p - 1.0).ln()).sqrt();
    let c0 = 2.515517;
    let c1 = 0.802853;
    let c2 = 0.010328;
    let d1 = 1.432788;
    let d2 = 0.189269;
    let d3 = 0.001308;

    t - (c0 + c1 * t + c2 * t * t) / (1.0 + d1 * t + d2 * t * t + d3 * t * t * t)
}

/// Calculate grid profit percentage for a buy-sell pair.
pub fn calc_grid_profit_pct(buy_price: f64, sell_price: f64) -> f64 {
    if buy_price <= 0.0 { return 0.0; }
    (sell_price - buy_price) / buy_price * 100.0
}

/// Find nearest round number as support/resistance.
pub fn find_round_number(price: f64, upward: bool) -> f64 {
    if price <= 0.0 { return 0.0; }
    let magnitude = 10_f64.powf(price.log10().floor());
    let step = if magnitude >= 10000.0 { 1000.0 }
        else if magnitude >= 1000.0 { 100.0 }
        else if magnitude >= 100.0 { 10.0 }
        else if magnitude >= 10.0 { 5.0 }
        else { 1.0 };
    if upward {
        (price / step).ceil() * step
    } else {
        (price / step).floor() * step
    }
}
