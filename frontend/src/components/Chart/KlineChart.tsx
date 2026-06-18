import { useRef, useEffect, useImperativeHandle, forwardRef } from 'react'
import {
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  createSeriesMarkers,
} from 'lightweight-charts'
import ReactChart from './ReactChart'
import { toLocaleTime } from './ReactChart/locale/zh_CN'

// ── Public API exposed via ref ────────────────────────────

export interface KlineChartHandle {
  /** Update the last candle (or append a new one) via series.update() — no re-render */
  update: (candle: { time: number; open: number; high: number; low: number; close: number; volume?: number }) => void
}

// ── Props ─────────────────────────────────────────────────

interface OverlayLine {
  name: string
  data: Array<{ time: number; value: number }>
  color: string
  lineWidth?: number
  priceScaleId?: string
}

interface KlineChartProps {
  data: Array<{
    time: number
    open: number
    high: number
    low: number
    close: number
    volume?: number
  }>
  height?: number
  markers?: Array<{
    time: number
    position: 'aboveBar' | 'belowBar' | 'inBar'
    color: string
    shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown'
    text?: string
  }>
  overlays?: OverlayLine[]
}

// ── Component ─────────────────────────────────────────────

const KlineChart = forwardRef<KlineChartHandle, KlineChartProps>(function KlineChart({ data, height, markers, overlays }, ref) {
  const chartRef = useRef<IChartApi | undefined>(undefined)
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | undefined>(undefined)
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | undefined>(undefined)
  const initializedRef = useRef(false)

  // ── Expose imperative API ──────────────────────────────

  useImperativeHandle(ref, () => ({
    update(candle) {
      const candleSeries = candleSeriesRef.current
      if (!candleSeries) return

      const bar: CandlestickData = {
        time: toLocaleTime(candle.time),
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
      }
      candleSeries.update(bar)

      // Also update volume series if present
      const volumeSeries = volumeSeriesRef.current
      if (volumeSeries && candle.volume !== undefined) {
        volumeSeries.update({
          time: toLocaleTime(candle.time),
          value: candle.volume,
          color: candle.close >= candle.open ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)',
        })
      }
    },
  }), [])

  const setChart = (c: IChartApi | undefined) => {
    chartRef.current = c
  }

  // ── Initial setup on mount ─────────────────────────────

  useEffect(() => {
    const chart = chartRef.current
    if (!chart || initializedRef.current) return
    initializedRef.current = true

    // Determine time scale settings based on data density
    let timeVisible = true
    const secondsVisible = false
    if (data.length >= 2) {
      const firstTime = data[0].time
      const lastTime = data[data.length - 1].time
      const spanHours = (lastTime - firstTime) / 3600
      if (spanHours > 2160) {
        timeVisible = false
      }
    }

    chart.applyOptions({
      timeScale: { timeVisible, secondsVisible },
    })

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#10b981',
      downColor: '#ef4444',
      borderDownColor: '#ef4444',
      borderUpColor: '#10b981',
      wickDownColor: '#ef4444',
      wickUpColor: '#10b981',
    })
    candleSeriesRef.current = candleSeries

    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    // Update time format based on data span
    if (data.length >= 2) {
      const firstTime = data[0].time
      const lastTime = data[data.length - 1].time
      const spanHours = (lastTime - firstTime) / 3600
      const newTimeVisible = spanHours <= 2160
      if (newTimeVisible !== timeVisible) {
        chart.applyOptions({
          timeScale: { timeVisible: newTimeVisible },
        })
      }
    }

    // Volume series
    if (data.length > 0 && data[0].volume !== undefined) {
      const volumeSeries = chart.addSeries(HistogramSeries, {
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      })

      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.8, bottom: 0 },
      })

      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)',
        }))
      )

      volumeSeriesRef.current = volumeSeries
    }

    if (markers && markers.length > 0) {
      createSeriesMarkers(candleSeries,
        markers.map((m) => ({
          time: toLocaleTime(m.time),
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        }))
      )
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
        })

        lineSeries.setData(
          overlay.data.map(d => ({
            time: toLocaleTime(d.time),
            value: d.value,
          }))
        )
      }
    }

    // Show the last 100 candles with 1/8 right padding for future candles
    if (data.length > 100) {
      const rangeWidth = 100
      chart.timeScale().setVisibleLogicalRange({
        from: data.length - rangeWidth,
        to: data.length - 1 + rangeWidth / 8,
      })
    } else if (data.length > 1) {
      const rangeWidth = data.length - 1
      chart.timeScale().setVisibleLogicalRange({
        from: 0,
        to: rangeWidth + rangeWidth / 8,
      })
    } else {
      chart.timeScale().fitContent()
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // ── Full data replacement (timeframe change, etc.) ─────

  useEffect(() => {
    const candleSeries = candleSeriesRef.current
    if (!candleSeries || data.length === 0) return

    // Only call setData when the entire dataset changes (not WS updates)
    const chartData: CandlestickData[] = data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    // Also update volume series
    const volumeSeries = volumeSeriesRef.current
    if (volumeSeries) {
      volumeSeries.setData(
        data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)',
        }))
      )
    }

    if (markers && markers.length > 0) {
      createSeriesMarkers(candleSeries,
        markers.map((m) => ({
          time: toLocaleTime(m.time),
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        }))
      )
    }

    // Fit to last 100 candles with 1/8 right padding
    if (data.length > 100) {
      const rangeWidth = 100
      chartRef.current?.timeScale().setVisibleLogicalRange({
        from: data.length - rangeWidth,
        to: data.length - 1 + rangeWidth / 8,
      })
    } else if (data.length > 1) {
      const rangeWidth = data.length - 1
      chartRef.current?.timeScale().setVisibleLogicalRange({
        from: 0,
        to: rangeWidth + rangeWidth / 8,
      })
    }
  }, [data, markers])

  return <ReactChart onLoad={setChart} height={height} />
})

export default KlineChart
