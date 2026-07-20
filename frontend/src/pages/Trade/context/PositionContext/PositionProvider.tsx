import { useCallback, useMemo, useState, type ReactNode } from 'react';
import { usePositionWs, type PositionWsEvent } from '@/service/ws';
import { useBot } from '../BotContext';
import { PositionContext, type PositionSide, type PositionsBySide } from '.';

const isHolding = (p: PositionWsEvent | null): boolean =>
  !!p && p.status !== 'closed' && p.quantity > 0;

export const PositionProvider = ({ children }: { children: ReactNode }) => {
  const { bot } = useBot();
  const [positions, setPositions] = useState<PositionsBySide>({ long: null, short: null });

  usePositionWs(bot?.symbol, (event) => {
    if (event.symbol !== bot?.symbol) return;

    // side 来自后端 position_to_ws_json 的 position_side_str，值为 'long' | 'short'。
    const side = event.side as PositionSide;
    if (side !== 'long' && side !== 'short') return;

    if (event.status === 'closed' || event.quantity <= 0) {
      setPositions((prev) => ({ ...prev, [side]: null }));
      return;
    }
    setPositions((prev) => ({ ...prev, [side]: event }));
  });

  const getPosition = useCallback((side: PositionSide) => positions[side], [positions]);

  const hasAnyPosition = useCallback(
    () => isHolding(positions.long) || isHolding(positions.short),
    [positions],
  );

  const value = useMemo(
    () => ({ positions, getPosition, hasAnyPosition }),
    [positions, getPosition, hasAnyPosition],
  );

  return <PositionContext.Provider value={value}>{children}</PositionContext.Provider>;
};
