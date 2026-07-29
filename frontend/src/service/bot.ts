import { api } from './client';
import type {
  ApiResponse,
  BotSummary,
  AutoBotDetail,
  AutoTradesPage,
  AutoBotStats,
  AnalysisLogsPage,
} from './types';

export async function createAutoBot(params: {
  symbol: string;
  exchange: string;
  leverage?: number;
  decide_interval_secs?: number;
  name?: string;
  paper_mode?: boolean;
}): Promise<ApiResponse<{ id: string }>> {
  return api.post('/auto/create', params);
}

export async function startAutoBot(id: string): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/auto/${id}/start`);
}

export async function stopAutoBot(id: string): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/auto/${id}/stop`);
}

export async function deleteAutoBot(id: string): Promise<ApiResponse<null>> {
  return api.del(`/auto/${id}/delete`);
}

export async function getAutoBotDetail(id: string): Promise<ApiResponse<AutoBotDetail>> {
  return api.get<AutoBotDetail>(`/auto/${id}`);
}

export async function getAutoTrades(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
): Promise<ApiResponse<AutoTradesPage>> {
  return api.get<AutoTradesPage>(`/auto/${botId}/trades?page=${page}&page_size=${pageSize}`);
}

export async function getAutoStats(botId: string): Promise<ApiResponse<AutoBotStats>> {
  return api.get<AutoBotStats>(`/auto/${botId}/stats`);
}

export async function getAutoAnalysisLogs(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
): Promise<ApiResponse<AnalysisLogsPage>> {
  return api.get<AnalysisLogsPage>(
    `/auto/${botId}/analysis-logs?page=${page}&page_size=${pageSize}`,
  );
}

export async function findActiveBot(): Promise<BotSummary | null> {
  try {
    const autoRes = await api.get<{ items: Array<{ id: string; status: string }>; total: number }>(
      '/auto/list',
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
