export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  message?: string;
}

export interface UserInfo {
  id: string;
  username: string;
  role: 'admin' | 'user';
  created_at?: string;
}

export interface LoginResponse {
  token: string;
  user: UserInfo;
}

export interface PermissionItem {
  name: string;
  label: string;
  status: 'ok' | 'warn' | 'error';
  detail: string;
}

export interface PositionModeResult {
  supported: boolean;
  mode: 'hedge' | 'oneway' | null;
  message?: string;
}

export interface StrategyDetail {
  name: string;
  description: string;
  version: number;
  source: { kind: 'human' } | { kind: 'aigenerated'; model: string };
}

export interface Bot {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  status: string;
  bot_type: 'chat' | 'agent';
  leverage: number;
  max_position_pct: number;
  decide_interval_secs: number;
  initial_capital: number;
  market_regime: string | null;
  ai_analysis: string | null;
  total_pnl: number;
  total_trades: number;
  win_trades: number;
  loss_trades: number;
  strategy_file: string | null;
  created_at: string;
  updated_at: string;
}

export interface BotTrade {
  id: string;
  bot_id: string;
  symbol: string;
  exchange: string;

  open_side: string;
  open_price: number;
  open_quantity: number;
  open_order_id: string | null;
  open_fee: number;
  opened_at: string;

  close_side: string | null;
  close_price: number | null;
  close_quantity: number | null;
  close_order_id: string | null;
  close_fee: number;
  closed_at: string | null;

  pnl: number;
  pnl_pct: number;

  stop_loss: number;
  take_profit: number;

  trigger_source: string;
  close_reason: string | null;

  status: string;
}

export interface BotDetail {
  bot: Bot;
  strategy: StrategyDetail | null;
}

export interface BotTradesPage {
  trades: BotTrade[];
  total: number;
  page: number;
  page_size: number;
}

export interface AnalysisLogsPage {
  items: AnalysisLog[];
  total: number;
  page: number;
  page_size: number;
}

export interface BotStats {
  win_rate: number;
  profit_loss_ratio: number;
  max_drawdown: number;
  avg_hold_time: string;
  max_win_streak: number;
  max_loss_streak: number;
  total_fee: number;
  net_pnl: number;
  total_trades: number;
  win_trades: number;
  loss_trades: number;
  total_volume: number;
  avg_pnl: number;
  max_profit: number;
  max_loss: number;
  net_pnl_after_fee: number;
}

export interface AnalysisLog {
  id: string;
  bot_id: string;
  analysis_type?: string;
  status: string;
  system_prompt?: string;
  user_prompt?: string;
  result: {
    action?: string;
    reason?: string;
    confidence?: number;
    analysis?: string;
    decision?: {
      action: string;
      reason: string;
      confidence: number;
    };
    market?: Record<string, any>;
    risk?: Record<string, any>;
    risk_warning?: string;
    raw_llm_response?: string | Record<string, any>;
    [key: string]: any;
  } | null;
  llm_model?: string;
  strategy_file?: string | null;
  error?: string;
  execution_status?: string | null;
  intercept_reason?: string | null;
  completed_at?: string | null;
  created_at: string;
}

export interface KlineCandle {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface HealthStatus {
  status: string;
  version?: string;
}

export interface PaperModeStatus {
  paper_mode: boolean | null;

  restore_error: string | null;
}

export interface CpuInfo {
  usage_pct: number;
  core_count: number;
  brand: string;
  frequency_mhz: number;
}

export interface MemoryInfo {
  total_bytes: number;
  used_bytes: number;
  usage_pct: number;
}

export interface SwapInfo {
  total_bytes: number;
  used_bytes: number;
}

export interface LoadAverage {
  one: number;
  five: number;
  fifteen: number;
}

export interface DiskInfo {
  mount_point: string;
  total_bytes: number;
  used_bytes: number;
  usage_pct: number;
}

export interface NetworkInfo {
  name: string;
  total_rx_bytes: number;
  total_tx_bytes: number;
  ips: string[];
}

export interface SystemInfo {
  cpu: CpuInfo;
  memory: MemoryInfo;
  swap: SwapInfo;
  load_average: LoadAverage;
  process_count: number;
  disks: DiskInfo[];
  network: NetworkInfo[];
  uptime_secs: number;
  host_name: string;
  os_name: string;
  os_version: string;
}

export interface DeepSeekModel {
  id: string;
  owned_by: string;
}

export interface BalanceInfo {
  total_balance: string;
  currency: string;
}
