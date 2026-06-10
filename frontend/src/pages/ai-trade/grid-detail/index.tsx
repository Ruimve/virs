import { createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { useParams, useNavigate } from '@solidjs/router';
import { api } from '../../../lib/api';

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
    if (pnl > 0) return <span class="text-emerald-600 font-semibold">+{pnl.toFixed(2)}</span>;
    if (pnl < 0) return <span class="text-red-600 font-semibold">{pnl.toFixed(2)}</span>;
    return <span class="text-gray-400">0.00</span>;
  };

  const statusConfig = (status: string) => {
    const map: Record<string, { text: string; dot: string; bg: string }> = {
      running: { text: '运行中', dot: 'bg-emerald-500', bg: 'bg-emerald-50 text-emerald-700' },
      paused: { text: '已暂停', dot: 'bg-amber-500', bg: 'bg-amber-50 text-amber-700' },
      stopped: { text: '已停止', dot: 'bg-gray-400', bg: 'bg-gray-100 text-gray-500' },
      draft: { text: '草稿', dot: 'bg-gray-300', bg: 'bg-gray-100 text-gray-400' },
    };
    return map[status] || { text: status, dot: 'bg-gray-400', bg: 'bg-gray-100 text-gray-500' };
  };

  onMount(() => {
    loadBot();
    loadTrades();
    const interval = setInterval(() => { loadBot(); }, 10000);
    onCleanup(() => clearInterval(interval));
  });

  return (
    <div class="min-h-screen bg-gray-50 text-gray-900">
      <Show when={bot()} fallback={
        <div class="flex items-center justify-center h-96">
          <Show when={!error()} fallback={<div class="text-red-600 text-sm">{error()}</div>}>
            <svg class="animate-spin h-5 w-5 text-gray-400" viewBox="0 0 24 24" fill="none">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
          </Show>
        </div>
      }>
        {(b) => {
          return (
            <div class="max-w-7xl mx-auto px-6 py-8">
              <div class="flex items-center justify-between mb-8">
                <div class="flex items-center gap-3">
                  <button
                    onClick={() => navigate('/setup/bot-type', { replace: true })}
                    class="p-1.5 rounded-lg hover:bg-gray-100 text-gray-400 hover:text-gray-600 transition-colors"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" /></svg>
                  </button>
                  <div>
                    <div class="flex items-center gap-2.5">
                      <h1 class="text-lg font-semibold text-gray-900">{b().name}</h1>
                      <span class={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium ${statusConfig(b().status).bg}`}>
                        <span class={`w-1.5 h-1.5 rounded-full ${statusConfig(b().status).dot} ${b().status === 'running' ? 'animate-pulse' : ''}`} />
                        {statusConfig(b().status).text}
                      </span>
                    </div>
                    <div class="flex items-center gap-2 mt-0.5 text-xs text-gray-500">
                      <span class="text-gray-700 font-medium">{b().symbol}</span>
                      <span class="text-gray-300">·</span>
                      <span>{b().exchange.toUpperCase()}</span>
                      <span class="text-gray-300">·</span>
                      <span>{b().leverage}x</span>
                      <Show when={b().market_regime}>
                        <span class="text-gray-300">·</span>
                        <span class="text-indigo-500">{b().market_regime}</span>
                      </Show>
                    </div>
                  </div>
                </div>

                <div class="flex items-center gap-2">
                  <Show when={b().status === 'running'}>
                    <button onClick={() => handleAction('stop')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-red-50 border border-red-200 text-red-600 hover:bg-red-100 transition-colors">
                      停止
                    </button>
                  </Show>
                  <Show when={b().status !== 'running' && b().status !== 'stopped'}>
                    <button onClick={() => handleAction('start')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-emerald-50 border border-emerald-200 text-emerald-700 hover:bg-emerald-100 transition-colors">
                      启动
                    </button>
                  </Show>
                  <button onClick={() => handleAction('delete')} class="px-3 py-1.5 rounded-lg text-xs font-medium bg-white border border-gray-200 text-gray-500 hover:text-red-600 hover:border-red-200 transition-colors">
                    删除
                  </button>
                </div>
              </div>

              <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 mb-8">
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">已实现盈亏</div>
                  <div class="text-lg font-mono">{formatPnl(b().total_pnl)}</div>
                </div>
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">未实现盈亏</div>
                  <div class="text-lg font-mono">{formatPnl(b().unrealized_pnl)}</div>
                </div>
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">成交次数</div>
                  <div class="text-lg font-mono text-gray-700">{b().total_trades}</div>
                </div>
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">已填充</div>
                  <div class="text-lg font-mono text-gray-700">{Math.min(b().grid_filled_count, b().grid_count)}<span class="text-gray-400 text-sm">/{b().grid_count}</span></div>
                </div>
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">利润率</div>
                  <div class="text-lg font-mono text-gray-700">{b().grid_profit_pct}<span class="text-gray-400 text-sm">%</span></div>
                </div>
                <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">价格区间</div>
                  <div class="text-sm font-mono text-gray-700 mt-0.5">
                    {b().upper_price > 0 ? (
                      <>{b().lower_price.toFixed(0)}<span class="text-gray-400"> — </span>{b().upper_price.toFixed(0)}</>
                    ) : '-'}
                  </div>
                </div>
              </div>

              <div class="flex items-center gap-1 mb-6 border-b border-gray-200">
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
                          ? 'text-gray-900 border-indigo-500'
                          : 'text-gray-400 border-transparent hover:text-gray-600'
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
                    <svg class="animate-spin h-5 w-5 text-gray-400" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={gridLevels().length > 0} fallback={
                    <div class="text-center py-12 text-gray-400 text-sm">暂无网格数据</div>
                  }>
                    <div class="rounded-xl border border-gray-200 bg-white overflow-hidden shadow-sm">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-gray-500 border-b border-gray-100 bg-gray-50">
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
                                <tr class={`border-b border-gray-50 ${isHolding ? 'bg-emerald-50/50' : isClosed ? 'bg-gray-50/50' : ''}`}>
                                  <td class="px-4 py-2 text-gray-600 font-mono">{level.level}</td>
                                  <td class="px-3 py-2 text-center">
                                    <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${level.side === 'buy' ? 'bg-emerald-50 text-emerald-700' : 'bg-red-50 text-red-700'}`}>
                                      {level.side === 'buy' ? '多' : '空'}
                                    </span>
                                  </td>
                                  <td class="px-3 py-2 text-emerald-600 text-right font-mono">{level.buy_price.toFixed(2)}</td>
                                  <td class="px-3 py-2 text-red-500 text-right font-mono">{level.sell_price.toFixed(2)}</td>
                                  <td class="px-3 py-2 text-gray-700 text-right font-mono">{level.last_fill_price > 0 ? level.last_fill_price.toFixed(2) : '-'}</td>
                                  <td class="px-3 py-2 text-gray-600 text-right font-mono">
                                    {Math.abs(level.hold_quantity) > 0 ? `${formatSmart(Math.abs(level.hold_quantity))}` : '-'}
                                  </td>
                                  <td class="px-3 py-2 text-center">
                                    {isClosed ? (
                                      <span class="inline-block w-2 h-2 rounded-full bg-gray-400" title="已平仓" />
                                    ) : isHolding ? (
                                      <span class="inline-block w-2 h-2 rounded-full bg-emerald-500" title="持仓中" />
                                    ) : (
                                      <span class="inline-block w-2 h-2 rounded-full bg-gray-300" title="等待" />
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
                    <svg class="animate-spin h-5 w-5 text-gray-400" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={trades().length > 0} fallback={
                    <div class="text-center py-12 text-gray-400 text-sm">暂无交易记录</div>
                  }>
                    <div class="rounded-xl border border-gray-200 bg-white overflow-hidden shadow-sm">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-gray-500 border-b border-gray-100 bg-gray-50">
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
                              <tr class="border-b border-gray-50 hover:bg-gray-50">
                                <td class="px-4 py-2.5 text-gray-600 text-right font-mono">{trade.grid_level}</td>
                                <td class={`px-3 py-2.5 ${trade.open_side === 'buy' ? 'text-emerald-600' : 'text-red-600'}`}>
                                  {trade.open_side === 'buy' ? '买入' : '卖出'}
                                </td>
                                <td class="px-3 py-2.5 text-gray-700 text-right font-mono">{trade.open_price.toFixed(2)}</td>
                                <td class="px-3 py-2.5 text-gray-500 text-right font-mono">{trade.open_quantity.toFixed(6)}</td>
                                <td class={`px-3 py-2.5 ${trade.close_side === 'buy' ? 'text-emerald-600' : trade.close_side === 'sell' ? 'text-red-600' : 'text-gray-400'}`}>
                                  {trade.close_side === 'buy' ? '买入' : trade.close_side === 'sell' ? '卖出' : '-'}
                                </td>
                                <td class="px-3 py-2.5 text-gray-700 text-right font-mono">{trade.close_price != null ? trade.close_price.toFixed(2) : '-'}</td>
                                <td class="px-3 py-2.5 text-gray-500 text-right font-mono">{trade.close_quantity != null ? trade.close_quantity.toFixed(6) : '-'}</td>
                                <td class="px-3 py-2.5 text-right font-mono">{trade.close_side ? formatPnl(trade.pnl) : '-'}</td>
                                <td class="px-3 py-2.5">
                                  <span class={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                                    trade.status === 'open' ? 'bg-blue-50 text-blue-700'
                                    : trade.status === 'orphaned' ? 'bg-amber-50 text-amber-700'
                                    : 'bg-gray-100 text-gray-500'
                                  }`}>
                                    {trade.status === 'open' ? '持仓中' : trade.status === 'orphaned' ? '异常' : '已平仓'}
                                  </span>
                                </td>
                                <td class="px-3 py-2.5 text-gray-500 text-[11px]">
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
                    <svg class="animate-spin h-5 w-5 text-gray-400" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  </div>
                }>
                  <Show when={analysisLogs().length > 0} fallback={
                    <div class="text-center py-12 text-gray-400 text-sm">暂无分析记录</div>
                  }>
                    <div class="grid grid-cols-1 lg:grid-cols-12 gap-4">
                      <div class="lg:col-span-4 space-y-2 max-h-[70vh] overflow-auto">
                        <For each={analysisLogs()}>
                          {(log) => (
                            <div
                              class={`p-3 rounded-lg border cursor-pointer transition-all ${
                                selectedAnalysis()?.id === log.id
                                  ? 'border-indigo-200 bg-indigo-50'
                                  : 'border-gray-200 bg-white hover:border-gray-300'
                              }`}
                              onClick={() => setSelectedAnalysis(log)}
                            >
                              <div class="flex items-center gap-2 mb-1.5">
                                <span class={`px-1.5 py-0.5 text-[10px] rounded font-medium ${
                                  log.analysis_type === 'initial' ? 'bg-blue-50 text-blue-700' : 'bg-purple-50 text-purple-700'
                                }`}>
                                  {log.analysis_type === 'initial' ? '首次' : '周期'}
                                </span>
                                <span class={`px-1.5 py-0.5 text-[10px] rounded font-medium ${
                                  log.status === 'completed' ? 'bg-emerald-50 text-emerald-700'
                                  : log.status === 'pending' ? 'bg-amber-50 text-amber-700'
                                  : 'bg-red-50 text-red-700'
                                }`}>
                                  {log.status === 'completed' ? '完成' : log.status === 'pending' ? '处理中' : '失败'}
                                </span>
                                <span class="text-[10px] text-gray-400">{new Date(log.created_at).toLocaleString('zh-CN')}</span>
                              </div>
                              <Show when={log.status === 'completed' && log.result?.decision?.action}>
                                <div class="text-xs text-gray-500 truncate">
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
                            <div class="rounded-xl border border-gray-200 bg-white p-5 space-y-4 shadow-sm">
                              <div class="flex items-center justify-between">
                                <h3 class="text-sm font-medium text-gray-800">分析详情</h3>
                                <span class="text-[10px] text-gray-400">{new Date(log().created_at).toLocaleString('zh-CN')}</span>
                              </div>
                              <Show when={log().system_prompt}>
                                <div>
                                  <button onClick={() => setShowSystemPrompt(!showSystemPrompt())} class="flex items-center gap-1 text-[10px] text-gray-400 uppercase tracking-wider mb-1.5 hover:text-gray-600 transition-colors">
                                    <svg class={`w-3 h-3 transition-transform ${showSystemPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                                    System Prompt
                                  </button>
                                  <Show when={showSystemPrompt()}>
                                    <pre class="p-3 bg-gray-50 rounded-lg text-[11px] text-gray-600 whitespace-pre-wrap break-words max-h-48 overflow-auto border border-gray-100">{log().system_prompt}</pre>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().user_prompt}>
                                <div>
                                  <button onClick={() => setShowUserPrompt(!showUserPrompt())} class="flex items-center gap-1 text-[10px] text-gray-400 uppercase tracking-wider mb-1.5 hover:text-gray-600 transition-colors">
                                    <svg class={`w-3 h-3 transition-transform ${showUserPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
                                    User Prompt
                                  </button>
                                  <Show when={showUserPrompt()}>
                                    <pre class="p-3 bg-gray-50 rounded-lg text-[11px] text-gray-600 whitespace-pre-wrap break-words max-h-48 overflow-auto border border-gray-100">{log().user_prompt}</pre>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().result?.decision}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">决策</div>
                                  <div class="flex items-center gap-3">
                                    <span class={`px-2 py-0.5 text-xs rounded font-medium ${
                                      log().result.decision.action === 'hold' ? 'bg-gray-100 text-gray-600'
                                      : log().result.decision.action === 'pause_grid' ? 'bg-red-50 text-red-600'
                                      : log().result.decision.action === 'resume_grid' ? 'bg-emerald-50 text-emerald-600'
                                      : log().result.decision.action === 'adjust_grid' ? 'bg-blue-50 text-blue-600'
                                      : log().result.decision.action === 'reduce_position' ? 'bg-amber-50 text-amber-600'
                                      : log().result.decision.action === 'cancel_order' ? 'bg-orange-50 text-orange-600'
                                      : 'bg-gray-100 text-gray-600'
                                    }`}>{log().result.decision.action}</span>
                                    <Show when={log().result.decision.confidence != null}>
                                      <span class="text-[10px] text-gray-400">置信度 {(log().result.decision.confidence * 100).toFixed(0)}%</span>
                                    </Show>
                                  </div>
                                  <Show when={log().result.decision.reason}>
                                    <p class="text-xs text-gray-600 mt-1">{log().result.decision.reason}</p>
                                  </Show>
                                </div>
                              </Show>
                              <Show when={log().result?.grid && (log().result.grid.upper_price || log().result.grid.lower_price || log().result.grid.grid_count || log().result.grid.grid_profit_pct)}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">网格参数</div>
                                  <div class="flex flex-wrap gap-3 text-xs text-gray-600">
                                    <Show when={log().result.grid.upper_price}><span>上界 {log().result.grid.upper_price}</span></Show>
                                    <Show when={log().result.grid.lower_price}><span>下界 {log().result.grid.lower_price}</span></Show>
                                    <Show when={log().result.grid.grid_count}><span>层数 {log().result.grid.grid_count}</span></Show>
                                    <Show when={log().result.grid.grid_profit_pct}><span>利润率 {log().result.grid.grid_profit_pct}%</span></Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.risk && (log().result.risk.leverage || log().result.risk.quantity_per_grid)}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">风控参数</div>
                                  <div class="flex flex-wrap gap-3 text-xs text-gray-600">
                                    <Show when={log().result.risk.leverage}><span>杠杆 {log().result.risk.leverage}x</span></Show>
                                    <Show when={log().result.risk.quantity_per_grid}><span>每格 {log().result.risk.quantity_per_grid} USDT</span></Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.market}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">市场状态</div>
                                  <div class="space-y-1">
                                    <Show when={log().result.market.market_regime}>
                                      <span class={`px-2 py-0.5 text-xs rounded font-medium ${
                                        log().result.market.market_regime === 'ranging' ? 'bg-blue-50 text-blue-600'
                                        : log().result.market.market_regime === 'trending_up' ? 'bg-emerald-50 text-emerald-600'
                                        : log().result.market.market_regime === 'trending_down' ? 'bg-red-50 text-red-600'
                                        : log().result.market.market_regime === 'volatile' ? 'bg-amber-50 text-amber-600'
                                        : 'bg-gray-100 text-gray-600'
                                      }`}>{log().result.market.market_regime}</span>
                                    </Show>
                                    <Show when={log().result.market.funding_rate_warning}>
                                      <p class="text-xs text-amber-700">⚠ 资金费率: {log().result.market.funding_rate_warning}</p>
                                    </Show>
                                    <Show when={log().result.market.event_impact}>
                                      <p class="text-xs text-purple-700">📋 事件影响: {log().result.market.event_impact}</p>
                                    </Show>
                                  </div>
                                </div>
                              </Show>
                              <Show when={log().result?.analysis}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">AI 分析</div>
                                  <p class="text-xs text-gray-700 leading-relaxed whitespace-pre-wrap">{log().result.analysis}</p>
                                </div>
                              </Show>
                              <Show when={log().result?.risk_warning}>
                                <div>
                                  <div class="text-[10px] text-amber-600 uppercase tracking-wider mb-1.5">风险提示</div>
                                  <p class="text-xs text-amber-700">{log().result.risk_warning}</p>
                                </div>
                              </Show>
                              <Show when={log().result?.raw_llm_response}>
                                <div>
                                  <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">LLM 原始响应</div>
                                  <pre class="p-3 bg-indigo-50 rounded-lg text-[11px] text-indigo-700 whitespace-pre-wrap break-words max-h-64 overflow-auto border border-indigo-100">{JSON.stringify(log().result.raw_llm_response, null, 2)}</pre>
                                </div>
                              </Show>
                              <div>
                                <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1.5">决策结果</div>
                                <pre class="p-3 bg-gray-50 rounded-lg text-[11px] text-gray-600 whitespace-pre-wrap break-words max-h-64 overflow-auto border border-gray-100">{JSON.stringify(log().result, null, 2)}</pre>
                              </div>
                              <Show when={log().error}>
                                <div>
                                  <div class="text-[10px] text-red-600 uppercase tracking-wider mb-1.5">错误</div>
                                  <pre class="p-3 bg-red-50 rounded-lg text-xs text-red-700 border border-red-100">{log().error}</pre>
                                </div>
                              </Show>
                            </div>
                          )}
                        </Show>
                        <Show when={!selectedAnalysis()}>
                          <div class="flex items-center justify-center h-64 text-gray-400 text-sm">
                            选择左侧记录查看详情
                          </div>
                        </Show>
                      </div>
                    </div>
                  </Show>
                </Show>
              </Show>
            </div>
          );
        }}
      </Show>
    </div>
  );
}
