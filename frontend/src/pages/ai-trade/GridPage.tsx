import { createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { api } from '../../lib/api';

function formatSmart(value: number): string {
  if (value === 0) return '0';
  if (value >= 1) return value.toFixed(4);
  if (value >= 0.001) return value.toFixed(6);
  if (value >= 0.000001) return value.toFixed(8);
  return value.toExponential(2);
}

// ── Types ──

interface GridBot {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  status: string;
  leverage: number;
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
  system_prompt: string | null;
  user_prompt: string | null;
  created_at: string;
  updated_at: string;
}

interface GridLevelInfo {
  level: number;
  price: number;
  side: string;
  buy_price: number;
  sell_price: number;
  open_price: number;
  close_price: number;
  filled: boolean;
  quantity: number;
  buy_filled: boolean;
  sell_filled: boolean;
  hold_quantity: number;
  avg_buy_price: number;
  last_fill_price: number;
}

interface GridTrade {
  id: string;
  bot_id: string;
  symbol: string;
  exchange: string;
  grid_level: number;
  open_side: string;
  open_price: number;
  open_quantity: number;
  open_order_id: string | null;
  opened_at: string;
  close_side: string | null;
  close_price: number | null;
  close_quantity: number | null;
  close_order_id: string | null;
  closed_at: string | null;
  pnl: number;
  pnl_pct: number;
  status: string;
  created_at: string;
}

interface AnalysisLog {
  id: string;
  bot_id: string;
  analysis_type: string;
  status: string;
  system_prompt: string;
  user_prompt: string;
  result: Record<string, any>;
  error: string | null;
  created_at: string;
  completed_at: string | null;
}

// ── Component ──

export default function GridPage() {
  // Form state
  const [name, setName] = createSignal('');
  const [symbol, setSymbol] = createSignal('');
  const [exchange, setExchange] = createSignal('binance');
  const [leverage, setLeverage] = createSignal(1);
  const [dynamicAdjust, setDynamicAdjust] = createSignal(true);

  // Bot list
  const [bots, setBots] = createSignal<GridBot[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [creating, setCreating] = createSignal(false);
  const [error, setError] = createSignal('');

  // Trade modal
  const [showTrades, setShowTrades] = createSignal(false);
  const [trades, setTrades] = createSignal<GridTrade[]>([]);
  const [gridLevels, setGridLevels] = createSignal<GridLevelInfo[]>([]);
  const [tradeBotName, setTradeBotName] = createSignal('');
  const [loadingTrades, setLoadingTrades] = createSignal(false);

  // Analysis log
  const [showAnalysis, setShowAnalysis] = createSignal(false);
  const [analysisLogs, setAnalysisLogs] = createSignal<AnalysisLog[]>([]);
  const [loadingAnalysis, setLoadingAnalysis] = createSignal(false);
  const [selectedAnalysis, setSelectedAnalysis] = createSignal<AnalysisLog | null>(null);

  // Paper trading
  const [paperEnabled, setPaperEnabled] = createSignal(false);
  const [paperLoading, setPaperLoading] = createSignal(false);

  // Grid levels in card
  const [expandedBotId, setExpandedBotId] = createSignal<string | null>(null);
  const [cardGridLevels, setCardGridLevels] = createSignal<GridLevelInfo[]>([]);
  const [loadingGridLevels, setLoadingGridLevels] = createSignal(false);

  const toggleGridLevels = async (bot: GridBot) => {
    if (expandedBotId() === bot.id) {
      setExpandedBotId(null);
      setCardGridLevels([]);
      return;
    }
    setExpandedBotId(bot.id);
    setLoadingGridLevels(true);
    try {
      const res = await api.get<{ grid_levels: GridLevelInfo[] }>(`/grid/${bot.id}/trades`);
      setCardGridLevels(res.data?.grid_levels || []);
    } catch (e) {
      console.error(e);
      setCardGridLevels([]);
    } finally {
      setLoadingGridLevels(false);
    }
  };

  const loadPaperStatus = async () => {
    try {
      const res = await api.get<{ enabled: boolean; pending_count: number }>('/grid/paper/status');
      if (res.data) setPaperEnabled(res.data.enabled);
    } catch (e) {
      console.error('Failed to load paper status:', e);
    }
  };

  const togglePaper = async () => {
    setPaperLoading(true);
    try {
      if (paperEnabled()) {
        await api.post('/grid/paper/disable');
      } else {
        await api.post('/grid/paper/enable');
      }
      setPaperEnabled(!paperEnabled());
    } catch (e: any) {
      setError(e.response?.data?.error || '操作失败');
    } finally {
      setPaperLoading(false);
    }
  };

  const loadBots = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const res = await api.get<{ items: GridBot[]; total: number }>('/grid/list');
      if (res.data) setBots(res.data.items);
    } catch (e: any) {
      console.error('Failed to load bots:', e);
    } finally {
      if (showLoading) setLoading(false);
    }
  };

  const handleCreateAndStart = async () => {
    if (!name().trim() || !symbol().trim()) {
      setError('请填写机器人名称和交易对');
      return;
    }

    setCreating(true);
    setError('');

    try {
      // Step 1: Create bot (draft)
      const createRes = await api.post<{ bot: { id: string } }>('/grid/create', {
        name: name().trim(),
        symbol: symbol().trim().toUpperCase(),
        exchange: exchange(),
        leverage: leverage(),
        dynamic_adjust: dynamicAdjust(),
      });

      const botId = createRes.data?.bot?.id;
      if (!botId) throw new Error('创建机器人失败');

      // Step 2: Start bot (AI analysis will be triggered automatically)
      await api.post(`/grid/${botId}/start`);

      // Reset form
      setName('');
      setSymbol('');
      setLeverage(1);
      setDynamicAdjust(true);

      await loadBots();
    } catch (e: any) {
      const msg = e.response?.data?.error || e.message || '操作失败';
      setError(msg);
    } finally {
      setCreating(false);
    }
  };

  const handleStop = async (bot: GridBot) => {
    try {
      await api.post(`/grid/${bot.id}/stop`);
      await loadBots();
    } catch (e: any) {
      setError(e.response?.data?.error || '停止失败');
    }
  };

  const handleStart = async (bot: GridBot) => {
    try {
      await api.post(`/grid/${bot.id}/start`);
      await loadBots();
    } catch (e: any) {
      setError(e.response?.data?.error || '启动失败');
    }
  };

  const handleReanalyze = async (bot: GridBot) => {
    try {
      await api.post(`/grid/${bot.id}/reanalyze`, {});
      await loadBots();
    } catch (e: any) {
      setError(e.response?.data?.error || '重新分析失败');
    }
  };

  const handleDelete = async (bot: GridBot) => {
    if (!confirm(`确定删除机器人 "${bot.name}" 吗？`)) return;
    try {
      if (bot.status === 'running') {
        await api.post(`/grid/${bot.id}/stop`);
      }
      await api.del(`/grid/${bot.id}/delete`);
      await loadBots();
    } catch (e: any) {
      setError(e.response?.data?.error || '删除失败');
    }
  };

  const handleViewTrades = async (bot: GridBot) => {
    setTradeBotName(bot.name);
    setShowTrades(true);
    setLoadingTrades(true);
    try {
      const res = await api.get<{ items: GridTrade[]; grid_levels: GridLevelInfo[] }>(`/grid/${bot.id}/trades`);
      setTrades(res.data?.items || []);
      setGridLevels(res.data?.grid_levels || []);
    } catch (e) {
      console.error(e);
      setTrades([]);
      setGridLevels([]);
    } finally {
      setLoadingTrades(false);
    }
  };

  const handleViewAnalysis = async (bot: GridBot) => {
    setShowAnalysis(true);
    setLoadingAnalysis(true);
    setSelectedAnalysis(null);
    try {
      const res = await api.get<{ items: AnalysisLog[] }>(`/grid/analysis-logs?bot_id=${bot.id}`);
      if (res.data?.items) {
        setAnalysisLogs(res.data.items);
      } else {
        setAnalysisLogs([]);
      }
    } catch (e) {
      console.error(e);
      setAnalysisLogs([]);
    } finally {
      setLoadingAnalysis(false);
    }
  };

  const statusLabel = (status: string) => {
    const map: Record<string, { text: string; class: string }> = {
      running: { text: '运行中', class: 'bg-emerald-50 text-emerald-700 border-emerald-200' },
      paused: { text: '已暂停', class: 'bg-amber-50 text-amber-700 border-amber-200' },
      stopped: { text: '已停止', class: 'bg-gray-100 text-gray-500 border-gray-200' },
      draft: { text: '草稿', class: 'bg-gray-100 text-gray-400 border-gray-200' },
    };
    return map[status] || { text: status, class: 'bg-gray-100 text-gray-500 border-gray-200' };
  };

  const formatPnl = (pnl: number) => {
    if (pnl > 0) return <span class="text-emerald-600 font-medium">+{pnl.toFixed(2)}</span>;
    if (pnl < 0) return <span class="text-red-600 font-medium">{pnl.toFixed(2)}</span>;
    return <span class="text-gray-400">0.00</span>;
  };

  onMount(() => {
    loadBots();
    loadPaperStatus();
    const interval = setInterval(() => { loadBots(false); }, 10000);
    onCleanup(() => clearInterval(interval));
  });

  return (
    <div class="min-h-screen bg-gray-50 text-gray-900">
      <div class="max-w-7xl mx-auto px-6 py-8">
        {/* Header */}
        <div class="mb-8 flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-semibold tracking-tight text-gray-900">半自动网格机器人</h1>
            <p class="text-sm text-gray-500 mt-1">AI 自动分析市场并生成网格参数，一键创建并启动</p>
          </div>
          <button
            onClick={togglePaper}
            disabled={paperLoading()}
            class={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium border transition-all ${
              paperEnabled()
                ? 'bg-amber-50 border-amber-200 text-amber-700 hover:bg-amber-100'
                : 'bg-gray-50 border-gray-200 text-gray-600 hover:bg-gray-100'
            }`}
          >
            <span class={`relative w-2 h-2 rounded-full ${paperEnabled() ? 'bg-amber-500' : 'bg-gray-300'}`} />
            <Show when={paperLoading()} fallback={
              <span>{paperEnabled() ? 'Paper 交易中' : 'Paper 交易'}</span>
            }>
              <span>切换中...</span>
            </Show>
          </button>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
          {/* ── Left: Create Form ── */}
          <div class="lg:col-span-4">
            <div class="rounded-xl border border-gray-200 bg-white p-5 shadow-sm">
              <h2 class="text-sm font-medium text-gray-700 mb-4">创建机器人</h2>

              <div class="space-y-4">
                {/* Name */}
                <div>
                  <label class="block text-xs text-gray-500 mb-1.5">机器人名称</label>
                  <input
                    type="text"
                    value={name()}
                    onInput={(e) => setName(e.currentTarget.value)}
                    placeholder="例如：BTC 网格 #1"
                    class="w-full bg-white border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-800 placeholder:text-gray-400 focus:outline-none focus:border-indigo-300 focus:ring-2 focus:ring-indigo-100 transition-colors"
                  />
                </div>

                {/* Symbol */}
                <div>
                  <label class="block text-xs text-gray-500 mb-1.5">交易对</label>
                  <input
                    type="text"
                    value={symbol()}
                    onInput={(e) => setSymbol(e.currentTarget.value)}
                    placeholder="例如：BTCUSDT"
                    class="w-full bg-white border border-gray-200 rounded-lg px-3 py-2 text-sm text-gray-800 placeholder:text-gray-400 focus:outline-none focus:border-indigo-300 focus:ring-2 focus:ring-indigo-100 transition-colors"
                  />
                </div>

                {/* Exchange */}
                <div>
                  <label class="block text-xs text-gray-500 mb-1.5">交易所</label>
                  <div class="flex gap-2">
                    <For each={['binance', 'okx', 'bybit']}>
                      {(ex) => (
                        <button
                          onClick={() => setExchange(ex)}
                          class={`flex-1 py-2 text-xs font-medium rounded-lg border transition-all ${
                            exchange() === ex
                              ? 'bg-indigo-50 border-indigo-200 text-indigo-700'
                              : 'bg-white border-gray-200 text-gray-500 hover:border-gray-300 hover:text-gray-700'
                          }`}
                        >
                          {ex.toUpperCase()}
                        </button>
                      )}
                    </For>
                  </div>
                </div>

                {/* Leverage */}
                <div>
                  <label class="block text-xs text-gray-500 mb-1.5">杠杆倍数</label>
                  <div class="flex items-center gap-3">
                    <input
                      type="range"
                      min="1"
                      max="20"
                      value={leverage()}
                      onInput={(e) => setLeverage(Number(e.currentTarget.value))}
                      class="flex-1 accent-indigo-500"
                    />
                    <span class="text-sm font-mono text-gray-700 w-8 text-right">{leverage()}x</span>
                  </div>
                </div>

                {/* Dynamic Adjust */}
                <div class="flex items-center justify-between py-1">
                  <div>
                    <div class="text-sm text-gray-700">动态调整</div>
                    <div class="text-xs text-gray-400">AI 定期评估并调整网格参数</div>
                  </div>
                  <button
                    onClick={() => setDynamicAdjust(!dynamicAdjust())}
                    class={`relative w-10 h-5 rounded-full transition-colors ${
                      dynamicAdjust() ? 'bg-indigo-500' : 'bg-gray-300'
                    }`}
                  >
                    <div
                      class={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${
                        dynamicAdjust() ? 'translate-x-5' : 'translate-x-0.5'
                      }`}
                    />
                  </button>
                </div>

                {/* Error */}
                <Show when={error()}>
                  <div class="text-xs text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
                    {error()}
                  </div>
                </Show>

                {/* Submit */}
                <button
                  onClick={handleCreateAndStart}
                  disabled={creating() || !name().trim() || !symbol().trim()}
                  class="w-full py-2.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 disabled:bg-gray-200 disabled:text-gray-400 text-white text-sm font-medium transition-colors flex items-center justify-center gap-2"
                >
                  <Show when={creating()} fallback={<span>创建并启动</span>}>
                    <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                    <span>AI 分析中...</span>
                  </Show>
                </button>
              </div>
            </div>
          </div>

          {/* ── Right: Bot List ── */}
          <div class="lg:col-span-8">
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-sm font-medium text-gray-700">我的机器人</h2>
              <Show when={bots().length > 0}>
                <button
                  onClick={() => handleViewAnalysis(bots()[0])}
                  class="text-xs text-indigo-600 hover:text-indigo-700 transition-colors"
                >
                  查看分析日志 →
                </button>
              </Show>
            </div>

            <Show
              when={!loading() && bots().length > 0}
              fallback={
                <div class="rounded-xl border border-gray-200 bg-white p-12 text-center shadow-sm">
                  <Show when={!loading()} fallback={
                    <div class="flex justify-center">
                      <svg class="animate-spin h-6 w-6 text-gray-400" viewBox="0 0 24 24" fill="none">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                      </svg>
                    </div>
                  }>
                    <div class="text-gray-400 text-sm">暂无机器人</div>
                    <div class="text-gray-300 text-xs mt-1">填写左侧表单创建第一个机器人</div>
                  </Show>
                </div>
              }
            >
              <div class="space-y-3">
                <For each={bots()}>
                  {(bot) => {
                    const s = () => statusLabel(bot.status);
                    return (
                      <div class="rounded-xl border border-gray-200 bg-white p-4 hover:border-gray-300 transition-colors shadow-sm">
                        <div class="flex items-start justify-between">
                          <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-2 mb-1.5">
                              <span class="text-sm font-medium text-gray-800 truncate">{bot.name}</span>
                              <span class={`text-[10px] px-1.5 py-0.5 rounded-full border ${s().class}`}>
                                {s().text}
                              </span>
                            </div>
                            <div class="flex items-center gap-2 text-xs text-gray-500">
                              <span>{bot.symbol}</span>
                              <span class="text-gray-300">·</span>
                              <span>{bot.exchange.toUpperCase()}</span>
                              <span class="text-gray-300">·</span>
                              <span>{bot.leverage}x</span>
                              <Show when={bot.market_regime}>
                                <span class="text-gray-300">·</span>
                                <span class="text-indigo-500">{bot.market_regime}</span>
                              </Show>
                            </div>
                          </div>

                          <div class="flex items-center gap-1.5 ml-4">
                            <Show when={bot.status === 'running'}>
                              <button
                                onClick={() => handleStop(bot)}
                                class="px-2.5 py-1 text-[11px] rounded-md bg-red-50 text-red-600 hover:bg-red-100 border border-red-200 transition-colors"
                              >
                                停止
                              </button>
                            </Show>
                            <Show when={bot.status !== 'running' && bot.status !== 'stopped'}>
                              <button
                                onClick={() => handleStart(bot)}
                                class="px-2.5 py-1 text-[11px] rounded-md bg-indigo-50 text-indigo-600 hover:bg-indigo-100 border border-indigo-200 transition-colors"
                              >
                                启动
                              </button>
                            </Show>
                            <Show when={bot.status === 'running'}>
                              <button
                                onClick={() => handleReanalyze(bot)}
                                class="px-2.5 py-1 text-[11px] rounded-md bg-amber-50 text-amber-600 hover:bg-amber-100 border border-amber-200 transition-colors"
                              >
                                重分析
                              </button>
                            </Show>
                            <button
                              onClick={() => handleViewTrades(bot)}
                              class="px-2.5 py-1 text-[11px] rounded-md bg-gray-50 text-gray-600 hover:bg-gray-100 border border-gray-200 transition-colors"
                            >
                              交易
                            </button>
                            <button
                              onClick={() => handleDelete(bot)}
                              class="px-2.5 py-1 text-[11px] rounded-md bg-gray-50 text-gray-500 hover:bg-red-50 hover:text-red-600 border border-gray-200 hover:border-red-200 transition-colors"
                            >
                              删除
                            </button>
                          </div>
                        </div>

                        {/* Stats row */}
                        <div class="grid grid-cols-7 gap-3 mt-3 pt-3 border-t border-gray-100">
                          <div>
                            <div class="text-[10px] text-gray-400">价格区间</div>
                            <div class="text-xs text-gray-600 font-mono mt-0.5">
                              {bot.upper_price > 0 ? `${bot.lower_price.toFixed(0)} - ${bot.upper_price.toFixed(0)}` : '-'}
                            </div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">网格数</div>
                            <div class="text-xs text-gray-600 font-mono mt-0.5">{bot.grid_count || '-'}</div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">利润率</div>
                            <div class="text-xs text-gray-600 font-mono mt-0.5">{bot.grid_profit_pct}%</div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">已填充</div>
                            <div class="text-xs text-gray-600 font-mono mt-0.5">{bot.grid_filled_count}/{bot.grid_count}</div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">交易次数</div>
                            <div class="text-xs text-gray-600 font-mono mt-0.5">{bot.total_trades}</div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">已实现盈亏</div>
                            <div class="text-xs font-mono mt-0.5">{formatPnl(bot.total_pnl)}</div>
                          </div>
                          <div>
                            <div class="text-[10px] text-gray-400">未实现盈亏</div>
                            <div class="text-xs font-mono mt-0.5">{formatPnl(bot.unrealized_pnl)}</div>
                          </div>
                        </div>

                        {/* Grid levels (collapsible) */}
                        <Show when={bot.upper_price > 0 && bot.grid_count > 0}>
                          <button
                            onClick={() => toggleGridLevels(bot)}
                            class="w-full mt-2 pt-2 border-t border-gray-100 text-[11px] text-gray-500 hover:text-gray-700 transition-colors flex items-center gap-1"
                          >
                            <span class={`transition-transform ${expandedBotId() === bot.id ? 'rotate-90' : ''}`}>▶</span>
                            网格层级 ({bot.grid_filled_count}/{bot.grid_count} 已成交)
                          </button>
                          <Show when={expandedBotId() === bot.id}>
                            <div class="mt-1 max-h-48 overflow-auto rounded border border-gray-100">
                              <Show when={!loadingGridLevels()} fallback={
                                <div class="p-3 text-center text-gray-400 text-xs">加载中...</div>
                              }>
                                <table class="w-full text-[11px]">
                                  <thead class="sticky top-0 bg-white">
                                    <tr class="text-gray-400 border-b border-gray-100">
                                      <th class="text-left px-2.5 py-1.5 font-medium">层级</th>
                                      <th class="text-center px-2 py-1.5 font-medium">方向</th>
                                      <th class="text-right px-2.5 py-1.5 font-medium">开仓价</th>
                                      <th class="text-right px-2.5 py-1.5 font-medium">平仓价</th>
                                      <th class="text-right px-2.5 py-1.5 font-medium">成交价</th>
                                      <th class="text-right px-2.5 py-1.5 font-medium">持仓</th>
                                      <th class="text-center px-2 py-1.5 font-medium">状态</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    <For each={cardGridLevels()}>
                                      {(level) => {
                                        const isHolding = Math.abs(level.hold_quantity) > 0;
                                        const isClosed = (level.buy_filled && level.sell_filled) && Math.abs(level.hold_quantity) === 0;
                                        return (
                                        <tr class={`border-b border-gray-50 ${isHolding ? 'bg-emerald-50/50' : isClosed ? 'bg-gray-50/50' : ''}`}>
                                          <td class="px-2.5 py-1 text-gray-600 font-mono">{level.level}</td>
                                          <td class="px-2 py-1 text-center">
                                            <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${level.side === 'buy' ? 'bg-emerald-100 text-emerald-700' : 'bg-red-100 text-red-700'}`}>
                                              {level.side === 'buy' ? '多' : '空'}
                                            </span>
                                          </td>
                                          <td class="px-2.5 py-1 text-emerald-600 text-right font-mono">{level.open_price.toFixed(2)}</td>
                                          <td class="px-2.5 py-1 text-red-500 text-right font-mono">{level.close_price.toFixed(2)}</td>
                                          <td class="px-2.5 py-1 text-gray-700 text-right font-mono">{level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}</td>
                                          <td class="px-2.5 py-1 text-gray-600 text-right font-mono">{Math.abs(level.hold_quantity) > 0 ? `${formatSmart(Math.abs(level.hold_quantity))} ≈ $${(Math.abs(level.hold_quantity) * level.price).toFixed(2)}` : '-'}</td>
                                          <td class="px-2 py-1 text-center">
                                            {isClosed ? (
                                              <span class="inline-block w-1.5 h-1.5 rounded-full bg-gray-400" title="已平仓"></span>
                                            ) : isHolding ? (
                                              <span class="inline-block w-1.5 h-1.5 rounded-full bg-emerald-500" title="持仓中"></span>
                                            ) : (
                                              <span class="inline-block w-1.5 h-1.5 rounded-full bg-gray-300" title="等待"></span>
                                            )}
                                          </td>
                                        </tr>
                                      )}}
                                    </For>
                                  </tbody>
                                </table>
                              </Show>
                            </div>
                          </Show>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </div>

      {/* ── Trade Modal ── */}
      <Show when={showTrades()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm" onClick={() => setShowTrades(false)}>
          <div class="bg-white border border-gray-200 rounded-2xl w-full max-w-5xl max-h-[80vh] overflow-hidden mx-4 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100">
              <h3 class="text-sm font-medium text-gray-800">{tradeBotName()}</h3>
              <button onClick={() => setShowTrades(false)} class="text-gray-400 hover:text-gray-600 text-lg leading-none">×</button>
            </div>
            <Show when={!loadingTrades()} fallback={
              <div class="p-8 text-center text-gray-400 text-sm">加载中...</div>
            }>
              <div class="flex h-[60vh]">
                {/* 网格层级表格 */}
                <div class="w-56 border-r border-gray-100 overflow-auto bg-gray-50">
                  <div class="px-3 py-2 text-[10px] font-medium text-gray-500 uppercase tracking-wide border-b border-gray-100 sticky top-0 bg-gray-50">
                    网格层级
                  </div>
                  <Show when={gridLevels().length > 0} fallback={
                    <div class="p-4 text-center text-gray-400 text-xs">暂无网格数据</div>
                  }>
                    <table class="w-full text-xs">
                      <thead>
                        <tr class="text-gray-400 border-b border-gray-100">
                          <th class="text-left px-3 py-1.5 font-medium">层级</th>
                          <th class="text-center px-2 py-1.5 font-medium">方向</th>
                          <th class="text-right px-3 py-1.5 font-medium">开仓价</th>
                          <th class="text-right px-3 py-1.5 font-medium">平仓价</th>
                          <th class="text-right px-3 py-1.5 font-medium">成交价</th>
                          <th class="text-right px-3 py-1.5 font-medium">持仓</th>
                          <th class="text-center px-2 py-1.5 font-medium">状态</th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={gridLevels()}>
                          {(level) => {
                            const isHolding = Math.abs(level.hold_quantity) > 0;
                            const isClosed = (level.buy_filled && level.sell_filled) && Math.abs(level.hold_quantity) === 0;
                            return (
                            <tr class={`border-b border-gray-50 ${isHolding ? 'bg-emerald-50/50' : isClosed ? 'bg-gray-50/50' : ''}`}>
                              <td class="px-3 py-1.5 text-gray-600 font-mono">{level.level}</td>
                              <td class="px-2 py-1.5 text-center">
                                <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${level.side === 'buy' ? 'bg-emerald-100 text-emerald-700' : 'bg-red-100 text-red-700'}`}>
                                  {level.side === 'buy' ? '多' : '空'}
                                </span>
                              </td>
                              <td class="px-3 py-1.5 text-emerald-600 text-right font-mono">{level.open_price.toFixed(2)}</td>
                              <td class="px-3 py-1.5 text-red-500 text-right font-mono">{level.close_price.toFixed(2)}</td>
                              <td class="px-3 py-1.5 text-gray-700 text-right font-mono">{level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}</td>
                              <td class="px-3 py-1.5 text-gray-600 text-right font-mono">{Math.abs(level.hold_quantity) > 0 ? `${formatSmart(Math.abs(level.hold_quantity))} ≈ $${(Math.abs(level.hold_quantity) * level.price).toFixed(2)}` : '-'}</td>
                              <td class="px-2 py-1.5 text-center">
                                {isClosed ? (
                                  <span class="inline-block w-2 h-2 rounded-full bg-gray-400" title="已平仓"></span>
                                ) : isHolding ? (
                                  <span class="inline-block w-2 h-2 rounded-full bg-emerald-500" title="持仓中"></span>
                                ) : (
                                  <span class="inline-block w-2 h-2 rounded-full bg-gray-300" title="等待"></span>
                                )}
                              </td>
                            </tr>
                          )}}
                        </For>
                      </tbody>
                    </table>
                  </Show>
                </div>

                {/* 交易记录表格 */}
                <div class="flex-1 overflow-auto">
                  <div class="px-5 py-2 text-[10px] font-medium text-gray-500 uppercase tracking-wide border-b border-gray-100 sticky top-0 bg-white">
                    交易记录
                  </div>
                  <Show when={trades().length > 0} fallback={
                    <div class="p-8 text-center text-gray-400 text-sm">暂无交易记录</div>
                  }>
                    <table class="w-full text-xs">
                      <thead>
                        <tr class="text-gray-500 border-b border-gray-100">
                          <th class="text-left px-4 py-2.5 font-medium">层级</th>
                          <th class="text-left px-2 py-2.5 font-medium">开仓方向</th>
                          <th class="text-right px-2 py-2.5 font-medium">开仓价</th>
                          <th class="text-right px-2 py-2.5 font-medium">开仓量</th>
                          <th class="text-left px-2 py-2.5 font-medium">平仓方向</th>
                          <th class="text-right px-2 py-2.5 font-medium">平仓价</th>
                          <th class="text-right px-2 py-2.5 font-medium">平仓量</th>
                          <th class="text-right px-2 py-2.5 font-medium">盈亏</th>
                          <th class="text-left px-3 py-2.5 font-medium">状态</th>
                          <th class="text-left px-3 py-2.5 font-medium">时间</th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={trades()}>
                          {(trade) => (
                            <tr class="border-b border-gray-50 hover:bg-gray-50">
                              <td class="px-4 py-2.5 text-gray-600 text-right font-mono">{trade.grid_level}</td>
                              <td class={`px-2 py-2.5 ${trade.open_side === 'buy' ? 'text-emerald-600' : 'text-red-600'}`}>
                                {trade.open_side === 'buy' ? '买入' : '卖出'}
                              </td>
                              <td class="px-2 py-2.5 text-gray-600 text-right font-mono">{trade.open_price.toFixed(2)}</td>
                              <td class="px-2 py-2.5 text-gray-600 text-right font-mono">{trade.open_quantity.toFixed(6)}</td>
                              <td class={`px-2 py-2.5 ${trade.close_side === 'buy' ? 'text-emerald-600' : trade.close_side === 'sell' ? 'text-red-600' : 'text-gray-400'}`}>
                                {trade.close_side === 'buy' ? '买入' : trade.close_side === 'sell' ? '卖出' : '-'}
                              </td>
                              <td class="px-2 py-2.5 text-gray-600 text-right font-mono">{trade.close_price != null ? trade.close_price.toFixed(2) : '-'}</td>
                              <td class="px-2 py-2.5 text-gray-600 text-right font-mono">{trade.close_quantity != null ? trade.close_quantity.toFixed(6) : '-'}</td>
                              <td class="px-2 py-2.5 text-right font-mono">{trade.close_side ? formatPnl(trade.pnl) : '-'}</td>
                              <td class="px-3 py-2.5">
                                <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${trade.status === 'open' ? 'bg-blue-100 text-blue-700' : trade.status === 'orphaned' ? 'bg-amber-100 text-amber-700' : 'bg-gray-100 text-gray-600'}`}>
                                  {trade.status === 'open' ? '持仓中' : trade.status === 'orphaned' ? '异常' : '已平仓'}
                                </span>
                              </td>
                              <td class="px-3 py-2.5 text-gray-500">{new Date(trade.status === 'open' ? trade.opened_at : (trade.closed_at || trade.opened_at)).toLocaleString('zh-CN')}</td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </Show>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* ── Analysis Log Modal ── */}
      <Show when={showAnalysis()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm" onClick={() => setShowAnalysis(false)}>
          <div class="bg-white border border-gray-200 rounded-2xl w-full max-w-4xl max-h-[85vh] overflow-hidden mx-4 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100">
              <h3 class="text-sm font-medium text-gray-800">AI 分析日志</h3>
              <button onClick={() => setShowAnalysis(false)} class="text-gray-400 hover:text-gray-600 text-lg leading-none">×</button>
            </div>
            <div class="flex h-[calc(85vh-57px)]">
              {/* Log list */}
              <div class="w-72 border-r border-gray-100 overflow-auto bg-gray-50">
                <Show when={!loadingAnalysis()} fallback={
                  <div class="p-4 text-center text-gray-400 text-xs">加载中...</div>
                }>
                  <Show when={analysisLogs().length > 0} fallback={
                    <div class="p-4 text-center text-gray-400 text-xs">暂无分析记录</div>
                  }>
                    <For each={analysisLogs()}>
                      {(log) => (
                        <div
                          class={`p-4 rounded-lg border cursor-pointer transition-all ${
                            selectedAnalysis()?.id === log.id
                              ? 'border-indigo-300 bg-indigo-50/50'
                              : 'border-gray-200 hover:border-gray-300'
                          }`}
                          onClick={() => setSelectedAnalysis(log)}
                        >
                          <div class="flex items-center justify-between mb-2">
                            <div class="flex items-center gap-2">
                              <span class={`px-2 py-0.5 text-[10px] rounded-full font-medium ${
                                log.analysis_type === 'initial'
                                  ? 'bg-blue-50 text-blue-600'
                                  : 'bg-purple-50 text-purple-600'
                              }`}>
                                {log.analysis_type === 'initial' ? '首次分析' : '周期分析'}
                              </span>
                              <span class={`px-2 py-0.5 text-[10px] rounded-full font-medium ${
                                log.status === 'pending'
                                  ? 'bg-yellow-50 text-yellow-600'
                                  : log.status === 'completed'
                                    ? 'bg-green-50 text-green-600'
                                    : 'bg-red-50 text-red-600'
                              }`}>
                                {log.status === 'pending' ? '处理中' : log.status === 'completed' ? '已完成' : '失败'}
                              </span>
                              <span class="text-xs text-gray-400">
                                {new Date(log.created_at).toLocaleString()}
                              </span>
                            </div>
                          </div>
                          <Show when={log.status === 'completed'}>
                            <p class="text-sm text-gray-700 line-clamp-2">
                              {log.result?.analysis || log.result?.reason || '无分析内容'}
                            </p>
                          </Show>
                          <Show when={log.status === 'pending'}>
                            <p class="text-sm text-yellow-600">等待 AI 响应...</p>
                          </Show>
                          <Show when={log.status === 'failed'}>
                            <p class="text-sm text-red-500">{log.error || '分析失败'}</p>
                          </Show>
                        </div>
                      )}
                    </For>
                  </Show>
                </Show>
              </div>

              {/* Detail */}
              <div class="flex-1 overflow-auto p-5">
                <Show when={selectedAnalysis()}>
                  {(log) => (
                    <div class="mt-4 p-4 rounded-lg border border-gray-200 bg-white">
                      <div class="flex items-center justify-between mb-3">
                        <h3 class="text-sm font-medium text-gray-900">分析详情</h3>
                        <button onClick={() => setSelectedAnalysis(null)} class="text-gray-400 hover:text-gray-600">
                          ✕
                        </button>
                      </div>

                      <div class="space-y-3 text-xs">
                        <div>
                          <div class="font-medium text-gray-500 mb-1">System Prompt</div>
                          <pre class="p-2 bg-gray-50 rounded text-gray-700 whitespace-pre-wrap break-words max-h-40 overflow-y-auto">{log().system_prompt}</pre>
                        </div>
                        <div>
                          <div class="font-medium text-gray-500 mb-1">User Prompt</div>
                          <pre class="p-2 bg-gray-50 rounded text-gray-700 whitespace-pre-wrap break-words max-h-40 overflow-y-auto">{log().user_prompt}</pre>
                        </div>
                        <div>
                          <div class="font-medium text-gray-500 mb-1">AI Result</div>
                          <pre class="p-2 bg-gray-50 rounded text-gray-700 whitespace-pre-wrap break-words max-h-60 overflow-y-auto">{JSON.stringify(log().result, null, 2)}</pre>
                        </div>
                        <Show when={log().error}>
                          <div>
                            <div class="font-medium text-red-500 mb-1">Error</div>
                            <pre class="p-2 bg-red-50 rounded text-red-700">{log().error}</pre>
                          </div>
                        </Show>
                      </div>
                    </div>
                  )}
                </Show>
                <Show when={!selectedAnalysis()}>
                  <div class="flex items-center justify-center h-full text-gray-400 text-sm">
                    选择左侧记录查看详情
                  </div>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
