import { type Component, createEffect, onMount } from 'solid-js'
import {
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  createSeriesMarkers,
} from 'lightweight-charts'
import SolidChart from './SolidChart'
import { toLocaleTime } from './SolidChart/locale/zh_CN';
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
  let chart: IChartApi | undefined
  let candleSeries: ISeriesApi<'Candlestick'> | undefined

  const setChart = (c: IChartApi | undefined) => {
    console.log(c);
    chart = c
  }

  onMount(() => {
    if (!chart) return

    // Determine time scale settings based on data density
    let timeVisible = true
    const secondsVisible = false
    if (props.data.length >= 2) {
      const firstTime = props.data[0].time
      const lastTime = props.data[props.data.length - 1].time
      const spanHours = (lastTime - firstTime) / 3600
      if (spanHours > 2160) {
        timeVisible = false
      }
    }

    chart.applyOptions({
      timeScale: { timeVisible, secondsVisible },
    })

    candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#10b981',
      downColor: '#ef4444',
      borderDownColor: '#ef4444',
      borderUpColor: '#10b981',
      wickDownColor: '#ef4444',
      wickUpColor: '#10b981',
    })

    const chartData: CandlestickData[] = props.data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    // Update time format based on data span
    if (props.data.length >= 2) {
      const firstTime = props.data[0].time
      const lastTime = props.data[props.data.length - 1].time
      const spanHours = (lastTime - firstTime) / 3600
      const newTimeVisible = spanHours <= 2160
      if (newTimeVisible !== timeVisible) {
        chart.applyOptions({
          timeScale: { timeVisible: newTimeVisible },
        })
      }
    }

    // After fitContent, optionally zoom to last N candles on next frame
    if (props.data.length > 100) {
      requestAnimationFrame(() => {
        chart?.timeScale().setVisibleLogicalRange({
          from: props.data.length - 100,
          to: props.data.length - 1,
        })
      })
    }

    if (props.data.length > 0 && props.data[0].volume !== undefined) {
      const volumeSeries = chart.addSeries(HistogramSeries, {
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      })

      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.8, bottom: 0 },
      })

      volumeSeries.setData(
        props.data.map((item) => ({
          time: toLocaleTime(item.time),
          value: item.volume || 0,
          color: item.close >= item.open ? 'rgba(16, 185, 129, 0.3)' : 'rgba(239, 68, 68, 0.3)',
        }))
      )
    }

    if (props.markers && props.markers.length > 0) {
      createSeriesMarkers(candleSeries,
        props.markers.map((m) => ({
          time: toLocaleTime(m.time),
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

    // Show the last 100 candles by default (user can scroll to see more)
    if (props.data.length > 100) {
      chart.timeScale().setVisibleLogicalRange({
        from: props.data.length - 100,
        to: props.data.length - 1,
      })
    } else {
      chart.timeScale().fitContent()
    }
  })

  createEffect(() => {
    if (!candleSeries || props.data.length === 0) return

    const chartData: CandlestickData[] = props.data.map((item) => ({
      time: toLocaleTime(item.time),
      open: item.open,
      high: item.high,
      low: item.low,
      close: item.close,
    }))

    candleSeries.setData(chartData)

    if (props.markers && props.markers.length > 0) {
      createSeriesMarkers(candleSeries,
        props.markers.map((m) => ({
          time: toLocaleTime(m.time),
          position: m.position,
          color: m.color,
          shape: m.shape,
          text: m.text,
        }))
      )
    }
  })

  return <SolidChart onLoad={setChart} height={props.height} />
}

export default KlineChart
