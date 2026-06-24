import { useRef, useEffect, useCallback } from 'react';
import { createWsInstance, useWsHook } from '../../lib/ws';

// ── Position WebSocket event (aligned with backend PositionEngine) ───
export interface PositionWsEventRaw {
  type: string;
  symbol: string;
  exchange: string;
  side: string;
  status: string;
  size: number;
  entry_price: number;
  current_price: number;
  leverage: number;
  margin: number;
  unrealized_pnl: number;
  realized_pnl: number;
  stop_loss: number | null;
  take_profit: number | null;
  liquidation_price: number | null;
  position_id: string;
  updated_at: string;
}

export interface PositionWsEvent {
  type: string;
  symbol: string;
  exchange: string;
  side: string;
  status: string;
  size: number;
  entryPrice: number;
  currentPrice: number;
  leverage: number;
  margin: number;
  unrealizedPnl: number;
  realizedPnl: number;
  stopLoss: number | null;
  takeProfit: number | null;
  liquidationPrice: number | null;
  positionId: string;
  updatedAt: string;
}

// ── Position WebSocket ────────────────────────────────────
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
      size: json.size,
      entryPrice: json.entry_price,
      currentPrice: json.current_price,
      leverage: json.leverage,
      margin: json.margin,
      unrealizedPnl: json.unrealized_pnl,
      realizedPnl: json.realized_pnl,
      stopLoss: json.stop_loss,
      takeProfit: json.take_profit,
      liquidationPrice: json.liquidation_price,
      positionId: json.position_id,
      updatedAt: json.updated_at,
    };
  } catch {
    return null;
  }
};

/**
 * 订阅指定 symbol 的仓位更新。
 * 连接建立后自动发送 subscribe 消息，组件卸载时发送 unsubscribe。
 */
export function usePositionWs(
  symbol: string | undefined,
  onEvent: (event: PositionWsEvent) => void,
): { connected: boolean } {
  const symbolRef = useRef(symbol);
  symbolRef.current = symbol;

  // 连接建立（或重连）后发送 subscribe
  const handleReconnect = useCallback(() => {
    const sym = symbolRef.current;
    if (sym && positionInst.ws?.readyState === WebSocket.OPEN) {
      positionInst.ws.send(JSON.stringify({ action: 'subscribe', symbol: sym }));
    }
  }, []);

  const result = useWsHook(
    positionInst,
    getPositionWsUrl,
    parsePositionWs,
    onEvent,
    handleReconnect,
  );

  // symbol 变化时：取消旧 symbol 订阅，订阅新 symbol
  useEffect(() => {
    if (!symbol) return;
    // 连接已打开则立即订阅
    if (positionInst.ws?.readyState === WebSocket.OPEN) {
      positionInst.ws.send(JSON.stringify({ action: 'subscribe', symbol }));
    }
    return () => {
      // 卸载或 symbol 变化时取消订阅
      if (positionInst.ws?.readyState === WebSocket.OPEN) {
        positionInst.ws.send(JSON.stringify({ action: 'unsubscribe', symbol }));
      }
    };
  }, [symbol]);

  return result;
}
