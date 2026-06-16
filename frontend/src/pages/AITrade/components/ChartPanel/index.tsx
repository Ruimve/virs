import { useRef } from 'react'
import KlineChart, { type KlineChartHandle } from '../../../../components/Chart/KlineChart';
import type { KlineCandle } from '../shared';

interface ChartPanelProps {
  klineData: KlineCandle[];
  klineTimeframe: string;
  onTimeframeChange: (tf: string) => void;
  chartRef?: React.RefObject<KlineChartHandle | null>;
}

export default function ChartPanel({ klineData, klineTimeframe, onTimeframeChange, chartRef }: ChartPanelProps) {
  const localRef = useRef<KlineChartHandle>(null)
  const ref = chartRef ?? localRef

  return (
    <div className="flex-1 min-h-0">
      <div className="flex items-center gap-1 px-4 pt-3 pb-1">
        {['1m', '5m', '15m', '1h', '4h', '1d'].map((tf) => (
          <button
            key={tf}
            onClick={() => onTimeframeChange(tf)}
            className={`px-2 py-0.5 rounded text-[10px] font-medium transition-colors ${
              klineTimeframe === tf
                ? 'bg-accent-light text-accent'
                : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
            }`}
          >
            {tf}
          </button>
        ))}
      </div>
      <div className="px-2 pb-2">
        {klineData.length > 0 ? (
          <KlineChart ref={ref} data={klineData} height={500} />
        ) : (
          <div className="flex items-center justify-center h-[400px] text-on-surface-tertiary text-xs">
            <svg className="animate-spin h-4 w-4 mr-2" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            加载K线...
          </div>
        )}
      </div>
    </div>
  );
}
