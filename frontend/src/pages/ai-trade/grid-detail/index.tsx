import { createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { useParams, useNavigate } from '@solidjs/router';
import { api } from '../../../lib/api';
import { isDark, toggleTheme } from '../../../lib/theme';

function formatSmart(value: number): string {
  if (value === 0) return '0';
  if (value >= 1) return value.toFixed(4);
  if (value >= 0.001) return value.toFixed(6);
  if (value >= 0.000001) return value.toFixed(8);
  return value.toExponential(2);
}

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
  created_at: string;
}

interface GridLevelInfo {
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

interface GridTrade {
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
}

export default function GridDetailPage() {
  const params = useParams();
  const navigate = useNavigate();

  const [bot, setBot] = createSignal<GridBot | null>(null);
  const [error, setError] = createSignal('');

  const [gridLevels, setGridLevels] = createSignal<GridLevelInfo[]>([]);
  const [trades, setTrades] = createSignal<GridTrade[]>([]);
  const [loadingTrades, setLoadingTrades] = createSignal(false);

  const [analysisLogs, setAnalysisLogs] = createSignal<AnalysisLog[]>([]);
  const [selectedAnalysis, setSelectedAnalysis] = createSignal<AnalysisLog | null>(null);
  const [loadingAnalysis, setLoadingAnalysis] = createSignal(false);

  const [activeTab, setActiveTab] = createSignal<'levels' | 'trades' | 'analysis'>('levels');
  const [showSystemPrompt, setShowSystemPrompt] = createSignal(false);
  const [showUserPrompt, setShowUserPrompt] = createSignal(false);

  const loadBot = async () => {
    try {
      const res = await api.get<{ bot: GridBot; trades: GridTrade[]; grid_levels: GridLevelInfo[] }>(`/grid/${params.id}`);
      if (res.data?.bot) {
        setBot(res.data.bot);
        if (res.data.grid_levels?.length) setGridLevels(res.data.grid_levels);
        if (res.data.trades?.length) setTrades(res.data.trades);
      }
      else setError('机器人不存在');
    } catch (e) {
      setError('加载失败');
    }
  };

  const loadTrades = async () => {
    setLoadingTrades(true);
    try {
      const res = await api.get<{ items: GridTrade[]; grid_levels: GridLevelInfo[] }>(`/grid/${params.id}/trades`);
      setTrades(res.data?.items || []);
      setGridLevels(res.data?.grid_levels || []);
    } catch (e) {
      console.error(e);
    } finally {
      setLoadingTrades(false);
    }
  };

  const loadAnalysis = async () => {
    setLoadingAnalysis(true);
    setSelectedAnalysis(null);
    try {
      const res = await api.get<{ items: AnalysisLog[] }>(`/grid/analysis-logs?bot_id=${params.id}`);
      setAnalysisLogs(res.data?.items || []);
      if (res.data?.items?.length) setSelectedAnalysis(res.data.items[0]);
    } catch (e) {
      console.error(e);
    } finally {
      setLoadingAnalysis(false);
    }
  };

  const handleAction = async (action: 'start' | 'stop' | 'delete') => {
    try {
      if (action === 'stop') await api.post(`/grid/${params.id}/stop`);
      else if (action === 'start') await api.post(`/grid/${params.id}/start`);
      else if (action === 'delete') {
        if (!confirm('确定删除此机器人？')) return;
        await api.post(`/grid/${params.id}/stop`).catch(() => {});
        await api.del(`/grid/${params.id}/delete`);
        navigate('/setup/bot-type', { replace: true });
        return;
      }
      await loadBot();
    } catch (e: any) {
      setError(e.response?.data?.error || '操作失败');
    }
  };

  const formatPnl = (pnl: number) => {
    if (pnl > 0) return <span class="text-emerald-400 font-semibold">+{pnl.toFixed(2)}</span>;
    if (pnl < 0) return <span class="text-red-400 font-semibold">{pnl.toFixed(2)}</span>;
    return <span class="text-on-surface-tertiary">0.00</span>;
  };

  const statusConfig = (status: string) => {
    const map: Record<string, { text: string; dot: string; bg: string }> = {
      running: { text: '运行中', dot: 'bg-emerald-500', bg: 'bg-emerald-500/10 text-emerald-400' },
      paused: { text: '已暂停', dot: 'bg-amber-500', bg: 'bg-amber-500/10 text-amber-400' },
      stopped: { text: '已停止', dot: 'bg-on-surface-muted', bg: 'bg-surface-2 text-on-surface-tertiary' },
      draft: { text: '草稿', dot: 'bg-on-surface-faint', bg: 'bg-surface-2 text-on-surface-tertiary' },
    };
    return map[status] || { text: status, dot: 'bg-on-surface-muted', bg: 'bg-surface-2 text-on-surface-tertiary' };
  };

  onMount(() => {
    loadBot();
    loadTrades();
    const interval = setInterval(() => { loadBot(); }, 10000);
    onCleanup(() => clearInterval(interval));
  });

  return (
    <div class="h-screen bg-base flex flex-col relative overflow-hidden">
      {/* Background glow */}
      <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      <Show when={bot()} fallback={
        <div class="flex-1 flex items-center justify-center relative z-10">
          <Show when={!error()} fallback={<div class="text-red-400 text-sm">{error()}</div>}>
            <svg class="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
          </Show>
        </div>
      }>
        {(b) => {
          return (
            <>
              {/* Top bar */}
              <div class="relative z-10 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle">
                <div class="flex items-center gap-2 md:gap-3">
                  <button
                    onClick={() => navigate('/setup/bot-type', { replace: true })}
                    class="p-1.5 rounded-lg hover:bg-surface-2 text-on-surface-tertiary hover:text-on-surface-secondary transition-colors"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" /></svg>
                  </button>
                  <div class="flex items-center gap-2">
                    <div class="w-7 h-7 md:w-8 md:h-8 rounded-lg bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 flex items-center justify-center">
                      <span class="text-xs md:text-sm font-extralight tracking-[0.2em] text-on-base">V</span>
                    </div>
                    <h1 class="text-sm md:text-lg font-extralight tracking-wide text-on-base">{b().name}</h1>
                    <span class={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${statusConfig(b().status).bg}`}>
                      <span class={`w-1 h-1 rounded-full ${statusConfig(b().status).dot} ${b().status === 'running' ? 'animate-pulse' : ''}`} />
                      {statusConfig(b().status).text}
                    </span>
                  </div>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-xs text-on-surface-tertiary hidden sm:inline">
                    {b().symbol} · {b().exchange.toUpperCase()} · {b().leverage}x
                  </span>
                  <Show when={b().status === 'running'}>
                    <button onClick={() => handleAction('stop')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-red-500/10 border border-red-500/20 text-red-400 hover:bg-red-500/20 transition-colors">
                      停止
                    </button>
                  </Show>
                  <Show when={b().status !== 'running' && b().status !== 'stopped'}>
                    <button onClick={() => handleAction('start')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20 transition-colors">
                      启动
                    </button>
                  </Show>
                  <button onClick={() => handleAction('delete')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-surface-1 border border-line-default text-on-surface-tertiary hover:text-red-400 hover:border-red-500/20 transition-colors">
                    删除
                  </button>
                  <button
                    onClick={toggleTheme}
                    class="p-2 rounded-lg text-on-surface-tertiary hover:text-on-surface-secondary hover:bg-surface-2 transition-colors"
                    title={isDark() ? 'Switch to light mode' : 'Switch to dark mode'}
                  >
                    <Show when={isDark()} fallback={
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" /></svg>
                    }>
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" /></svg>
                    </Show>
                  </button>
                </div>
              </div>

              {/* Content */}
              <div class="flex-1 overflow-y-auto relative z-10">
                <div class="max-w-7xl mx-auto px-4 md:px-8 py-6">

              <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 mb-8">
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">已实现盈亏</div>
                  <div class="text-lg font-mono">{formatPnl(b().total_pnl)}</div>
                </div>
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">未实现盈亏</div>
                  <div class="text-lg font-mono">{formatPnl(b().unrealized_pnl)}</div>
                </div>
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">成交次数</div>
                  <div class="text-lg font-mono text-on-surface">{b().total_trades}</div>
                </div>
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">已填充</div>
                  <div class="text-lg font-mono text-on-surface">{Math.min(b().grid_filled_count, b().grid_count)}<span class="text-on-surface-tertiary text-sm">/{b().grid_count}</span></div>
                </div>
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">利润率</div>
                  <div class="text-lg font-mono text-on-surface">{b().grid_profit_pct}<span class="text-on-surface-tertiary text-sm">%</span></div>
                </div>
                <div class="rounded-xl border border-line-default bg-surface-1 p-4 shadow-sm">
                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1">价格区间</div>
                  <div class="text-sm font-mono text-on-surface mt-0.5">
                    {b().upper_price > 0 ? (
                      <>{b().lower_price.toFixed(0)}<span class="text-on-surface-tertiary"> — </span>{b().upper_price.toFixed(0)}</>
                    ) : '-'}
                  </div>
                </div>
              </div>

              <div class="flex items-center gap-1 mb-6 border-b border-line-default">
                <For each={[
                  { key: 'levels' as const, label: '网格层级' },
                  { key: 'trades' as const, label: '交易记录' },
                  { key: 'analysis' as const, label: 'AI 分析' },
                ]}>
                  {(tab) => (
                    <button
                      onClick={() => {
                        setActiveTab(tab.key);
                        if (tab.key === 'analysis' && analysisLogs().length === 0) loadAnalysis();
                      }}
                      class={`px-4 py-2.5 text-xs font-medium transition-colors border-b-2 -mb-px ${
                        activeTab() === tab.key
                          ? 'text-on-base border-indigo-500'
                          : 'text-on-surface-tertiary border-transparent hover:text-on-surface-secondary'
                      }`}
                    >
                      {tab.label}
                    </button>
                  )}
                </For>
              </div>

              <Show when={activeTab() === 'levels'}>
                <Show when={!loadingTrades()} fallback={
                  <div class="flex justify-center py-12">
                    <svg class="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={gridLevels().length > 0} fallback={
                    <div class="text-center py-12 text-on-surface-tertiary text-sm">暂无网格数据</div>
                  }>
                    <div class="rounded-xl border border-line-default bg-surface-1 overflow-hidden shadow-sm">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
                            <th class="text-left px-4 py-2.5 font-medium">层级</th>
                            <th class="text-center px-3 py-2.5 font-medium">方向</th>
                            <th class="text-right px-3 py-2.5 font-medium">开仓价</th>
                            <th class="text-right px-3 py-2.5 font-medium">平仓价</th>
                            <th class="text-right px-3 py-2.5 font-medium">成交价</th>
                            <th class="text-right px-3 py-2.5 font-medium">持仓</th>
                            <th class="text-center px-3 py-2.5 font-medium">状态</th>
                          </tr>
                        </thead>
                        <tbody>
                          <For each={gridLevels()}>
                            {(level) => {
                              const isHolding = Math.abs(level.hold_quantity) > 0;
                              const isClosed = level.buy_filled && level.sell_filled && Math.abs(level.hold_quantity) === 0;
                              return (
                                <tr class={`border-b border-line-subtle ${isHolding ? 'bg-emerald-500/5' : isClosed ? 'bg-base-secondary/50' : ''}`}>
                                  <td class="px-4 py-2 text-on-surface-secondary font-mono">{level.level}</td>
                                  <td class="px-3 py-2 text-center">
                                    <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${level.side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
                                      {level.side === 'buy' ? '多' : '空'}
                                    </span>
                                  </td>
                                  <td class="px-3 py-2 text-emerald-400 text-right font-mono">{level.buy_price.toFixed(2)}</td>
                                  <td class="px-3 py-2 text-red-400 text-right font-mono">{level.sell_price.toFixed(2)}</td>
                                  <td class="px-3 py-2 text-on-surface text-right font-mono">{level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}</td>
                                  <td class="px-3 py-2 text-on-surface-secondary text-right font-mono">
                                    {Math.abs(level.hold_quantity) > 0 ? `${formatSmart(Math.abs(level.hold_quantity))}` : '-'}
                                  </td>
                                  <td class="px-3 py-2 text-center">
                                    {isClosed ? (
                                      <span class="inline-block w-2 h-2 rounded-full bg-on-surface-muted" title="已平仓" />
                                    ) : isHolding ? (
                                      <span class="inline-block w-2 h-2 rounded-full bg-emerald-500" title="持仓中" />
                                    ) : (
                                      <span class="inline-block w-2 h-2 rounded-full bg-on-surface-faint" title="等待" />
                                    )}
                                  </td>
                                </tr>
                              );
                            }}
                          </For>
                        </tbody>
                      </table>
                    </div>
                  </Show>
                </Show>
              </Show>

              <Show when={activeTab() === 'trades'}>
                <Show when={!loadingTrades()} fallback={
                  <div class="flex justify-center py-12">
                    <svg class="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={trades().length > 0} fallback={
                    <div class="text-center py-12 text-on-surface-tertiary text-sm">暂无交易记录</div>
                  }>
                    <div class="rounded-xl border border-line-default bg-surface-1 overflow-hidden shadow-sm">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
                            <th class="text-right px-4 py-2.5 font-medium">层级</th>
                            <th class="text-left px-3 py-2.5 font-medium">开仓</th>
                            <th class="text-right px-3 py-2.5 font-medium">开仓价</th>
                            <th class="text-right px-3 py-2.5 font-medium">开仓量</th>
                            <th class="text-left px-3 py-2.5 font-medium">平仓</th>
                            <th class="text-right px-3 py-2.5 font-medium">平仓价</th>
                            <th class="text-right px-3 py-2.5 font-medium">平仓量</th>
                            <th class="text-right px-3 py-2.5 font-medium">盈亏</th>
                            <th class="text-left px-3 py-2.5 font-medium">状态</th>
                            <th class="text-left px-3 py-2.5 font-medium">时间</th>
                          </tr>
                        </thead>
                        <tbody>
                          <For each={trades()}>
                            {(trade) => (
                              <tr class="border-b border-line-subtle hover:bg-base-secondary">
                                <td class="px-4 py-2.5 text-on-surface-secondary text-right font-mono">{trade.grid_level}</td>
                                <td class={`px-3 py-2.5 ${trade.open_side === 'buy' ? 'text-emerald-400' : 'text-red-400'}`}>
                                  {trade.open_side === 'buy' ? '买入' : '卖出'}
                                </td>
                                <td class="px-3 py-2.5 text-on-surface text-right font-mono">{trade.open_price.toFixed(2)}</td>
                                <td class="px-3 py-2.5 text-on-surface-tertiary text-right font-mono">{trade.open_quantity.toFixed(6)}</td>
                                <td class={`px-3 py-2.5 ${trade.close_side === 'buy' ? 'text-emerald-400' : trade.close_side === 'sell' ? 'text-red-400' : 'text-on-surface-tertiary'}`}>
                                  {trade.close_side === 'buy' ? '买入' : trade.close_side === 'sell' ? '卖出' : '-'}
                                </td>
                                <td class="px-3 py-2.5 text-on-surface text-right font-mono">{trade.close_price != null ? trade.close_price.toFixed(2) : '-'}</td>
                                <td class="px-3 py-2.5 text-on-surface-tertiary text-right font-mono">{trade.close_quantity != null ? trade.close_quantity.toFixed(6) : '-'}</td>
                                <td class="px-3 py-2.5 text-right font-mono">{trade.close_side ? formatPnl(trade.pnl) : '-'}</td>
                                <td class="px-3 py-2.5">
                                  <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                                    trade.status === 'open' ? 'bg-blue-500/10 text-blue-400'
                                    : trade.status === 'orphaned' ? 'bg-amber-500/10 text-amber-400'
                                    : 'bg-surface-2 text-on-surface-tertiary'
                                  }`}>
                                    {trade.status === 'open' ? '持仓中' : trade.status === 'orphaned' ? '异常' : '已平仓'}
                                  </span>
                                </td>
                                <td class="px-3 py-2.5 text-on-surface-tertiary text-[11px]">
                                  {new Date(trade.status === 'open' ? trade.opened_at : (trade.closed_at || trade.opened_at)).toLocaleString('zh-CN')}
                                </td>
                              </tr>
                            )}
                          </For>
                        </tbody>
                      </table>
                    </div>
                  </Show>
                </Show>
              </Show>

              <Show when={activeTab() === 'analysis'}>
                <Show when={!loadingAnalysis()} fallback={
                  <div class="flex justify-center py-12">
                    <svg class="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={analysisLogs().length > 0} fallback={
                    <div class="text-center py-12 text-on-surface-tertiary text-sm">暂无分析记录</div>
                  }>
                    <div class="grid grid-cols-1 lg:grid-cols-12 gap-4">
                      <div class="lg:col-span-4 space-y-2 max-h-[70vh] overflow-auto">
                        <For each={analysisLogs()}>
                          {(log) => (
                            <div
                              class={`p-3 rounded-lg border cursor-pointer transition-all ${
                                selectedAnalysis()?.id === log.id
                                  ? 'border-indigo-500/20 bg-indigo-500/10'
                                  : 'border-line-default bg-surface-1 hover:border-line-strong'
                              }`}
                              onClick={() => setSelectedAnalysis(log)}
                            >
                              <div class="flex items-center gap-2 mb-1.5">
                                <span class={`px-1.5 py-0.5 text-[10px] rounded font-medium ${
                                  log.analysis_type === 'initial' ? 'bg-blue-500/10 text-blue-400' : 'bg-purple-500/10 text-purple-400'
                                }`}>
                                  {log.analysis_type === 'initial' ? '首次' : '周期'}
                                </span>
                                <span class={`px-1.5 py-0.5 text-[10px] rounded font-medium ${
                                  log.status === 'completed' ? 'bg-emerald-500/10 text-emerald-400'
                                  : log.status === 'pending' ? 'bg-amber-500/10 text-amber-400'
                                  : 'bg-red-500/10 text-red-400'
                                }`}>
                                  {log.status === 'completed' ? '完成' : log.status === 'pending' ? '处理中' : '失败'}
                                </span>
                                <span class="text-[10px] text-on-surface-tertiary">{new Date(log.created_at).toLocaleString('zh-CN')}</span>
                              </div>
                              <Show when={log.status === 'completed' && log.result?.decision?.action}>
                                <div class="text-xs text-on-surface-tertiary truncate">
                                  {log.result.decision.action} — {log.result.decision.reason || ''}
                                </div>
                              </Show>
                            </div>
                          )}
                        </For>
                      </div>

                      <div class="lg:col-span-8">
                        <Show when={selectedAnalysis()}>
                          {(log) => (
                            <div class="rounded-xl border border-line-default bg-surface-1 p-5 space-y-4 shadow-sm">
                              <div class="flex items-center justify-between">
                                <h3 class="text-sm font-medium text-on-surface">分析详情</h3>
                                <span class="text-[10px] text-on-surface-tertiary">{new Date(log().created_at).toLocaleString('zh-CN')}</span>
                              </div>
                              <Show when={log().system_prompt}>
                                <div>
                                  <button onClick={() => setShowSystemPrompt(!showSystemPrompt())} class="flex items-center gap-1 text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5 hover:text-on-surface-secondary transition-colors">
                                    <svg class={`w-3 h-3 transition-transform ${showSystemPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                                    System Prompt
                                  </button>
                                  <Show when={showSystemPrompt()}>
                                    <pre class="p-3 bg-base-secondary rounded-lg text-[11px] text-on-surface-secondary whitespace-pre-wrap break-words max-h-48 overflow-auto border border-line-subtle">{log().system_prompt}</pre>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().user_prompt}>
                                <div>
                                  <button onClick={() => setShowUserPrompt(!showUserPrompt())} class="flex items-center gap-1 text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5 hover:text-on-surface-secondary transition-colors">
                                    <svg class={`w-3 h-3 transition-transform ${showUserPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                                    User Prompt
                                  </button>
                                  <Show when={showUserPrompt()}>
                                    <pre class="p-3 bg-base-secondary rounded-lg text-[11px] text-on-surface-secondary whitespace-pre-wrap break-words max-h-48 overflow-auto border border-line-subtle">{log().user_prompt}</pre>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().result?.decision}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">决策</div>
                                  <div class="flex items-center gap-3">
                                    <span class={`px-2 py-0.5 text-xs rounded font-medium ${
                                      log().result.decision.action === 'hold' ? 'bg-surface-2 text-on-surface-secondary'
                                      : log().result.decision.action === 'pause_grid' ? 'bg-red-500/10 text-red-400'
                                      : log().result.decision.action === 'resume_grid' ? 'bg-emerald-500/10 text-emerald-400'
                                      : log().result.decision.action === 'adjust_grid' ? 'bg-blue-500/10 text-blue-400'
                                      : log().result.decision.action === 'reduce_position' ? 'bg-amber-500/10 text-amber-400'
                                      : log().result.decision.action === 'cancel_order' ? 'bg-orange-500/10 text-orange-400'
                                      : 'bg-surface-2 text-on-surface-secondary'
                                    }`}>{log().result.decision.action}</span>
                                    <Show when={log().result.decision.confidence != null}>
                                      <span class="text-[10px] text-on-surface-tertiary">置信度 {(log().result.decision.confidence * 100).toFixed(0)}%</span>
                                    </Show>
                                  </div>
                                  <Show when={log().result.decision.reason}>
                                    <p class="text-xs text-on-surface-secondary mt-1">{log().result.decision.reason}</p>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().result?.grid && (log().result.grid.upper_price || log().result.grid.lower_price || log().result.grid.grid_count || log().result.grid.grid_profit_pct)}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">网格参数</div>
                                  <div class="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                                    <Show when={log().result.grid.upper_price}><span>上界 {log().result.grid.upper_price}</span></Show>
                                    <Show when={log().result.grid.lower_price}><span>下界 {log().result.grid.lower_price}</span></Show>
                                    <Show when={log().result.grid.grid_count}><span>层数 {log().result.grid.grid_count}</span></Show>
                                    <Show when={log().result.grid.grid_profit_pct}><span>利润率 {log().result.grid.grid_profit_pct}%</span></Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.risk && (log().result.risk.leverage || log().result.risk.quantity_per_grid)}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">风控参数</div>
                                  <div class="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                                    <Show when={log().result.risk.leverage}><span>杠杆 {log().result.risk.leverage}x</span></Show>
                                    <Show when={log().result.risk.quantity_per_grid}><span>每格 {log().result.risk.quantity_per_grid} USDT</span></Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.market}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">市场状态</div>
                                  <div class="space-y-1">
                                    <Show when={log().result.market.market_regime}>
                                      <span class={`px-2 py-0.5 text-xs rounded font-medium ${
                                        log().result.market.market_regime === 'ranging' ? 'bg-blue-500/10 text-blue-400'
                                        : log().result.market.market_regime === 'trending_up' ? 'bg-emerald-500/10 text-emerald-400'
                                        : log().result.market.market_regime === 'trending_down' ? 'bg-red-500/10 text-red-400'
                                        : log().result.market.market_regime === 'volatile' ? 'bg-amber-500/10 text-amber-400'
                                        : 'bg-surface-2 text-on-surface-secondary'
                                      }`}>{log().result.market.market_regime}</span>
                                    </Show>
                                    <Show when={log().result.market.funding_rate_warning}>
                                      <p class="text-xs text-amber-400">⚠ 资金费率: {log().result.market.funding_rate_warning}</p>
                                    </Show>
                                    <Show when={log().result.market.event_impact}>
                                      <p class="text-xs text-purple-400">📋 事件影响: {log().result.market.event_impact}</p>
                                    </Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.analysis}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">AI 分析</div>
                                  <p class="text-xs text-on-surface leading-relaxed whitespace-pre-wrap">{log().result.analysis}</p>
                                </div>
                              </Show>
                              <Show when={log().result?.risk_warning}>
                                <div>
                                  <div class="text-[10px] text-amber-400 uppercase tracking-wider mb-1.5">风险提示</div>
                                  <p class="text-xs text-amber-400">{log().result.risk_warning}</p>
                                </div>
                              </Show>
                              <Show when={log().result?.raw_llm_response}>
                                <div>
                                  <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">LLM 原始响应</div>
                                  <pre class="p-3 bg-indigo-500/10 rounded-lg text-[11px] text-indigo-400 whitespace-pre-wrap break-words max-h-64 overflow-auto border border-indigo-500/20">{JSON.stringify(log().result.raw_llm_response, null, 2)}</pre>
                                </div>
                              </Show>
                              <div>
                                <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">决策结果</div>
                                <pre class="p-3 bg-base-secondary rounded-lg text-[11px] text-on-surface-secondary whitespace-pre-wrap break-words max-h-64 overflow-auto border border-line-subtle">{JSON.stringify(log().result, null, 2)}</pre>
                              </div>
                              <Show when={log().error}>
                                <div>
                                  <div class="text-[10px] text-red-400 uppercase tracking-wider mb-1.5">错误</div>
                                  <pre class="p-3 bg-red-500/10 rounded-lg text-xs text-red-400 border border-red-500/20">{log().error}</pre>
                                </div>
                              </Show>
                            </div>
                          )}
                        </Show>
                        <Show when={!selectedAnalysis()}>
                          <div class="flex items-center justify-center h-64 text-on-surface-tertiary text-sm">
                            选择左侧记录查看详情
                          </div>
                        </Show>
                      </div>
                    </div>
                  </Show>
            </Show>
          </Show>
                </div>
              </div>
            </>
          );
        }}
      </Show>
    </div>
  );
}
