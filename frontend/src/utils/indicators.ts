/**
 * 计算简单移动平均线 (SMA)
 */
export function computeSMA(data: number[], period: number): (number | null)[] {
  const result: (number | null)[] = []
  for (let i = 0; i < data.length; i++) {
    if (i < period - 1) {
      result.push(null)
    } else {
      let sum = 0
      for (let j = i - period + 1; j <= i; j++) sum += data[j]
      result.push(sum / period)
    }
  }
  return result
}

/**
 * 计算指数移动平均线 (EMA)
 */
export function computeEMA(data: number[], period: number): (number | null)[] {
  const result: (number | null)[] = []
  const k = 2 / (period + 1)
  let ema: number | null = null
  for (let i = 0; i < data.length; i++) {
    if (i < period - 1) {
      result.push(null)
    } else if (ema === null) {
      // First EMA = SMA of first `period` values
      let sum = 0
      for (let j = i - period + 1; j <= i; j++) sum += data[j]
      ema = sum / period
      result.push(ema)
    } else {
      ema = data[i] * k + ema * (1 - k)
      result.push(ema)
    }
  }
  return result
}

/**
 * 计算布林带 (Bollinger Bands)
 * 返回 { upper, lower, middle } 三条线
 */
export function computeBBands(
  data: number[],
  period: number,
  mult: number,
): { upper: (number | null)[]; lower: (number | null)[]; middle: (number | null)[] } {
  const sma = computeSMA(data, period)
  const upper: (number | null)[] = []
  const lower: (number | null)[] = []

  for (let i = 0; i < data.length; i++) {
    if (sma[i] === null) {
      upper.push(null)
      lower.push(null)
    } else {
      let sumSq = 0
      for (let j = i - period + 1; j <= i; j++) {
        sumSq += (data[j] - sma[i]!) ** 2
      }
      const std = Math.sqrt(sumSq / period)
      upper.push(sma[i]! + mult * std)
      lower.push(sma[i]! - mult * std)
    }
  }

  return { upper, lower, middle: sma }
}
