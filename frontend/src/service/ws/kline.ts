import { useEffect } from 'react';
import { createWsInstance, useWs, sendWs } from '../../lib/ws';

export interface KlineWsEventRaw {
  exchange: string;
  symbol: string;
  timeframe: string;
  candle: {
    open_time: number;
    close_time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
    quote_volume: number;
    trades: number;
    closed: boolean;
  };
  event_type: 'Update' | 'Closed' | 'Backfilled';
}

export interface KlineWsEvent {
  exchange: string;
  symbol: string;
  timeframe: string;
  candle: {
    close: number;
    high: number;
    low: number;
    open: number;
    volume: number;
    time: number;
  };
  event_type: 'Update' | 'Closed' | 'Backfilled';
}

const klineInst = createWsInstance<KlineWsEvent>();

function getKlineWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws/kline`;
}

const parseKlineWs = (raw: string): KlineWsEvent | null => {
  try {
    const json = JSON.parse(raw) as KlineWsEventRaw;
    return {
      ...json,
      candle: {
        close: json?.candle?.close,
        high: json?.candle?.high,
        low: json?.candle?.low,
        open: json?.candle?.open,
        time: json?.candle?.open_time / 1000,
        volume: json?.candle?.volume,
      },
    };
  } catch {
    return null;
  }
};

export function useKlineWs(
  onEvent: (event: KlineWsEvent) => void,
  onReconnect?: () => void,
  timeframe?: string,
): { connected: boolean } {
  const { connected } = useWs(klineInst, getKlineWsUrl, parseKlineWs, onEvent, onReconnect);

  useEffect(() => {
    if (!connected) return;
    if (timeframe) {
      sendWs(klineInst, JSON.stringify({ action: 'subscribe', timeframe }));
    } else {
      sendWs(klineInst, JSON.stringify({ action: 'unsubscribe' }));
    }
  }, [timeframe, connected]);

  return { connected };
}
