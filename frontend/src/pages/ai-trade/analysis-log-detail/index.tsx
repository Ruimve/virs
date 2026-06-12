import { createSignal, Show, onMount } from 'solid-js';
import { useParams, useNavigate } from '@solidjs/router';
import { api } from '../../../lib/api';

interface AnalysisLog {
  id: string;
  bot_id: string;
  analysis_type: string;
  status: string;
  system_prompt: string;
  user_prompt: string;
  result: any;
  error: string | null;
  llm_model: string;
  created_at: string;
}

export default function AnalysisLogDetailPage() {
  const params = useParams();
  const navigate = useNavigate();

  const [log, setLog] = createSignal<AnalysisLog | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [showSystemPrompt, setShowSystemPrompt] = createSignal(false);
  const [showUserPrompt, setShowUserPrompt] = createSignal(false);

  onMount(async () => {
    try {
      const botType = params.botType;
      const endpoint = botType === 'auto'
        ? `/auto/analysis-logs?bot_id=${params.botId}`
        : `/grid/analysis-logs?bot_id=${params.botId}`;

      const res = await api.get<{ logs: AnalysisLog[] }>(endpoint);
      const found = (res.data?.logs || []).find((l: AnalysisLog) => l.id === params.logId);
      setLog(found || null);
    } catch (e) {
      console.error('Failed to load analysis log:', e);
    } finally {
      setLoading(false);
    }
  });

  const getDecision = (log: AnalysisLog) => {
    if (log.result?.decision) return log.result.decision;
    return null;
  };

  const getActionLabel = (action: string) => {
    switch (action) {
      case 'open_long': return '开多';
      case 'open_short': return '开空';
      case 'close_position': return '平仓';
      case 'hold': return '持有';
      case 'adjust_grid': return '调整网格';
      case 'pause_grid': return '暂停网格';
      case 'resume_grid': return '恢复网格';
      case 'reduce_position': return '减仓';
      case 'cancel_order': return '取消订单';
      default: return action;
    }
  };

  const getActionColor = (action: string) => {
    switch (action) {
      case 'open_long': return 'bg-emerald-500/10 text-emerald-400';
      case 'open_short': return 'bg-red-500/10 text-red-400';
      case 'close_position': return 'bg-blue-500/10 text-blue-400';
      case 'hold': return 'bg-surface-2 text-on-surface-tertiary';
      case 'adjust_grid': return 'bg-blue-500/10 text-blue-400';
      case 'pause_grid': return 'bg-red-500/10 text-red-400';
      case 'resume_grid': return 'bg-emerald-500/10 text-emerald-400';
      case 'reduce_position': return 'bg-amber-500/10 text-amber-400';
      default: return 'bg-surface-2 text-on-surface-tertiary';
    }
  };

  return (
    <div class="min-h-screen bg-base">
      {/* Top bar */}
      <div class="sticky top-0 z-20 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
        <div class="flex items-center gap-3">
          <button
            onClick={() => navigate(-1 as any)}
            class="p-1.5 rounded-lg hover:bg-surface-2 transition-colors text-on-surface-tertiary hover:text-on-surface"
          >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h1 class="text-sm font-medium text-on-surface">AI 决策详情</h1>
        </div>
      </div>

      <div class="max-w-3xl mx-auto px-4 md:px-8 py-6">
        <Show when={!loading()} fallback={
          <div class="text-center py-20 text-on-surface-tertiary text-sm">加载中...</div>
        }>
          <Show when={log()} fallback={
            <div class="text-center py-20 text-on-surface-tertiary text-sm">未找到该记录</div>
          }>
            {(l) => {
              const decision = getDecision(l());
              return (
                <div class="space-y-4">
                  {/* Header */}
                  <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                    <div class="flex items-center justify-between mb-3">
                      <div class="flex items-center gap-2">
                        <Show when={decision}>
                          {(d) => (
                            <span class={`text-xs font-medium px-2 py-0.5 rounded ${getActionColor(d().action)}`}>
                              {getActionLabel(d().action)}
                            </span>
                          )}
                        </Show>
                        <Show when={l().status === 'failed'}>
                          <span class="text-xs font-medium px-2 py-0.5 rounded bg-red-500/10 text-red-400">失败</span>
                        </Show>
                      </div>
                      <span class="text-[11px] text-on-surface-tertiary">
                        {new Date(l().created_at).toLocaleString('zh-CN')}
                      </span>
                    </div>
                    <Show when={l().llm_model}>
                      <div class="text-[11px] text-on-surface-tertiary mb-2">
                        模型: <span class="text-indigo-400 font-mono">{l().llm_model}</span>
                      </div>
                    </Show>
                    <Show when={decision?.confidence != null}>
                      <div class="text-[11px] text-on-surface-tertiary">
                        置信度: <span class="text-on-surface-secondary">{(decision.confidence * 100).toFixed(0)}%</span>
                      </div>
                    </Show>
                  </div>

                  {/* Decision */}
                  <Show when={decision}>
                    {(d) => (
                      <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                        <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">决策</div>
                        <Show when={d().reason}>
                          <p class="text-xs text-on-surface-secondary leading-relaxed">{d().reason}</p>
                        </Show>
                      </div>
                    )}
                  </Show>

                  {/* Analysis */}
                  <Show when={l().result?.analysis}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">AI 分析</div>
                      <p class="text-xs text-on-surface-secondary leading-relaxed whitespace-pre-wrap">{l().result.analysis}</p>
                    </div>
                  </Show>

                  {/* Market */}
                  <Show when={l().result?.market}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">市场状态</div>
                      <div class="space-y-1">
                        <Show when={l().result.market.market_regime}>
                          <span class={`px-2 py-0.5 text-xs rounded font-medium ${
                            l().result.market.market_regime === 'ranging' ? 'bg-blue-500/10 text-blue-400'
                            : l().result.market.market_regime === 'trending_up' ? 'bg-emerald-500/10 text-emerald-400'
                            : l().result.market.market_regime === 'trending_down' ? 'bg-red-500/10 text-red-400'
                            : l().result.market.market_regime === 'volatile' ? 'bg-amber-500/10 text-amber-400'
                            : 'bg-surface-2 text-on-surface-secondary'
                          }`}>{l().result.market.market_regime}</span>
                        </Show>
                        <Show when={l().result.market.funding_rate_warning}>
                          <p class="text-xs text-amber-400">⚠ 资金费率: {l().result.market.funding_rate_warning}</p>
                        </Show>
                        <Show when={l().result.market.event_impact}>
                          <p class="text-xs text-purple-400">事件影响: {l().result.market.event_impact}</p>
                        </Show>
                      </div>
                    </div>
                  </Show>

                  {/* Grid params (for grid bot) */}
                  <Show when={l().result?.grid && (l().result.grid.upper_price || l().result.grid.lower_price)}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">网格参数</div>
                      <div class="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                        <Show when={l().result.grid.upper_price}><span>上界 {l().result.grid.upper_price}</span></Show>
                        <Show when={l().result.grid.lower_price}><span>下界 {l().result.grid.lower_price}</span></Show>
                        <Show when={l().result.grid.grid_count}><span>层数 {l().result.grid.grid_count}</span></Show>
                        <Show when={l().result.grid.grid_profit_pct}><span>利润率 {l().result.grid.grid_profit_pct}%</span></Show>
                      </div>
                    </div>
                  </Show>

                  {/* Risk params (for grid bot) */}
                  <Show when={l().result?.risk && (l().result.risk.leverage || l().result.risk.quantity_per_grid)}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">风控参数</div>
                      <div class="flex flex-wrap gap-3 text-xs text-on-surface-secondary">
                        <Show when={l().result.risk.leverage}><span>杠杆 {l().result.risk.leverage}x</span></Show>
                        <Show when={l().result.risk.quantity_per_grid}><span>每格 {l().result.risk.quantity_per_grid} USDT</span></Show>
                      </div>
                    </div>
                  </Show>

                  {/* Risk warning */}
                  <Show when={l().result?.risk_warning && l().result.risk_warning !== 'none'}>
                    <div class="bg-amber-500/5 rounded-xl border border-amber-500/20 p-5">
                      <div class="text-[10px] text-amber-400 uppercase tracking-wider mb-2">风险提示</div>
                      <p class="text-xs text-amber-400">{l().result.risk_warning}</p>
                    </div>
                  </Show>

                  {/* Error */}
                  <Show when={l().error}>
                    <div class="bg-red-500/5 rounded-xl border border-red-500/20 p-5">
                      <div class="text-[10px] text-red-400 uppercase tracking-wider mb-2">错误</div>
                      <pre class="text-xs text-red-400 whitespace-pre-wrap">{l().error}</pre>
                    </div>
                  </Show>

                  {/* System Prompt (collapsible) */}
                  <Show when={l().system_prompt}>
                    <div class="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
                      <button
                        onClick={() => setShowSystemPrompt(!showSystemPrompt())}
                        class="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors"
                      >
                        <svg class={`w-3 h-3 transition-transform ${showSystemPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                        </svg>
                        System Prompt
                      </button>
                      <Show when={showSystemPrompt()}>
                        <div class="px-5 pb-4">
                          <pre class="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">{l().system_prompt}</pre>
                        </div>
                      </Show>
                    </div>
                  </Show>

                  {/* User Prompt (collapsible) */}
                  <Show when={l().user_prompt}>
                    <div class="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
                      <button
                        onClick={() => setShowUserPrompt(!showUserPrompt())}
                        class="w-full flex items-center gap-2 px-5 py-3 text-[10px] text-on-surface-tertiary uppercase tracking-wider hover:text-on-surface-secondary transition-colors"
                      >
                        <svg class={`w-3 h-3 transition-transform ${showUserPrompt() ? 'rotate-90' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                        </svg>
                        User Prompt
                      </button>
                      <Show when={showUserPrompt()}>
                        <div class="px-5 pb-4">
                          <pre class="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">{l().user_prompt}</pre>
                        </div>
                      </Show>
                    </div>
                  </Show>

                  {/* Raw LLM Response */}
                  <Show when={l().result?.raw_llm_response}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">LLM 原始响应</div>
                      <pre class="text-[11px] text-indigo-400 bg-indigo-500/5 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-indigo-500/10">
                        {typeof l().result.raw_llm_response === 'string'
                          ? l().result.raw_llm_response
                          : JSON.stringify(l().result.raw_llm_response, null, 2)}
                      </pre>
                    </div>
                  </Show>

                  {/* Full result JSON */}
                  <Show when={l().result && !l().result.raw_llm_response}>
                    <div class="bg-surface-1 rounded-xl border border-line-default p-5 shadow-sm">
                      <div class="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">完整结果</div>
                      <pre class="text-[11px] text-on-surface-secondary bg-surface-2 rounded-lg p-3 overflow-x-auto whitespace-pre-wrap break-all max-h-64 overflow-y-auto font-mono leading-relaxed border border-line-subtle">{JSON.stringify(l().result, null, 2)}</pre>
                    </div>
                  </Show>
                </div>
              );
            }}
          </Show>
        </Show>
      </div>
    </div>
  );
}
