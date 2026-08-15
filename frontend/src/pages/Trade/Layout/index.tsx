import { Outlet, useParams } from 'react-router-dom';
import { Layout } from '@/layout';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { TradeHeader } from './Header';
import { BotProvider } from '../context/BotContext/BotProvider';
import { Suspense, useState } from 'react';
import { fetchBot } from '../context/BotContext/bot';
import Fallback from '@/components/Transition/Fallback';

export const LayoutBox = () => {
  const { botId } = useParams();

  const [promiseBot] = useState(() => fetchBot(botId));
  return (
    <Suspense fallback={<Fallback label="正在读取AI..." startProgress={90} progress={100} />}>
      <BotProvider promiseBot={promiseBot}>
        <PaperProvider>
          <PositionProvider>
            <Layout header={<TradeHeader />}>
              <Outlet />
            </Layout>
          </PositionProvider>
        </PaperProvider>
      </BotProvider>
    </Suspense>
  );
};

export default LayoutBox;
