import { memo } from 'react'
import GridLevelsTab from './GridLevelsTab'
import { useBot } from '../../context/BotContext'

const Levels = () => {
  const { gridLevels, loading } = useBot()
  return <GridLevelsTab gridLevels={gridLevels} loading={loading} />
}

export default memo(Levels)
