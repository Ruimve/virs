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

// ── Component ─────────────────────────────────────────────

const KlineChart = forwardRef<KlineChartHandle, KlineChartProps>(function KlineChart(
  { data, height, markers, overlays },
  ref,
) {
  const chartRef = useRef<IChartApi | undefined>(undefined);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | undefined>(undefined);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | undefined>(undefined);
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

  // ── Initial setup on mount ─────────────────────────────

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || initializedRef.current) return;
    initializedRef.current = true;

    // Determine time scale settings based on data density
    let timeVisible = true;
    const secondsVisible = false;
    if (data.length >= 2) {
      const firstTime = data[0].time;
      const lastTime = data[data.length - 1].time;
      const spanHours = (lastTime - firstTime) / 3600;
      if (spanHours > 2160) {
        timeVisible = false;
      }
    }

    chart.applyOptions({
      timeScale: { timeVisible, secondsVisible },
    });

    // 读取主题色（仅在初始化时读取一次）
    readChartColors();
    const c = colorsRef.current;

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: c.up,
      downColor: c.down,
      borderDownColor: c.down,
      borderUpColor: c.up,
      wickDownColor: c.down,
      wickUpColor: c.up,
    });
    candleSeriesRef.current = candleSeries;

    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }));

    candleSeries.setData(chartData);

    // Update time format based on data span
    if (data.length >= 2) {
      const firstTime = data[0].time;
      const lastTime = data[data.length - 1].time;
      const spanHours = (lastTime - firstTime) / 3600;
      const newTimeVisible = spanHours <= 2160;
      if (newTimeVisible !== timeVisible) {
        chart.applyOptions({
          timeScale: { timeVisible: newTimeVisible },
        });
      }
    }

    // Volume series
    if (data.length > 0 && data[0].volume !== undefined) {
      const volumeSeries = chart.addSeries(HistogramSeries, {
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      });

      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.8, bottom: 0 },
      });

      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? c.upVolume : c.downVolume,
        })),
      );

      volumeSeriesRef.current = volumeSeries;
    }

    if (markers && markers.length > 0) {
      createSeriesMarkers(
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

    // Render overlay lines
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
      }
    }

    // Show the last N candles with 1/8 right padding for future candles
    // 电脑端 100 根，手机端 50 根
    const rangeWidth = getVisibleRangeWidth();
    if (data.length > rangeWidth) {
      chart.timeScale().setVisibleLogicalRange({
        from: data.length - rangeWidth,
        to: data.length - 1 + rangeWidth / 8,
      });
    } else if (data.length > 1) {
      const rangeWidth = data.length - 1;
      chart.timeScale().setVisibleLogicalRange({
        from: 0,
        to: rangeWidth + rangeWidth / 8,
      });
    } else {
      chart.timeScale().fitContent();
    }
  }, [data, markers, overlays, readChartColors]);

  // ── Full data replacement (timeframe change, etc.) ─────

  useEffect(() => {
    const candleSeries = candleSeriesRef.current;
    if (!candleSeries || data.length === 0) return;

    // Only call setData when the entire dataset changes (not WS updates)
    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }));

    candleSeries.setData(chartData);

    // Also update volume series
    const volumeSeries = volumeSeriesRef.current;
    if (volumeSeries) {
      const c = colorsRef.current;
      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? c.upVolume : c.downVolume,
        })),
      );
    }

    if (markers && markers.length > 0) {
      createSeriesMarkers(
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

    // Fit to last N candles with 1/8 right padding
    // 电脑端 100 根，手机端 50 根
    const rangeWidth = getVisibleRangeWidth();
    if (data.length > rangeWidth) {
      chartRef.current?.timeScale().setVisibleLogicalRange({
        from: data.length - rangeWidth,
        to: data.length - 1 + rangeWidth / 8,
      });
    } else if (data.length > 1) {
      const rangeWidth = data.length - 1;
      chartRef.current?.timeScale().setVisibleLogicalRange({
        from: 0,
        to: rangeWidth + rangeWidth / 8,
      });
    }
  }, [data, markers]);

  return <ReactChart onLoad={setChart} height={height} />;
});

export default memo(KlineChart);
