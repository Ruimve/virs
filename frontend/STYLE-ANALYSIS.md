# VIRS 前端样式分析报告

> 全量扫描范围: `src/` 下全部 93 个文件，涵盖 components、pages、service、context、layout

---

## 一、重复样式模式 — 应抽取为公共组件

### 1. Stat 字段组件（label + value）— 出现 6 处

**完全相同的本地 `Stat` 组件定义了 2 次：**

| 文件                                  | 行号     | 说明                                                                                                                                                         |
| ------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `AutoBot/Bot/PositionStats/index.tsx` | L169-177 | `<div className="min-w-0"><div text-[11px] uppercase tracking-wider text-on-surface-tertiary mb-0.5">...</div><div text-sm font-mono tabular-nums truncate>` |
| `GridBot/Bot/PositionStats/index.tsx` | L164-172 | **完全相同**                                                                                                                                                 |

**相同的 label + value 行内模式出现 4 处（未封装）：**

| 文件                               | 场景                                                            |
| ---------------------------------- | --------------------------------------------------------------- |
| `AutoBot/Bot/TradeStats/index.tsx` | 13 个统计指标（L114-123）                                       |
| `GridBot/Bot/TradeStats/index.tsx` | 13 个统计指标（L109-118）                                       |
| `Trade/AutoBot/System/index.tsx`   | Card 内 label-value 对（如 L155-156 主机名、L159-160 操作系统） |
| `Setup/ReviewLaunch/index.tsx`     | Summary 表格行（L183-248 共 9 行）                              |

**建议：** 抽取 `src/components/Stat/index.tsx`

```
Props: { label: string; value: ReactNode; sub?: string; color?: string; highlight?: boolean }
```

---

### 2. 侧边栏 Section 容器（title + count + scrollable list + empty）— 出现 4 处

**完全相同的结构模式：**

```
<div className="flex flex-col min-h-0">
  <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
    <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">标题</span>
    <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">数量</span>
  </div>
  <div className="flex-1 overflow-y-auto">
    {empty ? (
      <div className="text-center py-6 text-sm text-on-surface-tertiary">暂无数据</div>
    ) : (
      <div className="divide-y divide-line-subtle">...</div>
    )}
  </div>
</div>
```

| 文件                                    | 标题       |
| --------------------------------------- | ---------- |
| `AutoBot/Bot/RecentDecisions/index.tsx` | "最近决策" |
| `AutoBot/Bot/RecentTrades/index.tsx`    | "最近交易" |
| `GridBot/Bot/RecentTrades/index.tsx`    | "最近成交" |
| `GridBot/Bot/LevelsOverview/index.tsx`  | "网格层级" |

**建议：** 抽取 `src/components/Panel/index.tsx`

```
Props: { title: string; count?: number; children: ReactNode; empty?: boolean; emptyText?: string }
```

---

### 3. pnlColor 函数 — 出现 4 处

```typescript
const pnlColor = (v: number) =>
  v > 0 ? 'text-success-text' : v < 0 ? 'text-danger-text' : 'text-on-surface';
```

| 文件                                          | 行号                                 |
| --------------------------------------------- | ------------------------------------ |
| `AutoBot/Bot/PositionStats/index.tsx`         | L17-18                               |
| `GridBot/Bot/PositionStats/index.tsx`         | L17-18                               |
| `AutoBot/Bot/RecentTrades/index.tsx`          | L9-10                                |
| `utils/utils.tsx` (formatPnl, formatPnlShort) | L3-16 — 已有类似逻辑但返回 ReactNode |

**建议：** 在 `utils/utils.tsx` 中统一导出 `pnlColor` 纯函数，4 处统一引用

---

### 4. Card 卡片容器 — 出现 10+ 处，样式不统一

**三种不同的 Card 样式变体：**

