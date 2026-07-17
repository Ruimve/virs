import { useRef, useEffect, useCallback } from 'react';
import { createWsInstance, useWs, sendWs } from '../../lib/ws';

export interface PositionWsEventRaw {
  type: string;
  symbol: string;
  exchange: string;
  side: string;
  status: string;
  quantity: number;
  entry_price: number;
  realized_pnl: number;
  stop_loss: number | null;
  take_profit: number | null;
  position_id: string;
  created_at: string;
  updated_at: string;
}

export interface PositionWsEvent {
  type: string;
  symbol: string;
  exchange: string;
  side: string;
  status: string;
  quantity: number;
  entryPrice: number;
  realizedPnl: number;
  stopLoss: number | null;
  takeProfit: number | null;
  positionId: string;
  createdAt: string;
  updatedAt: string;
}

const positionInst = createWsInstance<PositionWsEvent>();

function getPositionWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws/position`;
}

const parsePositionWs = (raw: string): PositionWsEvent | null => {
  try {
    const json = JSON.parse(raw) as PositionWsEventRaw;
    return {
      type: json.type,
      symbol: json.symbol,
      exchange: json.exchange,
      side: json.side,
      status: json.status,
      quantity: json.quantity,
      entryPrice: json.entry_price,
      realizedPnl: json.realized_pnl,
      stopLoss: json.stop_loss,
      takeProfit: json.take_profit,
      positionId: json.position_id,
      createdAt: json.created_at,
      updatedAt: json.updated_at,
    };
  } catch {
    return null;
  }
};

export function usePositionWs(
  symbol: string | undefined,
  onEvent: (event: PositionWsEvent) => void,
): { connected: boolean } {
  const symbolRef = useRef(symbol);
  symbolRef.current = symbol;

  // 重连后自动重新订阅当前 symbol
  const handleReconnect = useCallback(() => {
    const sym = symbolRef.current;
    if (sym) {
      sendWs(positionInst, JSON.stringify({ action: 'subscribe', symbol: sym }));
    }
  }, []);

  const result = useWs(
    positionInst,
    getPositionWsUrl,
    parsePositionWs,
    onEvent,
    handleReconnect,
  );

  useEffect(() => {
    if (!symbol) return;
    sendWs(positionInst, JSON.stringify({ action: 'subscribe', symbol }));
    return () => {
      sendWs(positionInst, JSON.stringify({ action: 'unsubscribe', symbol }));
    };
  }, [symbol]);

  return result;
}
