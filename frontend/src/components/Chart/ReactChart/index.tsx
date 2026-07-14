import { useRef, useEffect, memo } from 'react';
import { createChart, type IChartApi, ColorType } from 'lightweight-charts';

export interface ReactChartProps {
  onLoad: (chart: IChartApi | undefined) => void;
  height?: number;
  timeVisible?: boolean;
  secondsVisible?: boolean;
}


function ReactChart({ onLoad, height, timeVisible, secondsVisible }: ReactChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | undefined>(undefined);


  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;


    const cs = getComputedStyle(el);
    const bgBase = cs.getPropertyValue('--bg-base').trim() || '#ffffff';
    const textOnSurfaceTertiary =
      cs.getPropertyValue('--text-on-surface-tertiary').trim() || '#94a3b8';
    const borderDefault = cs.getPropertyValue('--border-default').trim() || '#e2e8f0';
    const borderSubtle = cs.getPropertyValue('--border-subtle').trim() || '#f1f5f9';

    const chart = createChart(el, {
      layout: {
        background: { type: ColorType.Solid, color: bgBase },
        textColor: textOnSurfaceTertiary,
        fontSize: 12,
      },
      grid: {
        vertLines: { color: borderSubtle },
        horzLines: { color: borderSubtle },
      },
      crosshair: {
        mode: 0,
      },
      rightPriceScale: {
        borderColor: borderDefault,
      },
      timeScale: {
        borderColor: borderDefault,
        timeVisible: timeVisible ?? true,
        secondsVisible: secondsVisible ?? false,
      },
      handleScroll: { vertTouchDrag: false },
    });

    chartRef.current = chart;
    onLoad(chart);

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        if (chart && width > 0 && height > 0) {
          chart.applyOptions({ width, height });
        }
      }
    });
    resizeObserver.observe(el);

    return () => {
      resizeObserver.disconnect();
      chart.remove();
      chartRef.current = undefined;
      onLoad(undefined);
    };

  }, [onLoad]);


  useEffect(() => {
    const chart = chartRef.current;
    if (!chart) return;
    chart.applyOptions({
      timeScale: {
        timeVisible: timeVisible ?? true,
        secondsVisible: secondsVisible ?? false,
      },
    });
  }, [timeVisible, secondsVisible]);

  return (
    <div
      ref={containerRef}
      className={`w-full rounded-lg border border-line-default overflow-hidden ${!height ? 'h-full' : ''}`}
      style={height ? { height: `${height}px` } : undefined}
    />
  );
}

export default memo(ReactChart);
