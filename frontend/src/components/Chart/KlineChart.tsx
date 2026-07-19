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

export interface KlineChartHandle {
  update: (candle: {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume?: number;
  }) => void;
}

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

const MOBILE_BREAKPOINT = 768;

const getColors = () => {
  const cs = getComputedStyle(document.documentElement);
  return {
    up: cs.getPropertyValue('--chart-up').trim() || '#10b981',
    upVolume: cs.getPropertyValue('--chart-up-volume').trim() || 'rgba(16, 185, 129, 0.3)',
    down: cs.getPropertyValue('--chart-down').trim() || '#ef4444',
    downVolume: cs.getPropertyValue('--chart-down-volume').trim() || 'rgba(239, 68, 68, 0.3)',
  };
};

const getVisibleRangeWidth = () => {
  if (typeof window !== 'undefined' && window.innerWidth < MOBILE_BREAKPOINT) {
    return 50;
  }
  return 100;
};

const applyVisibleRange = (chart: IChartApi, dataLength: number) => {
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
};

const KlineChart = forwardRef<KlineChartHandle, KlineChartProps>(function KlineChart(
  { data, height, markers, overlays },
  ref,
) {
  const chartRef = useRef<IChartApi>(null);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'>>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'>>(null);

  const markersPluginRef = useRef<{ detach: () => void } | null>(null);
  const overlaySeriesRef = useRef<ISeriesApi<'Line'>[]>([]);
  const initializedRef = useRef(false);

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

  useImperativeHandle(
    ref,
    () => ({
      update(candle) {
        const candleSeries = candleSeriesRef.current;
        const volumeSeries = volumeSeriesRef.current;
        const colors = colorsRef.current;
        if (!candleSeries || !volumeSeries || !colors) return;

        candleSeries.update({
          time: toLocaleTime(candle.time),
          open: candle.open,
          high: candle.high,
          low: candle.low,
          close: candle.close,
        });

        if (candle.volume !== undefined) {
          const { upVolume, downVolume } = colors;
          volumeSeries.update({
            time: toLocaleTime(candle.time),
            value: candle.volume,
            color: candle.close >= candle.open ? upVolume : downVolume,
          });
        }
      },
    }),
    [],
  );

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || data.length === 0) return;

    const isFirstInit = !initializedRef.current;

    if (isFirstInit) {
      initializedRef.current = true;

      const colors = getColors();
      colorsRef.current = colors;

      const candleSeries = chart.addSeries(CandlestickSeries, {
        upColor: colors.up,
        downColor: colors.down,
        borderDownColor: colors.down,
        borderUpColor: colors.up,
        wickDownColor: colors.down,
        wickUpColor: colors.up,
      });
      candleSeriesRef.current = candleSeries;

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

    const colors = colorsRef.current;
    const candleSeries = candleSeriesRef.current;
    if (!candleSeries) return;

    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }));
    candleSeries.setData(chartData);

    const volumeSeries = volumeSeriesRef.current;
    if (volumeSeries) {
      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? colors.upVolume : colors.downVolume,
        })),
      );
    }

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

    applyVisibleRange(chart, data.length);
  }, [data, markers, overlays, readChartColors]);

  const onLoad = useCallback((c: IChartApi) => {
    chartRef.current = c;
  }, []);

  return <ReactChart onLoad={onLoad} height={height} />;
});

export default memo(KlineChart);
