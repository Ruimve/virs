import { createContext, useContext } from 'react';
import { type BotDetail } from '@/service';

export type BotContextType = BotDetail | null;

export const BotContext = createContext<BotContextType>(null);

export const useBot = () => {
  const context = useContext(BotContext);
  if (!context) {
    throw new Error('useBot 必须在 BotProvider 内部使用');
  }
  return context;
};
