import { useEffect, useState, type ReactNode } from 'react';
import { useLocation, useParams } from 'react-router-dom';
import {
  getAutoBotDetail,
  getGridBotDetail,
  type AutoBot,
  type AutoTrade,
  type GridBot,
  type GridLevelInfo,
  type GridTrade,
} from '@/service';
import { BotContext } from '.';

export const BotProvider = ({ children }: { children: ReactNode }) => {
  const location = useLocation();
  const params = useParams();
  const [bot, setBot] = useState<AutoBot | GridBot | null>(null);
  const [trades, setTrades] = useState<AutoTrade[] | GridTrade[]>([]);
  const [gridLevels, setGridLevels] = useState<GridLevelInfo[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const botType = location.pathname.split('/')[2];
    const botId = params?.botId;
    if (!botId) return;

    if (botType === 'auto') {
      setLoading(true);
      getAutoBotDetail(botId)
        .then((bot) => {
          setBot(bot?.data?.bot || null);
          setTrades([]);
        })
        .finally(() => {
          setLoading(false);
        });
    } else if (botType === 'grid') {
      setLoading(true);
      getGridBotDetail(botId)
        .then((bot) => {
          setBot(bot?.data?.bot || null);
          setTrades(bot?.data?.trades || []);
          setGridLevels(bot?.data?.grid_levels || []);
        })
        .finally(() => {
          setLoading(false);
        });
    }
  }, [location.pathname, params?.botId]);

  return (
    <BotContext.Provider value={{ bot, trades, gridLevels, loading }}>
      {children}
    </BotContext.Provider>
  );
};
