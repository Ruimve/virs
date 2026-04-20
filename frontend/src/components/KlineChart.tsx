import { type Component, createEffect, onCleanup, onMount } from 'solid-js'
import {
  createChart,
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  type Time,
  ColorType,
} from 'lightweight-charts'

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
}

const KlineChart: Component<KlineChartProps> = (props) => {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let candleSeries: ISeriesApi<'Candlestick'> | undefined

  onMount(() => {
    if (!containerRef) return

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
        timeVisible: true,
        secondsVisible: false,
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

    chart.timeScale().fitContent()

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

    chart?.timeScale().fitContent()
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
