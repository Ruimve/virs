import { useMemo, useState, type ReactNode } from 'react';
import { usePositionWs, type PositionWsEvent } from '@/service/ws';
import { useBot } from '../BotContext';
import { PositionContext } from '.';

export const PositionProvider = ({ children }: { children: ReactNode }) => {
  const { bot } = useBot();
  const [position, setPosition] = useState<PositionWsEvent | null>(null);

  usePositionWs(bot?.symbol, (event) => {
    if (event.symbol !== bot?.symbol) return;
    // 仓位关闭后清空 state，避免残留 closed 数据
    if (event.status === 'closed' || event.size <= 0) {
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
