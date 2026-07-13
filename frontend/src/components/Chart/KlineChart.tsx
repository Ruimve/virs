import { useRef, useEffect, useImperativeHandle, forwardRef, memo, useCallback } from 'react';
import {
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  createSeriesMarkers,
} from 'lightweight-charts';
import ReactChart from './ReactChart';
import { toLocaleTime } from './ReactChart/locale/zh_CN';

// ── Public API exposed via ref ────────────────────────────

export interface KlineChartHandle {
  /** Update the last candle (or append a new one) via series.update() — no re-render */
  update: (candle: {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume?: number;
  }) => void;
}

// ── Props ─────────────────────────────────────────────────

interface OverlayLine {
  name: string;
  data: Array<{ time: number; value: number }>;
  color: string;
  lineWidth?: number;
  priceScaleId?: string;
}

interface KlineChartProps {
  data: Array<{
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume?: number;
  }>;
  height?: number;
  markers?: Array<{
    time: number;
    position: 'aboveBar' | 'belowBar' | 'inBar';
    color: string;
    shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown';
    text?: string;
  }>;
  overlays?: OverlayLine[];
}

// 移动端断点（与 Tailwind md 一致）
const MOBILE_BREAKPOINT = 768;
// 可视区域 K 线根数：电脑端 100，手机端 50
function getVisibleRangeWidth() {
  if (typeof window !== 'undefined' && window.innerWidth < MOBILE_BREAKPOINT) {
    return 50;
  }
  return 100;
}

/** Set visible range to last N candles with 1/8 right padding */
function applyVisibleRange(chart: IChartApi | undefined, dataLength: number) {
  if (!chart) return;
  const visibleWidth = getVisibleRangeWidth();
  if (dataLength > visibleWidth) {
    chart.timeScale().setVisibleLogicalRange({
      from: dataLength - visibleWidth,
      to: dataLength - 1 + visibleWidth / 8,
    });
  } else if (dataLength > 1) {
    const lastIdx = dataLength - 1;
    chart.timeScale().setVisibleLogicalRange({
      from: 0,
      to: lastIdx + lastIdx / 8,
    });
  } else {
    chart.timeScale().fitContent();
  }
}

// ── Component ─────────────────────────────────────────────

