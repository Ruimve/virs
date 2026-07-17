import { useMemo, useState, type ReactNode } from 'react';
import { usePositionWs, type PositionWsEvent } from '@/service/ws';
import { useBot } from '../BotContext';
import { PositionContext } from '.';

export const PositionProvider = ({ children }: { children: ReactNode }) => {
  const { bot } = useBot();
  const [position, setPosition] = useState<PositionWsEvent | null>(null);

  usePositionWs(bot?.symbol, (event) => {
    if (event.symbol !== bot?.symbol) return;

    if (event.status === 'closed' || event.quantity <= 0) {
      setPosition(null);
      return;
    }
    setPosition(event);
  });

  const value = useMemo(() => {
    return {
      position,
    };
  }, [position]);

  return <PositionContext.Provider value={value}>{children}</PositionContext.Provider>;
};
