import { createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { useParams, useNavigate } from '@solidjs/router';
import { api } from '../../lib/api';

interface AutoBot {
  id: string;
  name: string;
  symbol: string;
  exchange: string;
  market_type: string;
  status: string;
  leverage: number;
  max_position_pct: number;
  decide_interval_secs: number;
  current_side: string | null;
  entry_price: number;
  position_size: number;
  stop_loss: number;
  take_profit: number;
  unrealized_pnl: number;
  market_regime: string | null;
  ai_analysis: string | null;
  total_pnl: number;
  total_trades: number;
  win_trades: number;
  loss_trades: number;
  created_at: string;
}

interface AutoTrade {
  id: string;
  bot_id: string;
  symbol: string;
  exchange: string;
  side: string;
  trade_type: string;
  price: number;
  quantity: number;
  pnl: number;
  pnl_pct: number;
  created_at: string;
}

interface AnalysisLog {
  id: string;
  bot_id: string;
  analysis_type: string;
  status: string;
  system_prompt: string;
  user_prompt: string;
  result: any;
  error: string | null;
  created_at: string;
}

export default function AutoDetailPage() {
  const params = useParams();
  const navigate = useNavigate();

  const [bot, setBot] = createSignal<AutoBot | null>(null);
  const [trades, setTrades] = createSignal<AutoTrade[]>([]);
  const [logs, setLogs] = createSignal<AnalysisLog[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal('');
  const [activeTab, setActiveTab] = createSignal<'trades' | 'analysis'>('trades');
  const [expandedLogId, setExpandedLogId] = createSignal<string | null>(null);

  const loadBot = async () => {
    try {
      const res = await api.get<{ bot: AutoBot; trades: AutoTrade[] }>(`/auto/${params.id}`);
      if (res.data?.bot) {
        setBot(res.data.bot);
        setTrades(res.data.trades || []);
      } else {
        setError('机器人不存在或加载失败');
      }
    } catch (e) {
      console.error('Failed to load auto bot:', e);
      setError('加载失败');
    } finally {
      setLoading(false);
    }
  };

  const loadLogs = async () => {
    try {
      const res = await api.get<{ logs: AnalysisLog[] }>('/auto/analysis-logs?bot_id=' + params.id);
      if (res.data?.logs) setLogs(res.data.logs);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  };

  const handleAction = async (action: 'start' | 'stop' | 'delete') => {
    try {
      if (action === 'delete') {
        if (!confirm('确定删除此机器人？将平仓所有持仓。')) return;
        await api.del(`/auto/${params.id}/delete`);
        navigate('/ai-trade/auto');
        return;
      }
      await api.post(`/auto/${params.id}/${action}`);
      await loadBot();
    } catch (e: any) {
      setError(e.response?.data?.error || '操作失败');
    }
  };

  const statusConfig = (status: string) => {
    const map: Record<string, { text: string; dot: string; bg: string }> = {
      running: { text: '运行中', dot: 'bg-emerald-500', bg: 'bg-emerald-50 text-emerald-700' },
      paused: { text: '已暂停', dot: 'bg-amber-500', bg: 'bg-amber-50 text-amber-700' },
      stopped: { text: '已停止', dot: 'bg-gray-400', bg: 'bg-gray-100 text-gray-500' },
      draft: { text: '草稿', dot: 'bg-gray-300', bg: 'bg-gray-100 text-gray-400' },
      error: { text: '错误', dot: 'bg-red-500', bg: 'bg-red-50 text-red-700' },
    };
    return map[status] || { text: status, dot: 'bg-gray-400', bg: 'bg-gray-100 text-gray-500' };
  };

  const tradeTypeLabel = (t: string) => {
    const map: Record<string, string> = {
      open_long: '开多', open_short: '开空',
      close_long: '平多', close_short: '平空',
      stop_loss: '止损', take_profit: '止盈',
    };
    return map[t] || t;
  };

  const tradeTypeColor = (t: string) => {
    if (t.startsWith('open_long') || t === 'close_short') return 'text-emerald-600';
    if (t.startsWith('open_short') || t === 'close_long') return 'text-red-600';
    if (t === 'stop_loss') return 'text-red-500';
    if (t === 'take_profit') return 'text-emerald-500';
    return 'text-gray-500';
  };

  const formatPnl = (pnl: number) => {
    if (pnl > 0) return <span class="text-emerald-600 font-semibold">+{pnl.toFixed(4)}</span>;
    if (pnl < 0) return <span class="text-red-600 font-semibold">{pnl.toFixed(4)}</span>;
    return <span class="text-gray-400">0.00</span>;
  };

  const getDecisionFromLog = (log: AnalysisLog) => {
    try {
      const r = log.result;
      if (r?.decision?.action) return r.decision;
      if (r?.action) return { action: r.action, reason: r.reason || '', confidence: r.confidence || 0 };
      return null;
    } catch {
      return null;
    }
  };

  onMount(() => {
    loadBot();
    loadLogs();
    const interval = setInterval(() => { loadBot(); loadLogs(); }, 10000);
    onCleanup(() => clearInterval(interval));
  });

  return (
    <div class="min-h-screen bg-gray-50 text-gray-900">
      <div class="max-w-7xl mx-auto px-6 py-8">
        <Show when={!loading() && bot()} fallback={
          <div class="flex flex-col items-center justify-center py-20 gap-4">
            <Show when={!loading() && !bot()} fallback={
              <svg class="animate-spin h-6 w-6 text-gray-400" viewBox="0 0 24 24" fill="none">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            }>
              <div class="text-gray-400 text-sm">{error() || '机器人不存在或加载失败'}</div>
              <button
                onClick={() => navigate('/ai-trade/auto')}
                class="px-4 py-2 text-xs font-medium border border-gray-200 rounded-lg text-gray-500 hover:bg-gray-50 transition-colors"
              >
                返回列表
              </button>
            </Show>
          </div>
        }>
          {(() => {
            const b = bot()!;
            const sc = statusConfig(b.status);
            return (
              <>
                <div class="flex items-center justify-between mb-6">
                  <div class="flex items-center gap-3">
                    <button
                      onClick={() => navigate('/ai-trade/auto')}
                      class="p-1.5 rounded-lg hover:bg-gray-100 text-gray-400 hover:text-gray-600 transition-colors"
                    >
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
                      </svg>
                    </button>
                    <div>
                      <div class="flex items-center gap-2">
                        <h1 class="text-lg font-semibold text-gray-900">{b.name}</h1>
                        <span class={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${sc.bg}`}>
                          <span class={`w-1 h-1 rounded-full ${sc.dot}`} />
                          {sc.text}
                        </span>
                      </div>
                      <div class="flex items-center gap-2 mt-0.5 text-xs text-gray-500">
                        <span class="font-medium text-gray-700">{b.symbol}</span>
                        <span class="text-gray-300">·</span>
                        <span>{b.exchange.toUpperCase()}</span>
                        <span class="text-gray-300">·</span>
                        <span>{b.market_type === 'perpetual' ? '合约' : '现货'}</span>
                        <span class="text-gray-300">·</span>
                        <span>{b.leverage}x</span>
                      </div>
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    <Show when={b.status === 'running'}>
                      <button
                        onClick={() => handleAction('stop')}
                        class="px-3 py-1.5 rounded-lg text-xs font-medium border border-red-200 text-red-600 hover:bg-red-50 transition-colors"
                      >
                        停止
                      </button>
                    </Show>
                    <Show when={b.status !== 'running'}>
                      <button
                        onClick={() => handleAction('start')}
                        class="px-3 py-1.5 rounded-lg text-xs font-medium border border-emerald-200 text-emerald-600 hover:bg-emerald-50 transition-colors"
                      >
                        启动
                      </button>
                    </Show>
                    <button
                      onClick={() => handleAction('delete')}
                      class="px-3 py-1.5 rounded-lg text-xs font-medium border border-gray-200 text-gray-500 hover:bg-red-50 hover:text-red-600 hover:border-red-200 transition-colors"
                    >
                      删除
                    </button>
                  </div>
                </div>

                <Show when={error()}>
                  <div class="mb-4 text-xs text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
                    {error()}
                  </div>
                </Show>

                <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
                  <div class="bg-white rounded-xl border border-gray-200 p-5 shadow-sm">
                    <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-3">仓位信息</div>
                    <Show when={b.current_side && b.current_side !== 'none'} fallback={
                      <div class="text-sm text-gray-400">无持仓</div>
                    }>
                      <div class="space-y-2">
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">方向</span>
                          <span class={b.current_side === 'long' ? 'text-emerald-600 font-medium' : 'text-red-600 font-medium'}>
                            {b.current_side === 'long' ? '做多' : '做空'}
                          </span>
                        </div>
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">入场价</span>
                          <span class="text-gray-700 font-mono">{b.entry_price.toFixed(2)}</span>
                        </div>
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">持仓量</span>
                          <span class="text-gray-700 font-mono">{b.position_size.toFixed(6)}</span>
                        </div>
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">止损</span>
                          <span class="text-red-500 font-mono">{b.stop_loss > 0 ? b.stop_loss.toFixed(2) : '-'}</span>
                        </div>
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">止盈</span>
                          <span class="text-emerald-500 font-mono">{b.take_profit > 0 ? b.take_profit.toFixed(2) : '-'}</span>
                        </div>
                        <div class="flex justify-between text-xs pt-2 border-t border-gray-100">
                          <span class="text-gray-500">未实现盈亏</span>
                          {formatPnl(b.unrealized_pnl)}
                        </div>
                      </div>
                    </Show>
                  </div>

                  <div class="bg-white rounded-xl border border-gray-200 p-5 shadow-sm">
                    <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-3">统计数据</div>
                    <div class="space-y-2">
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">累计盈亏</span>
                        {formatPnl(b.total_pnl)}
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">总交易次数</span>
                        <span class="text-gray-700 font-mono">{b.total_trades}</span>
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">盈利 / 亏损</span>
                        <span>
                          <span class="text-emerald-600 font-mono">{b.win_trades}</span>
                          <span class="text-gray-300 mx-1">/</span>
                          <span class="text-red-600 font-mono">{b.loss_trades}</span>
                        </span>
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">胜率</span>
                        <span class="text-gray-700 font-mono">
                          {b.total_trades > 0 ? ((b.win_trades / b.total_trades) * 100).toFixed(1) + '%' : '-'}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div class="bg-white rounded-xl border border-gray-200 p-5 shadow-sm">
                    <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-3">配置</div>
                    <div class="space-y-2">
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">市场类型</span>
                        <span class="text-gray-700">{b.market_type === 'perpetual' ? '合约' : '现货'}</span>
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">杠杆</span>
                        <span class="text-gray-700 font-mono">{b.leverage}x</span>
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">仓位占比</span>
                        <span class="text-gray-700 font-mono">{b.max_position_pct}%</span>
                      </div>
                      <div class="flex justify-between text-xs">
                        <span class="text-gray-500">决策周期</span>
                        <span class="text-gray-700 font-mono">{b.decide_interval_secs}s</span>
                      </div>
                      <Show when={b.market_regime}>
                        <div class="flex justify-between text-xs">
                          <span class="text-gray-500">市场状态</span>
                          <span class="text-indigo-500">{b.market_regime}</span>
                        </div>
                      </Show>
                    </div>
                  </div>
                </div>

                <Show when={b.ai_analysis}>
                  <div class="bg-white rounded-xl border border-gray-200 p-5 shadow-sm mb-6">
                    <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-2">AI 分析</div>
                    <p class="text-xs text-gray-600 leading-relaxed whitespace-pre-wrap">{b.ai_analysis}</p>
                  </div>
                </Show>

                <div class="bg-white rounded-xl border border-gray-200 shadow-sm overflow-hidden">
                  <div class="flex border-b border-gray-100">
                    <button
                      onClick={() => setActiveTab('trades')}
                      class={`px-5 py-3 text-xs font-medium transition-colors ${
                        activeTab() === 'trades'
                          ? 'text-indigo-600 border-b-2 border-indigo-600'
                          : 'text-gray-500 hover:text-gray-700'
                      }`}
                    >
                      交易记录 ({trades().length})
                    </button>
                    <button
                      onClick={() => setActiveTab('analysis')}
                      class={`px-5 py-3 text-xs font-medium transition-colors ${
                        activeTab() === 'analysis'
                          ? 'text-indigo-600 border-b-2 border-indigo-600'
                          : 'text-gray-500 hover:text-gray-700'
                      }`}
                    >
                      AI 决策日志 ({logs().length})
                    </button>
                  </div>

                  <Show when={activeTab() === 'trades'}>
                    <Show when={trades().length > 0} fallback={
                      <div class="text-center py-12 text-gray-400 text-xs">暂无交易记录</div>
                    }>
                      <div class="divide-y divide-gray-50">
                        <For each={trades().slice(0, 50)}>
                          {(t) => (
                            <div class="flex items-center justify-between px-5 py-3 hover:bg-gray-50/50">
                              <div class="flex items-center gap-3">
                                <span class={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-gray-50`}>
                                  {tradeTypeLabel(t.trade_type)}
                                </span>
                                <div>
                                  <div class="text-xs text-gray-700 font-mono">
                                    {t.side === 'buy' ? '买入' : '卖出'} {t.quantity.toFixed(6)} @ {t.price.toFixed(2)}
                                  </div>
                                  <div class="text-[10px] text-gray-400 mt-0.5">
                                    {new Date(t.created_at).toLocaleString('zh-CN')}
                                  </div>
                                </div>
                              </div>
                              <Show when={t.pnl !== 0}>
                                <div class="text-right">
                                  {formatPnl(t.pnl)}
                                  <Show when={t.pnl_pct !== 0}>
                                    <div class="text-[10px] text-gray-400">{t.pnl_pct.toFixed(2)}%</div>
                                  </Show>
                                </div>
                              </Show>
                            </div>
                          )}
                        </For>
                      </div>
                    </Show>
                  </Show>

                  <Show when={activeTab() === 'analysis'}>
                    <Show when={logs().length > 0} fallback={
                      <div class="text-center py-12 text-gray-400 text-xs">暂无 AI 决策日志</div>
                    }>
                      <div class="divide-y divide-gray-50">
                        <For each={logs().slice(0, 50)}>
                          {(log) => {
                            const decision = getDecisionFromLog(log);
                            const rawResponse = log.result?.raw_llm_response;
                            return (
                              <div class="px-5 py-4">
                                <div class="flex items-center justify-between mb-2">
                                  <div class="flex items-center gap-2">
                                    <Show when={decision} fallback={
                                      <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-gray-100 text-gray-500">
                                        {log.status === 'failed' ? '失败' : '未知'}
                                      </span>
                                    }>
                                      {(d: any) => (
                                        <span class={`text-[10px] font-medium px-1.5 py-0.5 rounded ${
                                          d.action === 'hold' ? 'bg-gray-100 text-gray-500' :
                                          d.action === 'open_long' ? 'bg-emerald-50 text-emerald-600' :
                                          d.action === 'open_short' ? 'bg-red-50 text-red-600' :
                                          'bg-blue-50 text-blue-600'
                                        }`}>
                                          {d.action === 'open_long' ? '开多' :
                                           d.action === 'open_short' ? '开空' :
                                           d.action === 'close_position' ? '平仓' :
                                           d.action === 'hold' ? '持有' : d.action}
                                        </span>
                                      )}
                                    </Show>
                                    <Show when={decision?.confidence}>
                                      <span class="text-[10px] text-gray-400">
                                        置信度 {(decision.confidence * 100).toFixed(0)}%
                                      </span>
                                    </Show>
                                  </div>
                                  <div class="flex items-center gap-2">
                                    <button
                                      onClick={() => setExpandedLogId(expandedLogId() === log.id ? null : log.id)}
                                      class="text-[10px] text-gray-400 hover:text-indigo-500 transition-colors"
                                    >
                                      {expandedLogId() === log.id ? '收起详情' : '查看详情'}
                                    </button>
                                    <span class="text-[10px] text-gray-400">
                                      {new Date(log.created_at).toLocaleString('zh-CN')}
                                    </span>
                                  </div>
                                </div>
                                <Show when={decision?.reason}>
                                  <p class="text-xs text-gray-600 mb-1">{decision.reason}</p>
                                </Show>
                                <Show when={log.result?.analysis}>
                                  <p class="text-[11px] text-gray-500 leading-relaxed line-clamp-3">{log.result.analysis}</p>
                                </Show>
                                <Show when={log.error}>
                                  <p class="text-[11px] text-red-500 mt-1">{log.error}</p>
                                </Show>
                                <Show when={log.result?.risk_warning && log.result.risk_warning !== 'none'}>
                                  <p class="text-[11px] text-amber-600 mt-1">⚠ {log.result.risk_warning}</p>
                                </Show>
                                <Show when={expandedLogId() === log.id}>
                                  <div class="mt-3 space-y-3 border-t border-gray-100 pt-3">
                                    <Show when={log.system_prompt}>
                                      <div>
                                        <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">System Prompt</div>
                                        <pre class="text-[11px] text-gray-600 bg-gray-50 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-48 overflow-y-auto font-mono leading-relaxed">{log.system_prompt}</pre>
                                      </div>
                                    </Show>
                                    <Show when={log.user_prompt}>
                                      <div>
                                        <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">User Prompt</div>
                                        <pre class="text-[11px] text-gray-600 bg-gray-50 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-48 overflow-y-auto font-mono leading-relaxed">{log.user_prompt}</pre>
                                      </div>
                                    </Show>
                                    <Show when={rawResponse}>
                                      <div>
                                        <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">原始 LLM 返回</div>
                                        <pre class="text-[11px] text-gray-600 bg-gray-50 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed">{typeof rawResponse === 'string' ? rawResponse : JSON.stringify(rawResponse, null, 2)}</pre>
                                      </div>
                                    </Show>
                                    <Show when={!rawResponse && log.result}>
                                      <div>
                                        <div class="text-[10px] text-gray-400 uppercase tracking-wider mb-1">解析后结果</div>
                                        <pre class="text-[11px] text-gray-600 bg-gray-50 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-48 overflow-y-auto font-mono leading-relaxed">{JSON.stringify(log.result, null, 2)}</pre>
                                      </div>
                                    </Show>
                                  </div>
                                </Show>
                              </div>
                            );
                          }}
                        </For>
                      </div>
                    </Show>
                  </Show>
                </div>
              </>
            );
          })()}
        </Show>
      </div>
    </div>
  );
}
