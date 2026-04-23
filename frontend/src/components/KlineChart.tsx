import { type Component, createEffect, onCleanup, onMount } from 'solid-js'
import {
  createChart,
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  type Time,
  ColorType,
} from 'lightweight-charts'
import type { OverlayLine } from '../utils/indicators'

export type { OverlayLine }

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

const KlineChart: Component<KlineChartProps> = (props) => {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let candleSeries: ISeriesApi<'Candlestick'> | undefined

  onMount(() => {
    if (!containerRef) return

    // Determine time scale settings based on data density
    // Reference: Binance time axis formatting per timeframe
    let timeVisible = true
    let secondsVisible = false
    if (props.data.length >= 2) {
      const firstTime = props.data[0].time
      const lastTime = props.data[props.data.length - 1].time
      const spanHours = (lastTime - firstTime) / 3600
      // < 48h: show HH:mm (1m/5m/15m/1h)
      // 48h ~ 90d: show MM-DD (4h/1d short range)
      // > 90d: show YYYY-MM-DD (1d long range)
      if (spanHours > 2160) {
        timeVisible = false
      }
    }

    chart = createChart(containerRef, {
      layout: {
        background: { type: ColorType.Solid, color: '#ffffff' },
        textColor: '#6b7280',
        fontSize: 12,
      },
      grid: {
        vertLines: { color: '#f3f4f6' },
        horzLines: { color: '#f3f4f6' },
      },
      crosshair: {
        mode: 0,
      },
      rightPriceScale: {
        borderColor: '#e5e7eb',
      },
      timeScale: {
        borderColor: '#e5e7eb',
        timeVisible,
        secondsVisible,
      },
      handleScroll: { vertTouchDrag: false },
    })

    candleSeries = chart.addCandlestickSeries({
      upColor: '#10b981',
      downColor: '#ef4444',
      borderDownColor: '#ef4444',
      borderUpColor: '#10b981',
      wickDownColor: '#ef4444',
      wickUpColor: '#10b981',
    })

    const chartData: CandlestickData[] = props.data.map((item) => ({
      time: item.time as Time,
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    if (props.data.length > 0 && props.data[0].volume !== undefined) {
      const volumeSeries = chart.addHistogramSeries({
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      })

      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.8, bottom: 0 },
      })

      volumeSeries.setData(
        props.data.map((item) => ({
          time: item.time as Time,
          value: item.volume || 0,
          color: item.close >= item.open ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)',
        }))
      )
    }

    if (props.markers && props.markers.length > 0) {
      candleSeries.setMarkers(
        props.markers.map((m) => ({
          time: m.time as Time,
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        }))
      )
    }

    // Render overlay lines
    if (props.overlays && props.overlays.length > 0) {
      for (const overlay of props.overlays) {
        const lineSeries = chart.addLineSeries({
          color: overlay.color,
          lineWidth: Math.min(Math.max(overlay.lineWidth || 1, 1), 4) as 1 | 2 | 3 | 4,
          priceScaleId: overlay.priceScaleId || 'right',
          lastValueVisible: false,
          priceLineVisible: false,
        })

        lineSeries.setData(
          overlay.data.map(d => ({
            time: d.time as Time,
            value: d.value,
          }))
        )
      }
    }

    // Show the last 100 candles by default (user can scroll to see more)
    if (props.data.length > 100) {
      chart.timeScale().setVisibleLogicalRange({
        from: props.data.length - 100,
        to: props.data.length - 1,
      })
    } else {
      chart.timeScale().fitContent()
    }

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        if (chart && width > 0 && height > 0) {
          chart.applyOptions({ width, height })
        }
      }
    })
    resizeObserver.observe(containerRef)

    onCleanup(() => {
      resizeObserver.disconnect()
      if (chart) {
        chart.remove()
        chart = undefined
        candleSeries = undefined
      }
    })
  })

  createEffect(() => {
    if (!candleSeries || props.data.length === 0) return

    const chartData: CandlestickData[] = props.data.map((item) => ({
      time: item.time as Time,
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    if (props.markers && props.markers.length > 0) {
      candleSeries.setMarkers(
        props.markers.map((m) => ({
          time: m.time as Time,
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        }))
      )
    }

    if (props.data.length > 100) {
      chart?.timeScale().setVisibleLogicalRange({
        from: props.data.length - 100,
        to: props.data.length - 1,
      })
    } else {
      chart?.timeScale().fitContent()
    }
  })

  return (
    <div
      ref={containerRef}
      class="w-full rounded-lg border border-gray-200/60 overflow-hidden"
      style={{ height: `${props.height || 400}px` }}
    />
  )
}

export default KlineChart