| 变体 | className                                                      | 使用位置                                          |
| ---- | -------------------------------------------------------------- | ------------------------------------------------- |
| A    | `bg-surface-1 rounded-xl border border-line-default shadow-sm` | LogList、LogDetail、GridLevelsTab、GridBot/Trades |
| B    | `bg-surface-1 border border-line-subtle rounded-xl p-4`        | System page（本地 Card 组件）                     |
| C    | `bg-surface-1 rounded-xl border border-line-default p-4`       | LogDetail 内各 section                            |

**差异：** border 用 `line-default` 还是 `line-subtle`，有无 `shadow-sm`，有无 `p-4`

**建议：** 抽取 `src/components/Card/index.tsx`

```
Props: { children: ReactNode; title?: string; padding?: boolean; border?: 'subtle' | 'default' }
默认: bg-surface-1 rounded-xl border border-line-default shadow-sm + p-4
```

---

### 5. 方向/动作 Badge（小标签）— 出现 10+ 处

**三种 Badge 类型，样式高度重复：**

**A) 买卖方向 Badge：**

```tsx
className={`text-[11px] font-medium px-1.5 py-0.5 rounded ${buy ? 'bg-success-bg text-success-text' : 'bg-danger-bg text-danger-text'}`}
```

出现: RecentTrades(AutoBot)、GridLevelsTab、GridBot/Trades、LevelsOverview — 共 6 处

**B) 决策动作 Badge（使用 actionColor）：**

```tsx
className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${actionColor(action)}`}
```

出现: DecisionCard、LogList、LogDetail — 共 5 处

**C) 状态 Badge：**

```tsx
className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${statusColor}`}
```

出现: GridBot/Trades(已平/持仓)、HealthCheck(CheckDetail Paper/Live)、DecisionCard(失败) — 共 5 处

**建议：** 抽取 `src/components/Badge/index.tsx`

```
Props: { variant: 'success' | 'danger' | 'warning' | 'info' | 'neutral'; size?: 'xs' | 'sm'; children: ReactNode }
```

---

### 6. Loading / Empty 状态 — 出现 7 处

**Loading 状态（图标 + 文字）：**

```tsx
<div className="flex flex-col items-center justify-center py-16 gap-4 text-on-surface-tertiary text-xs">
  <Icon size={40} />
  <span className="tracking-wider">加载文字</span>
</div>
```

出现: AutoBot/Trades、GridBot/Trades、LogList、LogDetail — 4 处

**Empty 状态：**

```tsx
<div className="text-center py-6 text-sm text-on-surface-tertiary">暂无数据</div>
```

出现: RecentDecisions、RecentTrades(AutoBot)、GridBot/RecentTrades、LevelsOverview、GridLevelsTab、ConfigureLlm — 6 处

**Center Spinner：**

```tsx
<div className="h-full flex items-center justify-center">
  <Spinner className="h-6 w-6 text-on-surface-tertiary" />
</div>
```

出现: System page、LogDetail — 2 处

**建议：** 抽取 `src/components/StateFeedback/index.tsx`

```
Props: { type: 'loading' | 'empty' | 'error'; text?: string; icon?: ReactNode }
```

---

### 7. Progress Bar — 出现 3 处，逻辑重复

| 文件                                      | 组件名                                          | barColor 函数                |
| ----------------------------------------- | ----------------------------------------------- | ---------------------------- |
| `AutoBot/System/index.tsx`                | `Progress` (L47-54)                             | `barColor(pct)` — 阈值 90/70 |
| `HealthCheck/CheckDetail/MiniBar.tsx`     | `MiniBar` (L9-16)                               | `pctBar(pct)` — 阈值 90/75   |
| `HealthCheck/CheckDetail/ResourceRow.tsx` | 使用 MiniBar，另有 `pctColor(pct)` — 阈值 90/75 |

**差异：** 阈值不一致（70 vs 75），尺寸不同（`h-1.5` vs `h-1`）

**建议：** 抽取 `src/components/Progress/index.tsx`

```
Props: { pct: number; size?: 'sm' | 'md'; thresholds?: { warning: number; danger: number } }
```

---

