import { api } from './client';
import type { ApiResponse, BotDetail, BotTradesPage, BotStats, AnalysisLogsPage } from './types';

export async function createBot(
  params?: {},
  bodyParams?: {
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
  return api.post('/bot/create', params, bodyParams, init);
}

export async function startBot(
  params: { id: string },
  bodyParams?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ started: boolean }>> {
  const { id, ...rest } = params;
  return api.post(`/bot/${id}/start`, rest, bodyParams, init);
}

export async function stopBot(
  params: { id: string },
  bodyParams?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ stopped: boolean }>> {
  const { id, ...rest } = params;
  return api.post(`/bot/${id}/stop`, rest, bodyParams, init);
}

export async function deleteBot(
  params: { id: string },
  init?: RequestInit,
): Promise<ApiResponse<null>> {
  const { id, ...rest } = params;
  return api.del(`/bot/${id}/delete`, rest, init);
}

export async function getBotDetail(
  params: { id: string },
  init?: RequestInit,
): Promise<ApiResponse<BotDetail>> {
  const { id, ...rest } = params;
  return api.get<BotDetail>(`/bot/${id}`, rest, init);
}

export async function getBotTrades(
  params: { botId: string; page: number; pageSize: number },
  init?: RequestInit,
): Promise<ApiResponse<BotTradesPage>> {
  return api.get<BotTradesPage>(
    `/bot/${params.botId}/trades`,
    { page: params.page, page_size: params.pageSize },
    init,
  );
}

export async function getBotStats(
  params: { botId: string },
  init?: RequestInit,
): Promise<ApiResponse<BotStats>> {
  const { botId, ...rest } = params;
  return api.get<BotStats>(`/bot/${botId}/stats`, rest, init);
}

export async function getBotAnalysisLogs(
  params: { botId: string; page: number; pageSize: number },
  init?: RequestInit,
): Promise<ApiResponse<AnalysisLogsPage>> {
  return api.get<AnalysisLogsPage>(
    `/bot/${params.botId}/analysis-logs`,
    { page: params.page, page_size: params.pageSize },
    init,
  );
}

export async function getBotList(
  params?: {},
  init?: RequestInit,
): Promise<ApiResponse<{ items: BotDetail[]; total: number }>> {
  return api.get<{ items: BotDetail[]; total: number }>('/bot/list', params, init);
}
