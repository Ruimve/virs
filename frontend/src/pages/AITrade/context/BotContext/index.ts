import { createContext, useContext } from 'react';
import {
  type AutoBot,
  type AutoTrade,
  type GridBot,
  type GridLevelInfo,
  type GridTrade,
} from '@/service';

interface BotContextType {
  bot: AutoBot | GridBot | null;
  trades: AutoTrade[] | GridTrade[];
  gridLevels: GridLevelInfo[];
  loading: boolean;
}

export const BotContext = createContext<BotContextType>({
  bot: null,
  gridLevels: [],
  loading: false,
  trades: [],
});

export const useBot = () => {
  const context = useContext(BotContext);
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用');
  }
  return context;
};
