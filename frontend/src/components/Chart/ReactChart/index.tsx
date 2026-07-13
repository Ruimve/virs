import { useRef, useEffect, memo } from 'react';
import { createChart, type IChartApi, ColorType } from 'lightweight-charts';

export interface ReactChartProps {
  onLoad: (chart: IChartApi | undefined) => void;
  height?: number;
  timeVisible?: boolean;
  secondsVisible?: boolean;
}

/**
 * Base chart container — handles chart creation, resize, and cleanup.
 * Passes the IChartApi instance back via `onLoad` callback so the parent
 * can create series and set data.
 *
 * Chart creation and option updates are split into separate effects so that
 * changing `timeVisible`/`secondsVisible` calls `chart.applyOptions()` instead
 * of destroying and recreating the entire chart (which would lose all series
 * data and user interaction state).
 */
function ReactChart({ onLoad, height, timeVisible, secondsVisible }: ReactChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | undefined>(undefined);

  // ── Effect 1: Create chart (runs once) ────────────────
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    // Read theme-aware CSS variables for chart colors
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onLoad]);

  // ── Effect 2: Apply time scale options (runs on mount + when options change) ──
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
