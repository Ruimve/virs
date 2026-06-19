import { useRef, useEffect } from 'react'
import { type IChartApi, type ISeriesApi, type LineData, LineSeries } from 'lightweight-charts'
import ReactChart from './ReactChart'
import { toLocaleTime } from './ReactChart/locale/zh_CN'

interface EquityChartProps {
  data: Array<[string, number]>
  height?: number
  initialBalance?: number
}

function EquityChart({ data, height, initialBalance }: EquityChartProps) {
  const chartRef = useRef<IChartApi | undefined>(undefined)
  const lineSeriesRef = useRef<ISeriesApi<'Line'> | undefined>(undefined)
  const initializedRef = useRef(false)

  const setChart = (c: IChartApi | undefined) => {
    chartRef.current = c
  }

  useEffect(() => {
    const chart = chartRef.current
    if (!chart || initializedRef.current) return
    initializedRef.current = true

    const lineSeries = chart.addSeries(LineSeries, {
      color: '#6366f1',
      lineWidth: 2,
      priceLineVisible: true,
      lastValueVisible: true,
    })
    lineSeriesRef.current = lineSeries

    const chartData: LineData[] = data.map((item) => ({
      time: toLocaleTime(new Date(item[0]).getTime() / 1000),
      value: item[1],
    }))

    lineSeries.setData(chartData)

    if (initialBalance && data.length > 0) {
      lineSeries.createPriceLine({
        price: initialBalance,
        color: '#9ca3af',
        lineWidth: 1,
        lineStyle: 2,
        axisLabelVisible: true,
        title: 'Initial',
      })
    }

    chart.timeScale().fitContent()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const lineSeries = lineSeriesRef.current
    if (!lineSeries || data.length === 0) return

    const chartData: LineData[] = data.map((item) => ({
      time: toLocaleTime(new Date(item[0]).getTime() / 1000),
      value: item[1],
    }))

    lineSeries.setData(chartData)
    chartRef.current?.timeScale().fitContent()
  }, [data])

  return <ReactChart onLoad={setChart} height={height} />
}

export default EquityChart
