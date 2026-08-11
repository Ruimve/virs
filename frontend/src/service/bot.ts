import { api } from './client';
import type {
  ApiResponse,
  BotSummary,
  AutoBotDetail,
  AutoTradesPage,
  AutoBotStats,
  AnalysisLogsPage,
} from './types';

export async function createAutoBot(
  params: {
    symbol: string;
    exchange: string;
    leverage?: number;
    max_position_pct?: number;
    decide_interval_secs?: number;
    name?: string;
    paper_mode?: boolean;
    auto_optimize?: boolean;
  },
  init?: RequestInit,
): Promise<ApiResponse<{ id: string }>> {
  return api.post('/auto/create', params, init);
}

export async function startAutoBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/auto/${id}/start`, undefined, init);
}

export async function stopAutoBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/auto/${id}/stop`, undefined, init);
}

export async function deleteAutoBot(id: string, init?: RequestInit): Promise<ApiResponse<null>> {
  return api.del(`/auto/${id}/delete`, init);
}

export async function getAutoBotDetail(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<AutoBotDetail>> {
  return api.get<AutoBotDetail>(`/auto/${id}`, init);
}

export async function getAutoTrades(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<AutoTradesPage>> {
  return api.get<AutoTradesPage>(`/auto/${botId}/trades?page=${page}&page_size=${pageSize}`, init);
}

export async function getAutoStats(
  botId: string,
  init?: RequestInit,
): Promise<ApiResponse<AutoBotStats>> {
  return api.get<AutoBotStats>(`/auto/${botId}/stats`, init);
}

export async function getAutoAnalysisLogs(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<AnalysisLogsPage>> {
  return api.get<AnalysisLogsPage>(
    `/auto/${botId}/analysis-logs?page=${page}&page_size=${pageSize}`,
    init,
  );
}

export async function findActiveBot(init?: RequestInit): Promise<BotSummary | null> {
  try {
    const autoRes = await api.get<{ items: Array<{ id: string; status: string }>; total: number }>(
      '/auto/list',
      init,
    );
    if (autoRes.success && autoRes.data?.items?.length) {
      const bot = autoRes.data.items.find((b) => b.status === 'running') || autoRes.data.items[0];
      return { id: bot.id };
    }
    return null;
  } catch {
    return null;
  }
}
