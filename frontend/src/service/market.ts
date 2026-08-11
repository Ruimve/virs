import { api } from './client';
import type { ApiResponse, KlineCandle } from './types';

export async function fetchKlines(
  params: {
    exchange: string;
    symbol: string;
    timeframe: string;
  },
  init?: RequestInit,
): Promise<ApiResponse<KlineCandle[]>> {
  const { exchange, symbol, timeframe } = params;
  const res = await api.get<{ candles: KlineCandle[] }>(
    `/market/klines?exchange=${exchange}&symbol=${symbol}&timeframe=${timeframe}`,
    init,
  );
  if (res.success && res.data?.candles) {
    return {
      success: true,
      data: res.data.candles.map((c: any) => {
        return {
          close: c.close,
          high: c.high,
          low: c.low,
          open: c.open,
          time: c.open_time / 1000,
          volume: c.volume,
        };
      }),
    };
  }
  return { success: false, message: res.message || '获取K线失败' };
}
