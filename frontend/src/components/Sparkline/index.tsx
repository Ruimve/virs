import { memo, useMemo } from 'react';

export interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  className?: string;
}

export const Sparkline = memo(
  ({ data, width = 48, height = 16, className = '' }: SparklineProps) => {
    const { points, lastPoint } = useMemo(() => {
      if (!data || data.length === 0) {
        return { points: '', lastPoint: null };
      }

      const pad = 2;
      const usableWidth = width - pad * 2;
      const usableHeight = height - pad * 2;
      const min = Math.min(...data);
      const max = Math.max(...data);
      const range = max - min;
      const stepX = data.length > 1 ? usableWidth / (data.length - 1) : 0;

      const coords = data.map((v, i) => {
        const x = pad + i * stepX;
        const y =
          range === 0
            ? pad + usableHeight / 2
            : pad + usableHeight - ((v - min) / range) * usableHeight;
        return { x, y };
      });

      return {
        points: coords.map((c) => `${c.x.toFixed(2)},${c.y.toFixed(2)}`).join(' '),
        lastPoint: coords[coords.length - 1],
      };
    }, [data, width, height]);

    if (!data || data.length === 0) {
      return <svg width={width} height={height} className={className} />;
    }

    return (
      <svg width={width} height={height} className={className} viewBox={`0 0 ${width} ${height}`}>
        <polyline
          points={points}
          fill="none"
          strokeWidth={1.5}
          strokeLinejoin="round"
          strokeLinecap="round"
          className="stroke-ai"
        />
        {lastPoint && <circle cx={lastPoint.x} cy={lastPoint.y} r={1.5} className="fill-ai" />}
      </svg>
    );
  },
);
