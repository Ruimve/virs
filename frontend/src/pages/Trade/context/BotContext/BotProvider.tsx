import { type ReactNode, Suspense, use, useMemo } from 'react';
import { getAutoBotDetail, type AutoBot, type StrategyDetail } from '@/service';
import { FullScreen } from '@/components/Transition/FullScreen';
import { BotLoading } from '@/components/Transition/Icon';
import { BotContext } from '.';

type Bot = {
  bot: AutoBot | null;
  strategy: StrategyDetail | null;
};

const fetchBot = (botId: string): Promise<Bot> => {
  if (!botId) {
    return Promise.resolve({ bot: null, strategy: null });
  }

  return getAutoBotDetail(botId).then((res) => ({
    bot: res?.data?.bot || null,
    strategy: res?.data?.strategy || null,
  }));
};

const promiseBot = (() => {
  const botId = location.pathname?.split('/')[3];
  return fetchBot(botId);
})();

export const BotProviderMain = ({ children }: { children: ReactNode }) => {
  const { bot, strategy } = use(promiseBot);

  const value = useMemo(() => {
    return {
      bot,
      strategy,
    };
  }, [bot, strategy]);

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};

export const BotProvider = ({ children }: { children: ReactNode }) => {
  return (
    <Suspense fallback={<FullScreen header icon={<BotLoading />} />}>
      <BotProviderMain>{children}</BotProviderMain>
    </Suspense>
  );
};
