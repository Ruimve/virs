import { type ReactNode, Suspense, use, useMemo } from 'react';
import {
  getAutoBotDetail,
  getGridBotDetail,
  type AutoBot,
  type GridBot,
  type GridLevelInfo,
} from '@/service';
import { FullScreen } from '@/components/Transition/FullScreen';
import { BotLoading } from '@/components/Transition/Icon';
import { BotContext } from '.';

type Bot = {
  bot: AutoBot | GridBot | null;
  gridLevels: GridLevelInfo[];
};


const fetchBot = (botType: string, botId: string): Promise<Bot> => {
  if (!botId) {

    return Promise.resolve({ bot: null, gridLevels: [] });
  }

  if (botType === 'auto') {
    return getAutoBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: [],
    }));
  } else if (botType === 'grid') {
    return getGridBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: res?.data?.grid_levels || [],
    }));
  } else {

    return Promise.resolve({ bot: null, gridLevels: [] });
  }
};

const promiseBot = (() => {
  const botType = location.pathname?.split('/')[2];
  const botId = location.pathname?.split('/')[3];
  return fetchBot(botType, botId);
})();

export const BotProviderMain = ({ children }: { children: ReactNode }) => {
  const { bot, gridLevels } = use(promiseBot);

  const value = useMemo(() => {
    return {
      bot,
      gridLevels,
    };
  }, [bot, gridLevels]);

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};

export const BotProvider = ({ children }: { children: ReactNode }) => {
  return (
    <Suspense fallback={<FullScreen header icon={<BotLoading />} />}>
      <BotProviderMain>{children}</BotProviderMain>
    </Suspense>
  );
};
