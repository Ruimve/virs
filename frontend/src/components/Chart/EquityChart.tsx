import { type Component, createEffect, onMount } from 'solid-js'
import {
  type IChartApi,
  type ISeriesApi,
  type LineData,
  LineSeries,
} from 'lightweight-charts'
import SolidChart from './SolidChart'
import { toLocaleTime } from './SolidChart/locale/zh_CN';

interface EquityChartProps {
  data: Array<[string, number]>
  height?: number
  initialBalance?: number
}

const EquityChart: Component<EquityChartProps> = (props) => {
  let chart: IChartApi | undefined
  let lineSeries: ISeriesApi<'Line'> | undefined

  const setChart = (c: IChartApi | undefined) => {
    chart = c
  }

  onMount(() => {
    if (!chart) return

    lineSeries = chart.addSeries(LineSeries, {
      color: '#6366f1',
      lineWidth: 2,
      priceLineVisible: true,
      lastValueVisible: true,
    })

    const chartData: LineData[] = props.data.map((item) => ({
      time: toLocaleTime(new Date(item[0]).getTime() / 1000),
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
  })

  createEffect(() => {
    if (!lineSeries || props.data.length === 0) return

    const chartData: LineData[] = props.data.map((item) => ({
      time: toLocaleTime(new Date(item[0]).getTime() / 1000),
      value: item[1],
    }))

    lineSeries.setData(chartData)
    chart?.timeScale().fitContent()
  })

  return <SolidChart onLoad={setChart} height={props.height} />
}

export default EquityChart
