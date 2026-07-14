import { createWsInstance, useWsHook } from '../../lib/ws';
import type { OrderBookData } from '../types';


export interface OrderBookWsEventRaw {
  exchange: string;
  symbol: string;
  bids: [number, number][];
  asks: [number, number][];
  timestamp: number;
}

export interface OrderBookWsEvent {
  exchange: string;
  symbol: string;
  orderBook: OrderBookData;
  timestamp: number;
}


const orderbookInst = createWsInstance<OrderBookWsEvent>();

function getOrderBookWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws/orderbook`;
}

const parseOrderBookWs = (raw: string): OrderBookWsEvent | null => {
  try {
    const json = JSON.parse(raw) as OrderBookWsEventRaw;
    return {
      exchange: json.exchange,
      symbol: json.symbol,
      orderBook: {
        bids: (json.bids || []).map(([price, amount]) => ({ price, amount })),
        asks: (json.asks || []).map(([price, amount]) => ({ price, amount })),
      },
      timestamp: json.timestamp,
    };
  } catch {
    return null;
  }
};

export function useOrderBookWs(
  onEvent: (event: OrderBookWsEvent) => void,
  onReconnect?: () => void,
): { connected: boolean } {
  return useWsHook(orderbookInst, getOrderBookWsUrl, parseOrderBookWs, onEvent, onReconnect);
}
