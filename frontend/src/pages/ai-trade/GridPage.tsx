import { createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import { api } from '../../lib/api';

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
  created_at: string;
}

export default function GridPage() {
  const navigate = useNavigate();

  const [bots, setBots] = createSignal<GridBot[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');

  const [showCreate, setShowCreate] = createSignal(false);
  const [name, setName] = createSignal('');
  const [symbol, setSymbol] = createSignal('');
  const [exchange, setExchange] = createSignal('binance');
  const [leverage, setLeverage] = createSignal(1);
  const [dynamicAdjust, setDynamicAdjust] = createSignal(true);
  const [creating, setCreating] = createSignal(false);

  const [paperEnabled, setPaperEnabled] = createSignal(false);
  const [paperLoading, setPaperLoading] = createSignal(false);

  const loadBots = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const res = await api.get<{ items: GridBot[]; total: number }>('/grid/list');
      if (res.data) setBots(res.data.items);
    } catch (e) {
      console.error('Failed to load bots:', e);
    } finally {
      if (showLoading) setLoading(false);
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

  const handleCreateAndStart = async () => {
    if (!name().trim() || !symbol().trim()) {
      setError('请填写机器人名称和交易对');
      return;
    }
    setCreating(true);
    setError('');
    try {
      const createRes = await api.post<{ bot: { id: string } }>('/grid/create', {
        name: name().trim(),
        symbol: symbol().trim().toUpperCase(),
        exchange: exchange(),
        leverage: leverage(),
        dynamic_adjust: dynamicAdjust(),
      });
      const botId = createRes.data?.bot?.id;
      if (!botId) throw new Error('创建机器人失败');
      await api.post(`/grid/${botId}/start`);
      setName('');
      setSymbol('');
      setLeverage(1);
      setDynamicAdjust(true);
      setShowCreate(false);
      await loadBots();
    } catch (e: any) {
      setError(e.response?.data?.error || e.message || '操作失败');
    } finally {
      setCreating(false);
    }
  };

  const handleQuickAction = async (bot: GridBot, action: 'start' | 'stop' | 'delete') => {
    try {
      if (action === 'stop') await api.post(`/grid/${bot.id}/stop`);
      else if (action === 'start') await api.post(`/grid/${bot.id}/start`);
      else if (action === 'delete') {
        if (!confirm(`确定删除机器人 "${bot.name}" 吗？`)) return;
        if (bot.status === 'running') await api.post(`/grid/${bot.id}/stop`);
        await api.del(`/grid/${bot.id}/delete`);
      }
      await loadBots();
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
    };
    return map[status] || { text: status, dot: 'bg-gray-400', bg: 'bg-gray-100 text-gray-500' };
  };

  const formatPnl = (pnl: number) => {
    if (pnl > 0) return <span class="text-emerald-600 font-semibold">+{pnl.toFixed(2)}</span>;
    if (pnl < 0) return <span class="text-red-600 font-semibold">{pnl.toFixed(2)}</span>;
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
        <div class="flex items-center justify-between mb-8">
          <div>
            <h1 class="text-xl font-semibold tracking-tight text-gray-900">网格机器人</h1>
            <p class="text-xs text-gray-500 mt-0.5">AI 驱动的网格交易策略</p>
          </div>
          <div class="flex items-center gap-3">
            <button
              onClick={togglePaper}
              disabled={paperLoading()}
              class={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium border transition-all ${
                paperEnabled()
                  ? 'bg-amber-50 border-amber-200 text-amber-700'
                  : 'bg-white border-gray-200 text-gray-500 hover:text-gray-700'
              }`}
            >
              <span class={`w-1.5 h-1.5 rounded-full ${paperEnabled() ? 'bg-amber-500' : 'bg-gray-300'}`} />
              {paperEnabled() ? 'Paper 交易中' : 'Paper 交易'}
            </button>
            <button
              onClick={() => setShowCreate(true)}
              class="flex items-center gap-1.5 px-4 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-medium transition-colors"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" />
              </svg>
              创建机器人
            </button>
          </div>
        </div>

        <Show when={error()}>
          <div class="mb-4 text-xs text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
            {error()}
          </div>
        </Show>

        <Show
          when={!loading() && bots().length > 0}
          fallback={
            <div class="rounded-xl border border-gray-200 bg-white p-16 text-center shadow-sm">
              <Show when={!loading()} fallback={
                <div class="flex justify-center">
                  <svg class="animate-spin h-5 w-5 text-gray-400" viewBox="0 0 24 24" fill="none">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                  </svg>
                </div>
              }>
                <div class="text-gray-400 text-sm">暂无机器人</div>
                <div class="text-gray-300 text-xs mt-1">点击右上角创建第一个机器人</div>
              </Show>
            </div>
          }
        >
          <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
            <For each={bots()}>
              {(bot) => {
                const sc = () => statusConfig(bot.status);
                return (
                  <div
                    onClick={() => navigate(`/ai-trade/grid/${bot.id}`)}
                    class="group rounded-xl border border-gray-200 bg-white hover:border-gray-300 p-5 cursor-pointer transition-all shadow-sm"
                  >
                    <div class="flex items-start justify-between mb-4">
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                          <span class="text-sm font-medium text-gray-800 truncate">{bot.name}</span>
                          <span class={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium ${sc().bg}`}>
                            <span class={`w-1 h-1 rounded-full ${sc().dot}`} />
                            {sc().text}
                          </span>
                        </div>
                        <div class="flex items-center gap-1.5 mt-1 text-xs text-gray-500">
                          <span class="text-gray-700 font-medium">{bot.symbol}</span>
                          <span class="text-gray-300">·</span>
                          <span>{bot.exchange.toUpperCase()}</span>
                          <span class="text-gray-300">·</span>
                          <span>{bot.leverage}x</span>
                        </div>
                      </div>
                      <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity" onClick={(e) => e.stopPropagation()}>
                        <Show when={bot.status === 'running'}>
                          <button
                            onClick={() => handleQuickAction(bot, 'stop')}
                            class="p-1.5 rounded-md hover:bg-red-50 text-gray-400 hover:text-red-500 transition-colors"
                            title="停止"
                          >
                            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" rx="1" /></svg>
                          </button>
                        </Show>
                        <Show when={bot.status !== 'running' && bot.status !== 'stopped'}>
                          <button
                            onClick={() => handleQuickAction(bot, 'start')}
                            class="p-1.5 rounded-md hover:bg-emerald-50 text-gray-400 hover:text-emerald-500 transition-colors"
                            title="启动"
                          >
                            <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
                          </button>
                        </Show>
                        <button
                          onClick={() => handleQuickAction(bot, 'delete')}
                          class="p-1.5 rounded-md hover:bg-red-50 text-gray-400 hover:text-red-500 transition-colors"
                          title="删除"
                        >
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                        </button>
                      </div>
                    </div>

                    <div class="grid grid-cols-3 gap-3">
                      <div>
                        <div class="text-[10px] text-gray-400 uppercase tracking-wider">已实现盈亏</div>
                        <div class="text-sm mt-0.5 font-mono">{formatPnl(bot.total_pnl)}</div>
                      </div>
                      <div>
                        <div class="text-[10px] text-gray-400 uppercase tracking-wider">未实现盈亏</div>
                        <div class="text-sm mt-0.5 font-mono">{formatPnl(bot.unrealized_pnl)}</div>
                      </div>
                      <div>
                        <div class="text-[10px] text-gray-400 uppercase tracking-wider">成交次数</div>
                        <div class="text-sm mt-0.5 font-mono text-gray-700">{bot.total_trades}</div>
                      </div>
                    </div>

                    <div class="flex items-center justify-between mt-4 pt-3 border-t border-gray-100">
                      <div class="flex items-center gap-3 text-[10px] text-gray-400">
                        <span>网格 {bot.grid_count || '-'}</span>
                        <span>利润率 {bot.grid_profit_pct}%</span>
                        <Show when={bot.upper_price > 0}>
                          <span>{bot.lower_price.toFixed(0)}-{bot.upper_price.toFixed(0)}</span>
                        </Show>
                      </div>
                      <Show when={bot.market_regime}>
                        <span class="text-[10px] text-indigo-500">{bot.market_regime}</span>
                      </Show>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </div>

      <Show when={showCreate()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm" onClick={() => setShowCreate(false)}>
          <div class="bg-white border border-gray-200 rounded-2xl w-full max-w-md mx-4 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between px-6 py-4 border-b border-gray-100">
              <h3 class="text-sm font-medium text-gray-800">创建网格机器人</h3>
              <button onClick={() => setShowCreate(false)} class="text-gray-400 hover:text-gray-600 transition-colors">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
              </button>
            </div>

            <div class="px-6 py-5 space-y-4">
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
                  <span class="text-sm font-mono text-gray-700 w-10 text-right">{leverage()}x</span>
                </div>
              </div>

              <div class="flex items-center justify-between py-1">
                <div>
                  <div class="text-sm text-gray-700">动态调整</div>
                  <div class="text-[10px] text-gray-400">AI 定期评估并调整网格参数</div>
                </div>
                <button
                  onClick={() => setDynamicAdjust(!dynamicAdjust())}
                  class={`relative w-9 h-5 rounded-full transition-colors ${dynamicAdjust() ? 'bg-indigo-600' : 'bg-gray-300'}`}
                >
                  <div class={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${dynamicAdjust() ? 'translate-x-[18px]' : 'translate-x-0.5'}`} />
                </button>
              </div>

              <Show when={error()}>
                <div class="text-xs text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
                  {error()}
                </div>
              </Show>
            </div>

            <div class="px-6 py-4 border-t border-gray-100">
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
      </Show>
    </div>
  );
}
