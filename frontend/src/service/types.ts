// ── API response types ──────────────────────────────────────

export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// ── User types ──────────────────────────────────────────────

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

// ── Plugin types ────────────────────────────────────────────

export interface PluginParam {
  name: string;
  label: string;
  param_type: 'int' | 'float';
  default: number;
  min?: number;
  max?: number;
  step?: number;
}

export interface Plugin {
  name: string;
  description: string;
  category: string;
  params: PluginParam[];
  required_timeframes?: string[];
}

// ── AI types ────────────────────────────────────────────────

export interface AiGeneratedParam {
  name: string;
  label: string;
  default: number;
  min: number | null;
  max: number | null;
  step: number | null;
}

export interface AiGenerateResult {
  code: string;
  name: string;
  description: string;
  params: AiGeneratedParam[];
  provider: string;
  model: string;
}

// ── Credentials types ───────────────────────────────────────

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

// ── Bot common ──────────────────────────────────────────────

export interface BotSummary {
  id: string;
  bot_type: 'grid' | 'auto';
}

export interface BotHeaderInfo {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  market_type: string;
  status: string;
  leverage: number;
}

// ── Auto Bot types ──────────────────────────────────────────

export interface AutoBot {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  market_type: string;
  status: string;
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
  created_at: string;
  updated_at: string;
}

export interface AutoTrade {
  id: string;
  bot_id: string;
  symbol: string;
  exchange: string;
  // 开仓
  open_side: string;
  open_price: number;
  open_quantity: number;
  open_order_id: string | null;
  open_fee: number;
  opened_at: string;
  // 平仓（未平仓时为 null）
  close_side: string | null;
  close_price: number | null;
  close_quantity: number | null;
  close_order_id: string | null;
  close_fee: number;
  closed_at: string | null;
  // 盈亏
  pnl: number;
  pnl_pct: number;
  // 风控边界（开仓时记录，trailing stop 更新时覆盖 stop_loss）
  stop_loss: number;
  take_profit: number;
  // 触发源与平仓原因
  trigger_source: string;
  close_reason: string | null;
  // 状态
  status: string;
}

export interface AutoBotDetail {
  bot: AutoBot;
}

export interface AutoTradesPage {
  trades: AutoTrade[];
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

export interface AutoBotStats {
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

// ── Grid Bot types ──────────────────────────────────────────

export interface GridBot {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  market_type: string;
  status: string;
  leverage: number;
  initial_capital: number;
  grid_count: number;
  upper_price: number;
  lower_price: number;
  grid_profit_pct: number;
  quantity_per_grid: number;
  total_pnl: number;
  unrealized_pnl: number;
  total_trades: number;
  grid_filled_count: number;
  market_regime: string | null;
  dynamic_adjust: boolean;
  ai_analysis: string | null;
  created_at: string;
  updated_at: string;
}

export interface GridLevelInfo {
  level: number;
  price: number;
  side: string;
  buy_price: number;
  sell_price: number;
  quantity: number;
  buy_filled: boolean;
  sell_filled: boolean;
  hold_quantity: number;
  avg_buy_price: number;
  last_fill_price: number;
}

export interface GridTrade {
  id: string;
  bot_id: string;
  grid_level: number;
  open_side: string;
  open_price: number;
  open_quantity: number;
  opened_at: string;
  close_side: string | null;
  close_price: number | null;
  close_quantity: number | null;
  pnl: number;
  pnl_pct: number;
  status: string;
  closed_at: string | null;
}

export interface GridBotDetail {
  bot: GridBot;
  grid_levels: GridLevelInfo[];
}

export interface GridTradesPage {
  trades: GridTrade[];
  total: number;
  page: number;
  page_size: number;
}

export interface GridBotStats {
  win_rate: number;
  profit_loss_ratio: number;
  max_drawdown: number;
  avg_hold_time: string;
  max_win_streak: number;
  max_loss_streak: number;
  net_pnl: number;
  realized_pnl: number;
  unrealized_pnl: number;
  total_pnl: number;
  total_trades: number;
  closed_trades: number;
  win_trades: number;
  loss_trades: number;
  grid_filled_count: number;
  avg_pnl: number;
  max_profit: number;
  max_loss: number;
}

// ── Analysis types ──────────────────────────────────────────

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
    grid?: Record<string, any>;
    risk?: Record<string, any>;
    risk_warning?: string;
    raw_llm_response?: string | Record<string, any>;
    [key: string]: any;
  } | null;
  llm_model?: string;
  error?: string;
  created_at: string;
}

// ── Market types ────────────────────────────────────────────

export interface KlineCandle {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface OrderBookEntry {
  price: number;
  amount: number;
}

export interface OrderBookData {
  bids: OrderBookEntry[];
  asks: OrderBookEntry[];
}

// ── System types ────────────────────────────────────────────

export interface HealthStatus {
  status: string;
  version?: string;
}

export interface PaperModeStatus {
  paper_mode: boolean;
}

// ── System Metrics types ───────────────────────────────────

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
