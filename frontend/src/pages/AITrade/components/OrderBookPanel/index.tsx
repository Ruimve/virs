import type { OrderBookEntry } from '../shared';
import { formatSmart } from '../shared';

interface OrderBookPanelProps {
  orderBook: { bids: OrderBookEntry[]; asks: OrderBookEntry[] };
}

export default function OrderBookPanel({ orderBook }: OrderBookPanelProps) {
  const { bids, asks } = orderBook;
  const maxBid = Math.max(...bids.map((b) => b.amount), 0.001);
  const maxAsk = Math.max(...asks.map((a) => a.amount), 0.001);

  return (
    <div className="h-full flex flex-col">
      <div className="px-4 pt-3 pb-1">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider font-medium">订单簿</div>
      </div>

      {/* Header row */}
      <div className="px-4 py-1 grid grid-cols-3 text-[10px] text-on-surface-muted uppercase tracking-wider">
        <span>价格</span>
        <span className="text-right">数量</span>
        <span className="text-right">累计</span>
      </div>

      {/* Asks (reversed) */}
      <div className="flex-1 overflow-y-auto min-h-0 px-1">
        {[...asks].reverse().map((a, i) => {
          const cumAsk = asks.slice(i).reduce((s, x) => s + x.amount, 0);
          return (
            <div key={`a-${i}`} className="relative grid grid-cols-3 px-3 py-0.5 text-[11px] font-mono">
              <div className="absolute inset-y-0 right-0 bg-red-500/[0.06]" style={{ width: `${(a.amount / maxAsk) * 100}%` }} />
              <span className="relative text-red-400">{formatSmart(a.price)}</span>
              <span className="relative text-right text-on-surface-tertiary">{formatSmart(a.amount)}</span>
              <span className="relative text-right text-on-surface-muted">{formatSmart(cumAsk)}</span>
            </div>
          );
        })}
      </div>

      {/* Spread */}
      {asks.length > 0 && bids.length > 0 && (
        <div className="px-4 py-1 text-center text-[10px] text-on-surface-muted border-y border-line-subtle">
          价差 {(asks[0].price - bids[0].price).toFixed(2)}
        </div>
      )}

      {/* Bids */}
      <div className="flex-1 overflow-y-auto min-h-0 px-1">
        {bids.map((b, i) => {
          const cumBid = bids.slice(0, i + 1).reduce((s, x) => s + x.amount, 0);
          return (
            <div key={`b-${i}`} className="relative grid grid-cols-3 px-3 py-0.5 text-[11px] font-mono">
              <div className="absolute inset-y-0 right-0 bg-emerald-500/[0.06]" style={{ width: `${(b.amount / maxBid) * 100}%` }} />
              <span className="relative text-emerald-400">{formatSmart(b.price)}</span>
              <span className="relative text-right text-on-surface-tertiary">{formatSmart(b.amount)}</span>
              <span className="relative text-right text-on-surface-muted">{formatSmart(cumBid)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
