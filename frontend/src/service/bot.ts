import { api } from './client';
import type {
  ApiResponse,
  BotSummary,
  ChatBotDetail,
  ChatTradesPage,
  ChatBotStats,
  AnalysisLogsPage,
} from './types';

export async function createChatBot(
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
  return api.post('/chat/create', params, init);
}

export async function startChatBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ started: boolean }>> {
  return api.post(`/chat/${id}/start`, undefined, init);
}

export async function stopChatBot(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<{ stopped: boolean }>> {
  return api.post(`/chat/${id}/stop`, undefined, init);
}

export async function deleteChatBot(id: string, init?: RequestInit): Promise<ApiResponse<null>> {
  return api.del(`/chat/${id}/delete`, init);
}

export async function getChatBotDetail(
  id: string,
  init?: RequestInit,
): Promise<ApiResponse<ChatBotDetail>> {
  return api.get<ChatBotDetail>(`/chat/${id}`, init);
}

export async function getChatTrades(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<ChatTradesPage>> {
  return api.get<ChatTradesPage>(`/chat/${botId}/trades?page=${page}&page_size=${pageSize}`, init);
}

export async function getChatStats(
  botId: string,
  init?: RequestInit,
): Promise<ApiResponse<ChatBotStats>> {
  return api.get<ChatBotStats>(`/chat/${botId}/stats`, init);
}

export async function getChatAnalysisLogs(
  botId: string,
  page: number = 1,
  pageSize: number = 20,
  init?: RequestInit,
): Promise<ApiResponse<AnalysisLogsPage>> {
  return api.get<AnalysisLogsPage>(
    `/chat/${botId}/analysis-logs?page=${page}&page_size=${pageSize}`,
    init,
  );
}

export async function findActiveBot(init?: RequestInit): Promise<BotSummary | null> {
  try {
    const chatRes = await api.get<{ items: Array<{ id: string; status: string }>; total: number }>(
      '/chat/list',
      init,
    );
    if (chatRes.success && chatRes.data?.items?.length) {
      const bot = chatRes.data.items.find((b) => b.status === 'running') || chatRes.data.items[0];
      return { id: bot.id };
    }
    return null;
  } catch {
    return null;
  }
}
