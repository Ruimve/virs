import { type Component } from 'solid-js'

const AutoPage: Component = () => {
  return (
    <div class="space-y-4">
      {/* ═══ 全自动交易机器人 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-4 border-b border-slate-100">
          <div class="flex items-center gap-3">
            <div class="w-1 h-5 bg-indigo-500 rounded-full" />
            <h3 class="text-[14px] font-semibold text-slate-800">全自动交易机器人</h3>
            <span class="px-2 py-0.5 rounded-md text-[11px] font-semibold bg-amber-50 text-amber-600">开发中</span>
          </div>
        </div>
        <div class="text-center py-16">
          <p class="text-slate-400 text-[13px]">全自动交易机器人功能正在开发中，敬请期待</p>
        </div>
      </div>

      {/* ═══ 历史分析记录 ═══ */}
      <div class="bg-white rounded-2xl border border-slate-200/80 shadow-sm overflow-hidden">
        <div class="px-5 py-4 border-b border-slate-100">
          <div class="flex items-center gap-3">
            <div class="w-1 h-5 bg-indigo-500 rounded-full" />
            <h3 class="text-[14px] font-semibold text-slate-800">历史分析记录</h3>
          </div>
        </div>
        <div class="text-center py-16">
          <p class="text-slate-400 text-[13px]">暂无历史分析记录</p>
        </div>
      </div>
    </div>
  )
}

export default AutoPage
