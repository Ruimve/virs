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
    // Read theme-aware CSS variables for chart colors
    const cs = getComputedStyle(el)
    const bgBase = cs.getPropertyValue('--bg-base').trim() || '#ffffff'
    const textOnSurfaceTertiary = cs.getPropertyValue('--text-on-surface-tertiary').trim() || '#94a3b8'
    const borderDefault = cs.getPropertyValue('--border-default').trim() || '#e2e8f0'
    const borderSubtle = cs.getPropertyValue('--border-subtle').trim() || '#f1f5f9'

    chart = createChart(el, {
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
      class="w-full rounded-lg border border-line-default overflow-hidden"
      style={{ height: `${props.height || 400}px` }}
    />
  )
}

export default SolidChart
