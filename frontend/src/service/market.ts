import { api } from './client';
import type { ApiResponse, KlineCandle, OrderBookData } from './types';

export async function fetchKlines(params: {
  exchange: string;
  symbol: string;
  market_type: string;
  timeframe: string;
}): Promise<ApiResponse<KlineCandle[]>> {
  const { exchange, symbol, market_type, timeframe } = params;
  const res = await api.get<{ candles: KlineCandle[] }>(
    `/market/klines?exchange=${exchange}&symbol=${symbol}&market_type=${market_type}&timeframe=${timeframe}`,
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
  return { success: false, error: res.error || '获取K线失败' };
}

export async function fetchOrderBook(params: {
  exchange: string;
  symbol: string;
  market_type: string;
}): Promise<ApiResponse<OrderBookData>> {
  const { exchange, symbol, market_type } = params;
  const res = await api.get<{ bids: number[][]; asks: number[][] }>(
    `/market/orderbook?exchange=${exchange}&symbol=${symbol}&market_type=${market_type}`,
  );
  if (res.success && res.data) {
    return {
      success: true,
      data: {
        bids: (res.data.bids || []).map(([price, amount]: number[]) => ({ price, amount })),
        asks: (res.data.asks || []).map(([price, amount]: number[]) => ({ price, amount })),
      },
    };
  }
  return { success: false, error: res.error || '获取订单簿失败' };
}

/**
 * 订阅订单簿实时流。
 * 后端重启后，订阅会丢失，前端需要重新订阅以确保 WS 推送恢复。
 */
export async function subscribeOrderBook(params: {
  exchange: string;
  symbol: string;
  market_type: string;
}): Promise<ApiResponse> {
  const res = await api.post('/orderbook/subscribe', {
    exchange: params.exchange,
    symbol: params.symbol,
    market_type: params.market_type,
  });
  return res;
}
