import { getBotDetail, getBotList, type BotDetail } from '@/service';

export const findActiveBot = async (): Promise<BotDetail | null> => {
  try {
    const botRes = await getBotList();
    if (botRes.success && botRes.data?.items?.length) {
      const activeBot = botRes.data.items.find((b) => b?.bot?.status === 'running');
      return activeBot ?? null;
    }
    return null;
  } catch {
    return null;
  }
};

export const fetchBot = async (botId?: string): Promise<BotDetail | null> => {
  try {
    if (!botId) {
      return null;
    }

    const result = await getBotDetail({ id: botId });

    if (result.success) {
      return result.data ?? null;
    }

    return null;
  } catch (e) {
    console.error((e as Error)?.message);
    return null;
  }
};
