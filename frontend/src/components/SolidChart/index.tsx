import { type Component, onCleanup } from 'solid-js'
import {
  createChart,
  type IChartApi,
  ColorType,
} from 'lightweight-charts'

export interface SolidChartProps {
  onLoad: (chart: IChartApi | undefined) => void
  height?: number
  timeVisible?: boolean
  secondsVisible?: boolean
}

/**
 * Base chart container — handles chart creation, resize, and cleanup.
 * Passes the IChartApi instance back via `ref` callback so the parent
 * can create series and set data.
 *
 * Chart is created synchronously via the div ref callback so that
 * the parent's onMount sees a valid chart instance.
 */
const SolidChart: Component<SolidChartProps> = (props) => {
  let chart: IChartApi | undefined

  const containerRef = (el: HTMLDivElement) => {
    chart = createChart(el, {
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
        timeVisible: props.timeVisible ?? true,
        secondsVisible: props.secondsVisible ?? false,
      },
      handleScroll: { vertTouchDrag: false },
    })

    props.onLoad(chart)

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        if (chart && width > 0 && height > 0) {
          chart.applyOptions({ width, height })
        }
      }
    })
    resizeObserver.observe(el)

    onCleanup(() => {
      resizeObserver.disconnect()
      if (chart) {
        chart.remove()
        chart = undefined
        props.onLoad(undefined)
      }
    })
  }

  return (
    <div
      ref={containerRef}
      class="w-full rounded-lg border border-gray-200/60 overflow-hidden"
      style={{ height: `${props.height || 400}px` }}
    />
  )
}

export default SolidChart