### 8. Key-Value Row（水平键值对）— 出现 12+ 处

```tsx
<div className="flex items-center justify-between px-3 py-2 bg-surface-1 border border-line-default rounded-lg">
  <span className="text-[12px] text-on-surface-tertiary">Label</span>
  <span className="text-[12px] text-on-surface-secondary font-mono">Value</span>
</div>
```

出现: ReviewLaunch summary (9 行)、ConfigureLlm balance、ConfigureExchange permissions — 共 12+ 处

**建议：** 抽取 `src/components/KeyValueRow/index.tsx`

```
Props: { label: string; value: ReactNode; valueColor?: string }
```

---

### 9. Table 公共结构 — 出现 2 处

几乎相同的 `<table>` 结构：

| 文件                               | 表头列数 |
| ---------------------------------- | -------- |
| `GridBot/Levels/GridLevelsTab.tsx` | 7 列     |
| `GridBot/Trades/index.tsx`         | 9 列     |

**相同的表头样式：**

```tsx
<thead>
  <tr className="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
    <th className="text-left/right/center px-3 py-2.5 font-medium">...</th>
  </tr>
</thead>
```

**相同的行样式：**

```tsx
<tr className="border-b border-line-subtle">
  <td className="px-3 py-2 ...">...</td>
</tr>
```

**建议：** 抽取 `src/components/Table/index.tsx`（含 thead/td 样式封装）

---

### 10. Pagination 分页 — 出现 2 处，完全相同

```tsx
<div className="flex items-center justify-between px-5 py-3 border-t border-line-subtle text-xs">
  <span>
    共 {total} 条 · 第 {page}/{totalPages} 页
  </span>
  <div className="flex items-center gap-2">
    <button className="px-2 py-1 rounded border border-line-default ...">上一页</button>
    <button className="px-2 py-1 rounded border border-line-default ...">下一页</button>
  </div>
</div>
```

出现: AutoBot/Trades、GridBot/Trades — 结构完全一致

**建议：** 抽取 `src/components/Pagination/index.tsx`

---

### 11. formatVolume 函数 — 出现 2 处，实现略有差异

| 文件                                       | 支持范围 |
| ------------------------------------------ | -------- |
| `AutoBot/Bot/TradeStats/index.tsx` (L8-12) | K, M     |
| `StickyMarket/index.tsx` (L24-29)          | K, M, B  |

**差异：** StickyMarket 版本多了 B（十亿级）

**建议：** 统一到 `utils/utils.tsx`，取 StickyMarket 的更完整版本

---

### 12. Section Title 样式 — 出现 15+ 处，有 2 种变体

| 变体 | className                                                                   | 使用位置                                                                    |
| ---- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| A    | `text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium` | TradeStats、DecisionCard、RecentDecisions/Trades、LogDetail、HealthCheck 等 |
| B    | `text-[10px] text-on-surface-tertiary uppercase tracking-wider`             | LogDetail 内各 section 标题、ConfigureExchange step 4                       |

**差异：** 字号 11px vs 10px，有无 `font-medium`，有无 `tracking-wider`

**建议：** 统一为一种，抽取为 Tailwind `@layer components` 的 `.section-title` 工具类，或封装 `Title` 组件

---

## 二、样式不一致问题

### 1. Badge 字号不统一

- 决策动作 Badge: `text-[10px]` (LogList L55)
- 买卖方向 Badge: `text-[11px]` (RecentTrades L35, GridLevelsTab L55)
- 状态 Badge: `text-[10px]` (GridBot/Trades L97)
- 失败 Badge: `text-[11px]` (DecisionCard L79) vs `text-[10px]` (LogList L60)

### 2. Card border 语义不统一

- 同一页面内 LogDetail: 部分用 `border-line-default` (L85, L122, L136)，部分用 `border-line-subtle` (L184, L234)
- System Card: 用 `border-line-subtle`，但 LogDetail 用 `border-line-default`

### 3. rounded 角度不统一

