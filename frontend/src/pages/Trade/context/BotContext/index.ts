import { createContext, useContext } from 'react';
import { type AutoBot, type GridBot, type GridLevelInfo, type StrategyDetail } from '@/service';

interface BotContextType {
  bot: AutoBot | GridBot | null;
  gridLevels: GridLevelInfo[];
  strategy: StrategyDetail | null;
}

export const BotContext = createContext<BotContextType>({
  bot: null,
  gridLevels: [],
  strategy: null,
});

export const useBot = () => {
  const context = useContext(BotContext);
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用');
  }
  return context;
};
