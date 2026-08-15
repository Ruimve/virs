import { getBotDetail, getBotList } from '@/service';
import type { BotContextType } from '.';

export const findActiveBot = async (): Promise<BotContextType> => {
  try {
    const botRes = await getBotList();
    if (botRes.success && botRes.data?.items?.length) {
      const activeBot = botRes.data.items.find((b) => b?.bot?.status === 'running');
      if (activeBot) {
        return { bot: activeBot.bot, strategy: activeBot.strategy };
      }
    }
    return { bot: null, strategy: null };
  } catch {
    return { bot: null, strategy: null };
  }
};

export const fetchBot = async (botId?: string): Promise<BotContextType> => {
  try {
    if (!botId) {
      return findActiveBot();
    }

    return getBotDetail({ id: botId }).then((res) => ({
      bot: res?.data?.bot || null,
      strategy: res?.data?.strategy || null,
    }));
  } catch (e) {
    console.error((e as Error)?.message);
    return Promise.resolve({ bot: null, strategy: null });
  }
};
