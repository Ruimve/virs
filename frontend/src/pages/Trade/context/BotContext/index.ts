import { createContext, useContext } from 'react';
import { type AutoBot, type GridBot, type GridLevelInfo } from '@/service';

interface BotContextType {
  bot: AutoBot | GridBot | null;
  gridLevels: GridLevelInfo[];
}

export const BotContext = createContext<BotContextType>({
  bot: null,
  gridLevels: [],
});

export const useBot = () => {
  const context = useContext(BotContext);
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用');
  }
  return context;
};
