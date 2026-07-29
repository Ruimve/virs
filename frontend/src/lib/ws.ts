import { useRef, useEffect, useCallback, useState } from 'react';

/** WebSocket 连接状态，比布尔 `connected` 更细粒度，便于 UI 展示重连中状态 */
export type WsState = 'idle' | 'connecting' | 'open' | 'closed';

export interface WsInstance<T> {
  ws: WebSocket | null;
  listeners: Set<(event: T) => void>;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
  reconnectAttempts: number;
  /** 单一状态回调集合，取代原 reconnectCallbacks + stateChangeCallbacks */
  stateCallbacks: Set<(state: WsState) => void>;
  refCount: number;
}

export function createWsInstance<T>(): WsInstance<T> {
  return {
    ws: null,
    listeners: new Set(),
    reconnectTimer: null,
    reconnectAttempts: 0,
    stateCallbacks: new Set(),
    refCount: 0,
  };
}

const BASE_RECONNECT_MS = 1000;
const MAX_RECONNECT_MS = 30000;
/** 最大重连次数，超过后停止重试并发出 error，避免无限重试 */
const MAX_RECONNECT_ATTEMPTS = 20;

function notifyState<T>(inst: WsInstance<T>, state: WsState) {
  inst.stateCallbacks.forEach((cb) => cb(state));
}

/** 计算指数退避延迟（带 jitter，避免群体同步重连） */
function backoffDelay(attempts: number): number {
  const base = Math.min(BASE_RECONNECT_MS * Math.pow(2, attempts), MAX_RECONNECT_MS);
  return Math.floor(base * (0.75 + Math.random() * 0.5));
}

function scheduleReconnect<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
) {
  if (inst.refCount <= 0) return;
  if (inst.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
    console.error(`[WS] Reached max reconnect attempts (${MAX_RECONNECT_ATTEMPTS}), giving up`);
    notifyState(inst, 'closed');
    return;
  }
  const delay = backoffDelay(inst.reconnectAttempts);
  inst.reconnectAttempts++;
  console.warn(`[WS] Reconnecting in ${delay}ms (attempt ${inst.reconnectAttempts})`);
  inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse), delay);
}

function connectWs<T>(inst: WsInstance<T>, getUrl: () => string, parse: (raw: string) => T | null) {
  // 已有可用连接则跳过
  if (inst.ws && inst.ws.readyState < WebSocket.CLOSING) return;

  notifyState(inst, 'connecting');

  try {
    const ws = new WebSocket(getUrl());

    // 绑定当前 ws 引用，防止旧连接的异步回调污染新连接的共享状态
    const isCurrent = () => inst.ws === ws;

    ws.onopen = () => {
      if (!isCurrent()) return;
      inst.reconnectAttempts = 0;
      notifyState(inst, 'open');
    };

    ws.onmessage = (e) => {
      if (!isCurrent()) return;
      try {
        const event = parse(e.data);
        if (event) inst.listeners.forEach((l) => l(event));
      } catch (err) {
        console.error('[WS] Failed to parse message:', err);
      }
    };

    ws.onclose = () => {
      if (!isCurrent()) return;
      inst.ws = null;
      notifyState(inst, 'closed');
      scheduleReconnect(inst, getUrl, parse);
    };

    ws.onerror = () => {
      if (!isCurrent()) return;
      console.warn('[WS] Connection error');
    };

    inst.ws = ws;
  } catch (err) {
    console.error('[WS] Failed to connect:', err);
    scheduleReconnect(inst, getUrl, parse);
  }
}

function disconnectWs<T>(inst: WsInstance<T>) {
  if (inst.reconnectTimer) {
    clearTimeout(inst.reconnectTimer);
    inst.reconnectTimer = null;
  }
  if (inst.ws) {
    // 先解绑回调，避免 close() 触发的 onclose 改写共享状态
    const ws = inst.ws;
    ws.onopen = null;
    ws.onmessage = null;
    ws.onclose = null;
    ws.onerror = null;
    ws.close();
    inst.ws = null;
  }
  inst.reconnectAttempts = 0;
  notifyState(inst, 'idle');
}

/** 安全发送：未 OPEN 时返回 false 并 warn，不让消费端裸触 inst.ws */
export function sendWs<T>(inst: WsInstance<T>, data: string): boolean {
  if (!inst.ws || inst.ws.readyState !== WebSocket.OPEN) {
    console.warn('[WS] send() called but connection not OPEN, message dropped');
    return false;
  }
  inst.ws.send(data);
  return true;
}

export interface UseWsResult {
  /** 连接是否处于 OPEN 状态，兼容旧消费端 */
  connected: boolean;
  /** 细粒度连接状态，便于 UI 展示重连中 */
  state: WsState;
  /** 安全发送，未 OPEN 时返回 false */
  send: (data: string) => boolean;
}

export function useWs<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
  onEvent: (event: T) => void,
  onReconnect?: () => void,
): UseWsResult {
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const onReconnectRef = useRef(onReconnect);
  onReconnectRef.current = onReconnect;

  const stableListener = useCallback((event: T) => {
    onEventRef.current(event);
  }, []);

  // 关键修复：依赖 [] 而非 inst.ws?.readyState。
  // 函数体内已实时读取 inst.ws，依赖值会随 readyState 变化，导致回调 identity 抖动 →
  // useEffect 重跑 → refCount 清零 disconnectWs → 再 connectWs，连接反复建断。
  const stableStateCallback = useCallback((state: WsState) => {
    if (state === 'open') {
      onReconnectRef.current?.();
    }
    setWsState(state);
  }, []);

  const [wsState, setWsState] = useState<WsState>(() =>
    inst.ws?.readyState === WebSocket.OPEN ? 'open' : 'idle',
  );

  useEffect(() => {
    inst.refCount++;
    inst.listeners.add(stableListener);
    inst.stateCallbacks.add(stableStateCallback);

    if (!inst.ws || inst.ws.readyState === WebSocket.CLOSED) {
      inst.reconnectAttempts = 0;
      connectWs(inst, getUrl, parse);
    } else if (inst.ws.readyState === WebSocket.OPEN) {
      setWsState('open');
      onReconnectRef.current?.();
    }

    return () => {
      inst.listeners.delete(stableListener);
      inst.stateCallbacks.delete(stableStateCallback);
      inst.refCount--;

      if (inst.refCount <= 0) {
        disconnectWs(inst);
      }
    };
  }, [inst, getUrl, parse, stableListener, stableStateCallback]);

  const send = useCallback((data: string) => sendWs(inst, data), [inst]);
  const connected = wsState === 'open';

  return { connected, state: wsState, send };
}
