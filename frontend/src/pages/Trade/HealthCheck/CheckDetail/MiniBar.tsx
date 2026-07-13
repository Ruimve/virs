import { memo } from 'react';
import ProgressBar from '@/components/ProgressBar';

const MiniBar = ({ pct }: { pct: number }) => <ProgressBar pct={pct} size="sm" />;

export default memo(MiniBar);
