import { type ReactNode, Suspense, use } from 'react';
import {
  getAutoBotDetail,
  getGridBotDetail,
  type AutoBot,
  type GridBot,
  type GridLevelInfo,
} from '@/service';
import FullScreen from '@/components/Transition/FullScreen';
import { Icon } from '@/components/Transition/Icon/BotLoading';
import { BotContext } from '.';

type Bot = {
  bot: AutoBot | GridBot | null;
  gridLevels: GridLevelInfo[];
};

// 数据获取函数，返回 Promise<{ bot, gridLevels? }>
const fetchBot = (botType: string, botId: string): Promise<Bot> => {
  if (!botId) {
    // 没有 botId 时，返回一个空数据（或抛出错误，视业务而定）
    return Promise.resolve({ bot: null, gridLevels: [] });
  }

  if (botType === 'auto') {
    return getAutoBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: [], // auto 类型没有 gridLevels
    }));
  } else if (botType === 'grid') {
    return getGridBotDetail(botId).then((res) => ({
      bot: res?.data?.bot || null,
      gridLevels: res?.data?.grid_levels || [],
    }));
  } else {
    // 未知类型，返回空数据
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

  return <BotContext.Provider value={{ bot, gridLevels }}>{children}</BotContext.Provider>;
};

export const BotProvider = ({ children }: { children: ReactNode }) => {
  return (
    <Suspense fallback={<FullScreen header icon={<Icon />} />}>
      <BotProviderMain>{children}</BotProviderMain>
    </Suspense>
  );
};
