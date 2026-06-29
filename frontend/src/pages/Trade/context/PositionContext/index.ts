import { createContext, useContext } from 'react';
import { type PositionWsEvent } from '@/service/ws';

interface PositionContextType {
  position: PositionWsEvent | null;
}

export const PositionContext = createContext<PositionContextType>({
  position: null,
});

export const usePositionContext = () => {
  const context = useContext(PositionContext);
  if (!context) {
    throw new Error('usePositionContext 必须在 PositionProvider 内部使用');
  }
  return context;
};