const KlineChart = forwardRef<KlineChartHandle, KlineChartProps>(function KlineChart(
  { data, height, markers, overlays },
  ref,
) {
  const chartRef = useRef<IChartApi | undefined>(undefined);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | undefined>(undefined);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | undefined>(undefined);
  // Minimal structural type — we only need detach() for cleanup.
  // The full ISeriesMarkersPluginApi<Time> is not exported from lightweight-charts,
  // and ReturnType<typeof createSeriesMarkers> uses <unknown> which causes variance issues.
  const markersPluginRef = useRef<{ detach: () => void } | null>(null);
  const overlaySeriesRef = useRef<ISeriesApi<'Line'>[]>([]);
  const initializedRef = useRef(false);

  // 主题感知颜色（在初始化时读取一次，避免每帧调用 getComputedStyle）
  const colorsRef = useRef({
    up: '',
    upVolume: '',
    down: '',
    downVolume: '',
  });

  const readChartColors = useCallback(() => {
    const cs = getComputedStyle(document.documentElement);
    colorsRef.current = {
      up: cs.getPropertyValue('--chart-up').trim() || '#10b981',
      upVolume: cs.getPropertyValue('--chart-up-volume').trim() || 'rgba(16, 185, 129, 0.3)',
      down: cs.getPropertyValue('--chart-down').trim() || '#ef4444',
      downVolume: cs.getPropertyValue('--chart-down-volume').trim() || 'rgba(239, 68, 68, 0.3)',
    };
  }, []);

  // ── Expose imperative API ──────────────────────────────

  useImperativeHandle(
    ref,
    () => ({
      update(candle) {
        const candleSeries = candleSeriesRef.current;
        if (!candleSeries) return;

        const bar: CandlestickData = {
          time: toLocaleTime(candle.time),
          open: candle.open,
          high: candle.high,
          low: candle.low,
          close: candle.close,
        };
        candleSeries.update(bar);

        // Also update volume series if present
        const volumeSeries = volumeSeriesRef.current;
        if (volumeSeries && candle.volume !== undefined) {
          const c = colorsRef.current;
          volumeSeries.update({
            time: toLocaleTime(candle.time),
            value: candle.volume,
            color: candle.close >= candle.open ? c.upVolume : c.downVolume,
          });
        }
      },
    }),
    [],
  );

  const setChart = useCallback((c: IChartApi | undefined) => {
    chartRef.current = c;
  }, []);

  // ── Single effect: init + data + markers + overlays ────
  // Merged from two effects to prevent:
  // 1. Double setData on mount (both effects ran)
  // 2. createSeriesMarkers leak (no ref saved for detach)
  // 3. Overlays not updating (Effect 2 didn't include overlays dep)

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || data.length === 0) return;

    const isFirstInit = !initializedRef.current;

    if (isFirstInit) {
      initializedRef.current = true;
      readChartColors();

      // Determine time scale settings based on data density
      let timeVisible = true;
      const secondsVisible = false;
      if (data.length >= 2) {
        const spanHours = (data[data.length - 1].time - data[0].time) / 3600;
        if (spanHours > 2160) timeVisible = false;
      }
      chart.applyOptions({ timeScale: { timeVisible, secondsVisible } });

      const c = colorsRef.current;

      // Create candlestick series
      const candleSeries = chart.addSeries(CandlestickSeries, {
        upColor: c.up,
        downColor: c.down,
        borderDownColor: c.down,
        borderUpColor: c.up,
        wickDownColor: c.down,
        wickUpColor: c.up,
      });
      candleSeriesRef.current = candleSeries;

      // Create volume series (only if volume data exists)
      if (data[0].volume !== undefined) {
        const volumeSeries = chart.addSeries(HistogramSeries, {
          priceFormat: { type: 'volume' },
          priceScaleId: 'volume',
        });
        chart.priceScale('volume').applyOptions({
          scaleMargins: { top: 0.8, bottom: 0 },
        });
        volumeSeriesRef.current = volumeSeries;
      }
    }

    const c = colorsRef.current;
    const candleSeries = candleSeriesRef.current!;

    // Set candlestick data (both init and update)
    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }));
    candleSeries.setData(chartData);

    // Set volume data
    const volumeSeries = volumeSeriesRef.current;
    if (volumeSeries) {
      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? c.upVolume : c.downVolume,
        })),
      );
    }

    // ── Markers: detach old, create new ──────────────────
    if (markersPluginRef.current) {
      markersPluginRef.current.detach();
      markersPluginRef.current = null;
    }
    if (markers && markers.length > 0) {
      markersPluginRef.current = createSeriesMarkers(
        candleSeries,
        markers.map((m) => ({
          time: toLocaleTime(m.time),
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        })),
      );
    }

    // ── Overlays: remove old, create new ─────────────────
    for (const series of overlaySeriesRef.current) {
      chart.removeSeries(series);
    }
    overlaySeriesRef.current = [];
    if (overlays && overlays.length > 0) {
      for (const overlay of overlays) {
        const lineSeries = chart.addSeries(LineSeries, {
          color: overlay.color,
          lineWidth: Math.min(Math.max(overlay.lineWidth || 1, 1), 4) as 1 | 2 | 3 | 4,
          priceScaleId: overlay.priceScaleId || 'right',
          lastValueVisible: false,
          priceLineVisible: false,
        });
        lineSeries.setData(
          overlay.data.map((d) => ({
            time: toLocaleTime(d.time),
            value: d.value,
          })),
        );
        overlaySeriesRef.current.push(lineSeries);
      }
    }

    // Set visible range
    applyVisibleRange(chart, data.length);
  }, [data, markers, overlays, readChartColors]);

  return <ReactChart onLoad={setChart} height={height} />;
});

export default memo(KlineChart);
