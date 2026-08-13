import { api } from './client';
import type {
  ApiResponse,
  BotSummary,
  BotDetail,
  BotTradesPage,
  BotStats,
  AnalysisLogsPage,
} from './types';

export async function createBot(
  params: {
    symbol: string;
    exchange: string;
    bot_type?: 'chat' | 'agent';
    leverage?: number;
    max_position_pct?: number;
    decide_interval_secs?: number;
    name?: string;
    paper_mode?: boolean;
    auto_optimize?: boolean;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/bot/create', params, init);
}

export async function startBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/bot/${id}/start`, undefined, init);
}

export async function stopBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/bot/${id}/stop`, undefined, init);
}

export async function deleteBot(id: string, init?: RequestInit): Promise<ApiResponse<null>> {
  return api.del(`/bot/${id}/delete`, init);
}

export async function getBotDetail(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<BotDetail>> {
  return api.get<BotDetail>(`/bot/${id}`, init);
}

export async function getBotTrades(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<BotTradesPage>> {
  return api.get<BotTradesPage>(`/bot/${botId}/trades?page=${page}&page_size=${pageSize}`, init);
}

export async function getBotStats(
  botId: string,
  init?: RequestInit,
): Promise<ApiResponse<BotStats>> {
  return api.get<BotStats>(`/bot/${botId}/stats`, init);
}

export async function getBotAnalysisLogs(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<AnalysisLogsPage>> {
  return api.get<AnalysisLogsPage>(
    `/bot/${botId}/analysis-logs?page=${page}&page_size=${pageSize}`,
    init,
  );
}

export async function findActiveBot(init?: RequestInit): Promise<BotSummary | null> {
  try {
    const botRes = await api.get<{ items: Array<{ id: string; status: string }>; total: number }>(
      '/bot/list',
      init,
    );
    if (botRes.success && botRes.data?.items?.length) {
      const bot = botRes.data.items.find((b) => b.status === 'running') || botRes.data.items[0];
      return { id: bot.id };
    }
    return null;
  } catch {
    return null;
  }
}
