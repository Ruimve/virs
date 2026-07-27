import { type ReactNode, Suspense, use, useMemo } from 'react';
import {
  getAutoBotDetail,
  getGridBotDetail,
  type AutoBot,
  type GridBot,
  type GridLevelInfo,
  type StrategyDetail,
} from '@/service';
import { FullScreen } from '@/components/Transition/FullScreen';
import { BotLoading } from '@/components/Transition/Icon';
import { BotContext } from '.';

type Bot = {
  bot: AutoBot | GridBot | null;
  gridLevels: GridLevelInfo[];
  strategy: StrategyDetail | null;
};

const fetchBot = (botType: string, botId: string): Promise<Bot> => {
  if (!botId) {
    return Promise.resolve({ bot: null, gridLevels: [], strategy: null });
  }

  if (botType === 'auto') {
    return getAutoBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: [],
      strategy: res?.data?.strategy || null,
    }));
  } else if (botType === 'grid') {
    return getGridBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: res?.data?.grid_levels || [],
      strategy: res?.data?.strategy || null,
    }));
  } else {
    return Promise.resolve({ bot: null, gridLevels: [], strategy: null });
  }
};

const promiseBot = (() => {
  const botType = location.pathname?.split('/')[2];
  const botId = location.pathname?.split('/')[3];
  return fetchBot(botType, botId);
})();

export const BotProviderMain = ({ children }: { children: ReactNode }) => {
  const { bot, gridLevels, strategy } = use(promiseBot);

  const value = useMemo(() => {
    return {
      bot,
      gridLevels,
      strategy,
    };
  }, [bot, gridLevels, strategy]);

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};

export const BotProvider = ({ children }: { children: ReactNode }) => {
  return (
    <Suspense fallback={<FullScreen header icon={<BotLoading />} />}>
      <BotProviderMain>{children}</BotProviderMain>
    </Suspense>
  );
};
