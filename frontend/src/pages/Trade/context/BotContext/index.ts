import { createContext, useContext } from 'react';
import { type AutoBot, type StrategyDetail } from '@/service';

interface BotContextType {
  bot: AutoBot | null;
  strategy: StrategyDetail | null;
}

export const BotContext = createContext<BotContextType>({
  bot: null,
  strategy: null,
});

export const useBot = () => {
  const context = useContext(BotContext);
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用');
  }
  return context;
};
