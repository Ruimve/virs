import { memo } from 'react';
import GridLevelsTab from './GridLevelsTab';
import { useBot } from '../../context/BotContext';

const Levels = () => {
  const { gridLevels } = useBot();
  return <GridLevelsTab gridLevels={gridLevels} />;
};

export default Levels;
