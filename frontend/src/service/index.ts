// Service layer — unified exports
export { api, getToken, setToken, removeToken } from './client';
export { login, logout, getUserInfo } from './auth';
export { fetchPlugins, validateScript, getAiStatus, generateStrategy } from './ai';
export {
  saveCredential,
  saveAiCredential,
  testCredential,
  checkPermissions,
  verifyPermissions,
  fetchCredentialStatus,
  fetchAiModels,
  fetchAiBalance,
  testAiCredential,
} from './credentials';
export {
  createGridBot,
  startGridBot,
  stopGridBot,
  deleteGridBot,
  getGridBotDetail,
  getGridTrades,
  getGridStats,
  getGridAnalysisLogs,
  createAutoBot,
  startAutoBot,
  stopAutoBot,
  deleteAutoBot,
  getAutoBotDetail,
  getAutoTrades,
  getAutoStats,
  getAutoAnalysisLogs,
  findActiveBot,
} from './bot';
export { fetchKlines, fetchOrderBook } from './market';
export { checkHealth, getPaperStatus, enablePaperMode, disablePaperMode } from './system';
export { useWs, useKlineWs } from './ws';
export type {
  WsEvent,
  KlineWsEvent,
  BotStatusEvent,
  PositionEvent,
  TradeEvent,
  NotificationEvent,
  PaperModeEvent,
} from './ws';
export type * from './types';
