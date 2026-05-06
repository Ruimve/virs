import { createSignal, For, Show, onMount } from 'solid-js';
import { api } from '../../lib/api';

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

interface GridTrade {
  id: string;
  bot_id: string;
  symbol: string;
  exchange: string;
  side: string;
  grid_level: number;
  price: number;
  quantity: number;
  pnl: number;
  pnl_pct: number;
  status: string;
  created_at: string;
}

interface AnalysisLog {
  bot_id: string;
  bot_name: string;
  symbol: string;
  exchange: string;
  market_regime: string | null;
  ai_analysis: string | null;
  system_prompt: string | null;
  user_prompt: string | null;
  upper_price: number;
  lower_price: number;
  grid_count: number;
  grid_profit_pct: number;
  quantity_per_grid: number;
  created_at: string;
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
  const [tradeBotName, setTradeBotName] = createSignal('');
  const [loadingTrades, setLoadingTrades] = createSignal(false);

  // Analysis log
  const [showAnalysis, setShowAnalysis] = createSignal(false);
  const [analysisLogs, setAnalysisLogs] = createSignal<AnalysisLog[]>([]);
  const [loadingAnalysis, setLoadingAnalysis] = createSignal(false);
  const [selectedAnalysis, setSelectedAnalysis] = createSignal<AnalysisLog | null>(null);

  const loadBots = async () => {
    setLoading(true);
    try {
      const res = await api.get<{ items: GridBot[]; total: number }>('/grid/list');
      if (res.data) setBots(res.data.items);
    } catch (e: any) {
      console.error('Failed to load bots:', e);
    } finally {
      setLoading(false);
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

      // Step 2: AI Analyze (fills parameters)
      await api.post(`/grid/${botId}/reanalyze`, {});

      // Step 3: Start bot
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
      const res = await api.get<{ items: GridTrade[] }>(`/grid/${bot.id}/trades`);
      setTrades(res.data?.items || []);
    } catch (e) {
      console.error(e);
      setTrades([]);
    } finally {
      setLoadingTrades(false);
    }
  };

