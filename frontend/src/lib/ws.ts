import { useRef, useEffect, useCallback, useState } from 'react';

// ── Generic WebSocket infrastructure ───────────────────────

export interface WsInstance<T> {
  ws: WebSocket | null;
  listeners: Set<(event: T) => void>;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
  reconnectAttempts: number;
  reconnectCallbacks: Set<() => void>;
  /** Reference count — how many hooks are currently mounted */
  refCount: number;
}

export function createWsInstance<T>(): WsInstance<T> {
  return {
    ws: null,
    listeners: new Set(),
    reconnectTimer: null,
    reconnectAttempts: 0,
    reconnectCallbacks: new Set(),
    refCount: 0,
  };
}

const BASE_RECONNECT_MS = 1000;
const MAX_RECONNECT_MS = 30000;

export function connectWs<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
  onStateChange: () => void,
) {
  if (inst.ws && inst.ws.readyState === WebSocket.OPEN) return;

  try {
    const ws = new WebSocket(getUrl());

    ws.onopen = () => {
      inst.reconnectAttempts = 0;
      onStateChange();
      inst.reconnectCallbacks.forEach((cb) => cb());
    };

    ws.onmessage = (e) => {
      try {
        const event = parse(e.data);
        if (event) inst.listeners.forEach((l) => l(event));
      } catch (err) {
        console.error('[WS] Failed to parse message:', err);
      }
    };

    ws.onclose = () => {
      inst.ws = null;
      onStateChange();
      // Only auto-reconnect if there are still active consumers
      if (inst.refCount > 0) {
        const delay = Math.min(
          BASE_RECONNECT_MS * Math.pow(2, inst.reconnectAttempts),
          MAX_RECONNECT_MS,
        );
        inst.reconnectAttempts++;
        inst.reconnectTimer = setTimeout(
          () => connectWs(inst, getUrl, parse, onStateChange),
          delay,
        );
      }
    };

    ws.onerror = () => {};

    inst.ws = ws;
  } catch (err) {
    console.error('[WS] Failed to connect:', err);
    if (inst.refCount > 0) {
      const delay = Math.min(
        BASE_RECONNECT_MS * Math.pow(2, inst.reconnectAttempts),
        MAX_RECONNECT_MS,
      );
      inst.reconnectAttempts++;
      inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse, onStateChange), delay);
    }
  }
}

function disconnectWs<T>(inst: WsInstance<T>) {
  if (inst.reconnectTimer) {
    clearTimeout(inst.reconnectTimer);
    inst.reconnectTimer = null;
  }
  if (inst.ws) {
    inst.ws.close();
    inst.ws = null;
  }
  inst.reconnectAttempts = 0;
}

export function useWsHook<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
  onEvent: (event: T) => void,
  onReconnect?: () => void,
): { connected: boolean } {
  // ── Stable refs for callbacks ──────────────────────────

  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const onReconnectRef = useRef(onReconnect);
  onReconnectRef.current = onReconnect;

  // ── Stable callback references (never recreated) ───────

  const stableListener = useCallback((event: T) => {
    onEventRef.current(event);
  }, []);

  const stableReconnect = useCallback(() => {
    onReconnectRef.current?.();
  }, []);

  // ── Reactive connected state ───────────────────────────

  const [connected, setConnected] = useState(() => inst.ws?.readyState === WebSocket.OPEN);

  const onStateChange = useCallback(() => {
    setConnected(inst.ws?.readyState === WebSocket.OPEN);
  }, [inst.ws?.readyState]);

  // ── Mount/unmount lifecycle ────────────────────────────
  // Dependencies are intentionally empty — all inputs are stable refs.
  // getUrl/parse are captured once on mount (they are module-level constants).

  useEffect(() => {
    inst.refCount++;
    inst.listeners.add(stableListener);
    inst.reconnectCallbacks.add(stableReconnect);

    // Connect if not already connected
    if (!inst.ws || inst.ws.readyState === WebSocket.CLOSED) {
      inst.reconnectAttempts = 0;
      connectWs(inst, getUrl, parse, onStateChange);
    } else if (inst.ws.readyState === WebSocket.OPEN) {
      setConnected(true);
    }

    return () => {
      inst.listeners.delete(stableListener);
      inst.reconnectCallbacks.delete(stableReconnect);
      inst.refCount--;

      // Only disconnect when no consumers remain
      if (inst.refCount <= 0) {
        disconnectWs(inst);
        setConnected(false);
      }
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return { connected };
}
