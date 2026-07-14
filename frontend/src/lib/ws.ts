import { useRef, useEffect, useCallback, useState } from 'react';


export interface WsInstance<T> {
  ws: WebSocket | null;
  listeners: Set<(event: T) => void>;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
  reconnectAttempts: number;
  reconnectCallbacks: Set<() => void>;

  stateChangeCallbacks: Set<() => void>;

  refCount: number;
}

export function createWsInstance<T>(): WsInstance<T> {
  return {
    ws: null,
    listeners: new Set(),
    reconnectTimer: null,
    reconnectAttempts: 0,
    reconnectCallbacks: new Set(),
    stateChangeCallbacks: new Set(),
    refCount: 0,
  };
}

const BASE_RECONNECT_MS = 1000;
const MAX_RECONNECT_MS = 30000;

export function connectWs<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
) {


  if (inst.ws && inst.ws.readyState < WebSocket.CLOSING) return;

  try {
    const ws = new WebSocket(getUrl());

    ws.onopen = () => {
      inst.reconnectAttempts = 0;
      inst.stateChangeCallbacks.forEach((cb) => cb());
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
      inst.stateChangeCallbacks.forEach((cb) => cb());

      if (inst.refCount > 0) {
        const delay = Math.min(
          BASE_RECONNECT_MS * Math.pow(2, inst.reconnectAttempts),
          MAX_RECONNECT_MS,
        );
        inst.reconnectAttempts++;
        inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse), delay);
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
      inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse), delay);
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


  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const onReconnectRef = useRef(onReconnect);
  onReconnectRef.current = onReconnect;


  const stableListener = useCallback((event: T) => {
    onEventRef.current(event);
  }, []);

  const stableReconnect = useCallback(() => {
    onReconnectRef.current?.();
  }, []);


  const [connected, setConnected] = useState(() => inst.ws?.readyState === WebSocket.OPEN);

  const stableStateChange = useCallback(() => {
    setConnected(inst.ws?.readyState === WebSocket.OPEN);
  }, [inst.ws?.readyState]);


  useEffect(() => {
    inst.refCount++;
    inst.listeners.add(stableListener);
    inst.reconnectCallbacks.add(stableReconnect);
    inst.stateChangeCallbacks.add(stableStateChange);


    if (!inst.ws || inst.ws.readyState === WebSocket.CLOSED) {
      inst.reconnectAttempts = 0;
      connectWs(inst, getUrl, parse);
    } else if (inst.ws.readyState === WebSocket.OPEN) {
      setConnected(true);
    }

    return () => {
      inst.listeners.delete(stableListener);
      inst.reconnectCallbacks.delete(stableReconnect);
      inst.stateChangeCallbacks.delete(stableStateChange);
      inst.refCount--;


      if (inst.refCount <= 0) {
        disconnectWs(inst);
        setConnected(false);
      }
    };
  }, []);

  return { connected };
}
