import { memo } from 'react';
import { Progress } from '@/components/Progress';

const MiniBar = ({ pct }: { pct: number }) => <Progress pct={pct} size="sm" />;

export default memo(MiniBar);