- Card: `rounded-xl`
- Badge: `rounded` (部分) vs `rounded-lg` (部分) vs `rounded-md` (部分)
- ConfigureLlm model chip: `rounded-md`
- SelectBotType feature tag: `rounded-md`
- ReviewLaunch badge: `rounded`

### 4. 间距系统不统一

padding:

- `px-3 py-2` — 侧边栏面板头
- `px-4 py-3` — 交易统计、决策卡片
- `px-4 py-5` — LogDetail section header
- `px-5 py-3` — 分页栏

gap:

- `gap-3` — TradeStats 网格
- `gap-4` — System cards
- `gap-2` — HealthCheck badges
- `gap-1.5` — Badge 内部

---

## 三、建议抽取的公共组件清单

| #   | 组件名          | 路径                                     | 替代文件数 | 优先级 |
| --- | --------------- | ---------------------------------------- | ---------- | ------ |
| 1   | `Stat`          | `src/components/Stat/index.tsx`          | 6          | **高** |
| 2   | `Panel`         | `src/components/Panel/index.tsx`         | 4          | **高** |
| 3   | `Badge`         | `src/components/Badge/index.tsx`         | 10+        | **高** |
| 4   | `Card`          | `src/components/Card/index.tsx`          | 10+        | **高** |
| 5   | `StateFeedback` | `src/components/StateFeedback/index.tsx` | 7          | **高** |
| 6   | `KeyValueRow`   | `src/components/KeyValueRow/index.tsx`   | 12+        | 中     |
| 7   | `Progress`      | `src/components/Progress/index.tsx`      | 3          | 中     |
| 8   | `Pagination`    | `src/components/Pagination/index.tsx`    | 2          | 中     |
| 9   | `Table`         | `src/components/Table/index.tsx`         | 2          | 低     |
| 10  | `Title`         | `src/components/Title/index.tsx`         | 15+        | 中     |

---

## 四、工具函数统一

| 函数                | 统一到                                       | 当前分布                   |
| ------------------- | -------------------------------------------- | -------------------------- |
| `pnlColor(v)`       | `src/pages/Trade/components/utils/utils.tsx` | 4 处重复定义               |
| `formatVolume(v)`   | 同上                                         | 2 处重复定义（取更完整版） |
| `barColor/pctColor` | 同上（或放入 Progress 组件内部）             | 3 处不同阈值               |

---

## 五、全局样式建议

在 `src/index.css` 中添加 `@layer components` 工具类，减少 Tailwind class 重复：

```css
@layer components {
  /* 统一 Section 标题 */
  .section-title {
    @apply text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium;
  }

  /* 统一数据文字 */
  .data-value {
    @apply text-sm font-mono tabular-nums;
  }

  /* 统一 Hero 数字 */
  .hero-value {
    @apply text-xl font-mono font-semibold tabular-nums text-on-surface;
  }

  /* 统一 mono 标签 */
  .mono-label {
    @apply text-[11px] font-mono tabular-nums;
  }
}
```

---

## 六、优先级实施建议

**Phase 1（立即执行 — 影响 50+ 处）：**

1. `Stat` 组件 — 消除 PositionStats 中 2 处完全相同的本地定义 + TradeStats 4 处内联
2. `Badge` 组件 — 统一 10+ 处不同尺寸/样式的标签
3. `Card` 组件 — 统一 3 种 Card 样式变体
4. `pnlColor` / `formatVolume` 工具函数统一

**Phase 2（短期 — 影响舒适度）：** 5. `Panel` 组件 — 4 处侧边栏面板统一 6. `StateFeedback` 组件 — 7 处 loading/empty 状态统一 7. `KeyValueRow` 组件 — ReviewLaunch 等 12+ 处键值对统一 8. 全局 CSS layer 工具类 — `section-title`、`data-value`

**Phase 3（中期 — 锦上添花）：** 9. `Progress` 组件 — 统一阈值和样式 10. `Pagination` 组件 — 2 处分页统一 11. `Table` 组件 — 2 处表格结构统一
