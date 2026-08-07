import { createContext, useContext } from 'react';
import { type PositionWsEvent } from '@/service/ws';

export type PositionSide = 'long' | 'short';

export interface PositionsBySide {
  long: PositionWsEvent | null;
  short: PositionWsEvent | null;
}

interface PositionContextType {
  positions: PositionsBySide;

  getPosition: (side: PositionSide) => PositionWsEvent | null;

  hasAnyPosition: () => boolean;
}

const defaultValue: PositionContextType = {
  positions: { long: null, short: null },
  getPosition: () => null,
  hasAnyPosition: () => false,
};

export const PositionContext = createContext<PositionContextType>(defaultValue);

export const usePositionContext = () => {
  const context = useContext(PositionContext);
  if (!context) {
    throw new Error('usePositionContext 必须在 PositionProvider 内部使用');
  }
  return context;
};
