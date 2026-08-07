import { useRef, useEffect, useCallback, useState } from 'react';

export type WsState = 'idle' | 'connecting' | 'open' | 'closed';

export interface WsInstance<T> {
  ws: WebSocket | null;
  listeners: Set<(event: T) => void>;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
  reconnectAttempts: number;

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

const MAX_RECONNECT_ATTEMPTS = 20;

function notifyState<T>(inst: WsInstance<T>, state: WsState) {
  inst.stateCallbacks.forEach((cb) => cb(state));
}

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
  if (inst.ws && inst.ws.readyState < WebSocket.CLOSING) return;

  notifyState(inst, 'connecting');

  try {
    const ws = new WebSocket(getUrl());

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

export function sendWs<T>(inst: WsInstance<T>, data: string): boolean {
  if (!inst.ws || inst.ws.readyState !== WebSocket.OPEN) {
    console.warn('[WS] send() called but connection not OPEN, message dropped');
    return false;
  }
  inst.ws.send(data);
  return true;
}

export interface UseWsResult {
  connected: boolean;

  state: WsState;

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