  const handleViewAnalysis = async () => {
    setShowAnalysis(true);
    setLoadingAnalysis(true);
    setSelectedAnalysis(null);
    try {
      const res = await api.get<{ items: GridBot[] }>('/grid/list');
      const allBots: GridBot[] = res.data?.items || [];
      const logs: AnalysisLog[] = allBots
        .filter((b: GridBot) => b.ai_analysis)
        .map((b: GridBot) => ({
          bot_id: b.id,
          bot_name: b.name,
          symbol: b.symbol,
          exchange: b.exchange,
          market_regime: b.market_regime,
          ai_analysis: b.ai_analysis,
          system_prompt: b.system_prompt,
          user_prompt: b.user_prompt,
          upper_price: b.upper_price,
          lower_price: b.lower_price,
          grid_count: b.grid_count,
          grid_profit_pct: b.grid_profit_pct,
          quantity_per_grid: b.quantity_per_grid,
          created_at: b.updated_at,
        }))
        .sort((a: AnalysisLog, b: AnalysisLog) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
      setAnalysisLogs(logs);
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

  onMount(loadBots);

  return (
    <div class="min-h-screen bg-gray-50 text-gray-900">
      <div class="max-w-7xl mx-auto px-6 py-8">
        {/* Header */}
        <div class="mb-8">
          <h1 class="text-2xl font-semibold tracking-tight text-gray-900">半自动网格机器人</h1>
          <p class="text-sm text-gray-500 mt-1">AI 自动分析市场并生成网格参数，一键创建并启动</p>
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
              <button
                onClick={() => handleViewAnalysis()}
                class="text-xs text-indigo-600 hover:text-indigo-700 transition-colors"
              >
                查看分析日志 →
              </button>
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
                                onClick={() => handleReanalyze(bot)}
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
                        <div class="grid grid-cols-6 gap-3 mt-3 pt-3 border-t border-gray-100">
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
                            <div class="text-[10px] text-gray-400">累计盈亏</div>
                            <div class="text-xs font-mono mt-0.5">{formatPnl(bot.total_pnl)}</div>
                          </div>
                        </div>
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
          <div class="bg-white border border-gray-200 rounded-2xl w-full max-w-3xl max-h-[80vh] overflow-hidden mx-4 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100">
              <h3 class="text-sm font-medium text-gray-800">{tradeBotName()} — 交易记录</h3>
              <button onClick={() => setShowTrades(false)} class="text-gray-400 hover:text-gray-600 text-lg leading-none">×</button>
            </div>
            <div class="overflow-auto max-h-[60vh]">
              <Show when={!loadingTrades()} fallback={
                <div class="p-8 text-center text-gray-400 text-sm">加载中...</div>
              }>
                <Show when={trades().length > 0} fallback={
                  <div class="p-8 text-center text-gray-400 text-sm">暂无交易记录</div>
                }>
                  <table class="w-full text-xs">
                    <thead>
                      <tr class="text-gray-500 border-b border-gray-100">
                        <th class="text-left px-5 py-2.5 font-medium">时间</th>
                        <th class="text-left px-3 py-2.5 font-medium">方向</th>
                        <th class="text-right px-3 py-2.5 font-medium">层级</th>
                        <th class="text-right px-3 py-2.5 font-medium">价格</th>
                        <th class="text-right px-3 py-2.5 font-medium">数量</th>
                        <th class="text-right px-5 py-2.5 font-medium">盈亏</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={trades()}>
                        {(trade) => (
                          <tr class="border-b border-gray-50 hover:bg-gray-50">
                            <td class="px-5 py-2.5 text-gray-500">{new Date(trade.created_at).toLocaleString('zh-CN')}</td>
                            <td class={`px-3 py-2.5 ${trade.side === 'buy' ? 'text-emerald-600' : 'text-red-600'}`}>
                              {trade.side === 'buy' ? '买入' : '卖出'}
                            </td>
                            <td class="px-3 py-2.5 text-gray-600 text-right font-mono">{trade.grid_level}</td>
                            <td class="px-3 py-2.5 text-gray-600 text-right font-mono">{trade.price.toFixed(2)}</td>
                            <td class="px-3 py-2.5 text-gray-600 text-right font-mono">{trade.quantity.toFixed(6)}</td>
                            <td class="px-5 py-2.5 text-right font-mono">{formatPnl(trade.pnl)}</td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                </Show>
              </Show>
            </div>
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
                        <button
                          onClick={() => setSelectedAnalysis(log)}
                          class={`w-full text-left px-4 py-3 border-b border-gray-100 hover:bg-white transition-colors ${
                            selectedAnalysis()?.bot_id === log.bot_id ? 'bg-indigo-50 border-l-2 border-l-indigo-500' : ''
                          }`}
                        >
                          <div class="text-xs text-gray-700 truncate">{log.bot_name}</div>
                          <div class="text-[10px] text-gray-400 mt-0.5">
                            {log.symbol} · {log.exchange.toUpperCase()} · {new Date(log.created_at).toLocaleString('zh-CN')}
                          </div>
                        </button>
                      )}
                    </For>
                  </Show>
                </Show>
              </div>

              {/* Detail */}
              <div class="flex-1 overflow-auto p-5">
                <Show when={selectedAnalysis()}>
                  {(log) => (
                    <div class="space-y-4">
                      {/* Header */}
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-medium text-gray-800">{log().bot_name}</span>
                        <Show when={log().market_regime}>
                          <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-indigo-50 text-indigo-600 border border-indigo-200">
                            {log().market_regime}
                          </span>
                        </Show>
                      </div>

                      {/* Params */}
                      <div class="grid grid-cols-3 gap-3">
                        <div class="rounded-lg bg-gray-50 border border-gray-200 p-3">
                          <div class="text-[10px] text-gray-400">价格区间</div>
                          <div class="text-sm text-gray-700 font-mono mt-1">
                            {log().lower_price.toFixed(0)} — {log().upper_price.toFixed(0)}
                          </div>
                        </div>
                        <div class="rounded-lg bg-gray-50 border border-gray-200 p-3">
                          <div class="text-[10px] text-gray-400">网格数量</div>
                          <div class="text-sm text-gray-700 font-mono mt-1">{log().grid_count}</div>
                        </div>
                        <div class="rounded-lg bg-gray-50 border border-gray-200 p-3">
                          <div class="text-[10px] text-gray-400">每格利润率</div>
                          <div class="text-sm text-gray-700 font-mono mt-1">{log().grid_profit_pct}%</div>
                        </div>
                      </div>

                      {/* Prompts */}
                      <Show when={log().system_prompt || log().user_prompt}>
                        <div class="space-y-3">
                          <Show when={log().system_prompt}>
                            <div class="rounded-lg bg-indigo-50/50 border border-indigo-100 p-4">
                              <div class="text-[10px] text-indigo-400 mb-2 font-medium">System Prompt</div>
                              <div class="text-xs text-gray-600 leading-relaxed whitespace-pre-wrap max-h-40 overflow-auto">{log().system_prompt}</div>
                            </div>
                          </Show>
                          <Show when={log().user_prompt}>
                            <div class="rounded-lg bg-violet-50/50 border border-violet-100 p-4">
                              <div class="text-[10px] text-violet-400 mb-2 font-medium">User Prompt</div>
                              <div class="text-xs text-gray-600 leading-relaxed whitespace-pre-wrap max-h-40 overflow-auto">{log().user_prompt}</div>
                            </div>
                          </Show>
                        </div>
                      </Show>

                      {/* AI Analysis text */}
                      <Show when={log().ai_analysis}>
                        <div class="rounded-lg bg-gray-50 border border-gray-200 p-4">
                          <div class="text-[10px] text-gray-400 mb-2">AI 分析报告</div>
                          <div class="text-xs text-gray-600 leading-relaxed whitespace-pre-wrap">{log().ai_analysis}</div>
                        </div>
                      </Show>
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
