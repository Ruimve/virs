import { type ReactNode, use, useMemo } from 'react';
import { getBotDetail, type Bot, type StrategyDetail } from '@/service';
import { BotContext } from '.';

type BotData = {
  bot: Bot | null;
  strategy: StrategyDetail | null;
};

const fetchBot = (botId: string): Promise<BotData> => {
  if (!botId) {
    return Promise.resolve({ bot: null, strategy: null });
  }

  return getBotDetail(botId).then((res) => ({
    bot: res?.data?.bot || null,
    strategy: res?.data?.strategy || null,
  }));
};

const promiseBot = (() => {
  const botId = location.pathname?.split('/')[3];
  return fetchBot(botId);
})();

export const BotProvider = ({ children }: { children: ReactNode }) => {
  const { bot, strategy } = use(promiseBot);

  const value = useMemo(() => {
    return {
      bot,
      strategy,
    };
  }, [bot, strategy]);

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};
