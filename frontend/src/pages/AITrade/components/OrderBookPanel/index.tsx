import type { OrderBookEntry } from '../shared';
import { formatSmart } from '../shared';

const MAX_ROWS = 8;

interface OrderBookPanelProps {
  orderBook: { bids: OrderBookEntry[]; asks: OrderBookEntry[] };
}

export default function OrderBookPanel({ orderBook }: OrderBookPanelProps) {
  const { bids, asks } = orderBook;
  const maxBid = Math.max(...bids.map((b) => b.amount), 0.001);
  const maxAsk = Math.max(...asks.map((a) => a.amount), 0.001);

  const displayAsks = [...asks].reverse().slice(0, MAX_ROWS);
  const displayBids = bids.slice(0, MAX_ROWS);

  return (
    <div className="h-full flex flex-col">
      {/* Header row */}
      <div className="px-4 py-1 grid grid-cols-3 text-[10px] text-on-surface-muted uppercase tracking-wider shrink-0">
        <span>价格</span>
        <span className="text-right">数量</span>
        <span className="text-right">累计</span>
      </div>

      {/* Asks (reversed, fixed rows) */}
      <div className="px-1 shrink-0">
        {displayAsks.map((a, i) => {
          const cumAsk = asks.slice(asks.length - 1 - i).reduce((s, x) => s + x.amount, 0);
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
        <div className="px-4 py-1 text-center text-[10px] text-on-surface-muted border-y border-line-subtle shrink-0">
          价差 {(asks[0].price - bids[0].price).toFixed(2)}
        </div>
      )}

      {/* Bids (fixed rows) */}
      <div className="px-1 shrink-0">
        {displayBids.map((b, i) => {
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
