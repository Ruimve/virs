import { type Component, createEffect, onCleanup, onMount } from 'solid-js'
import {
  createChart,
  type IChartApi,
  type ISeriesApi,
  type LineData,
  type Time,
  ColorType,
} from 'lightweight-charts'

interface EquityChartProps {
  data: Array<[string, number]>
  height?: number
  initialBalance?: number
}

const EquityChart: Component<EquityChartProps> = (props) => {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let lineSeries: ISeriesApi<'Line'> | undefined

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
      },
      handleScroll: { vertTouchDrag: false },
    })

    lineSeries = chart.addLineSeries({
      color: '#6366f1',
      lineWidth: 2,
      priceLineVisible: true,
      lastValueVisible: true,
    })

    const chartData: LineData[] = props.data.map((item) => ({
      time: (new Date(item[0]).getTime() / 1000) as Time,
      value: item[1],
    }))

    lineSeries.setData(chartData)

    if (props.initialBalance && props.data.length > 0) {
      lineSeries.createPriceLine({
        price: props.initialBalance,
        color: '#9ca3af',
        lineWidth: 1,
        lineStyle: 2,
        axisLabelVisible: true,
        title: 'Initial',
      })
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
        lineSeries = undefined
      }
    })
  })

  createEffect(() => {
    if (!lineSeries || props.data.length === 0) return

    const chartData: LineData[] = props.data.map((item) => ({
      time: (new Date(item[0]).getTime() / 1000) as Time,
      value: item[1],
    }))

    lineSeries.setData(chartData)
    chart?.timeScale().fitContent()
  })

  return (
    <div
      ref={containerRef}
      class="w-full rounded-lg border border-gray-200/60 overflow-hidden"
      style={{ height: `${props.height || 250}px` }}
    />
  )
}

export default EquityChart
