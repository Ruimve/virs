import { Suspense, useState } from 'react';
import { Outlet, useParams } from 'react-router-dom';
import Fallback from '@/components/Transition/Fallback';
import { Layout } from '@/layout';
import { fetchBot } from '@/context/BotContext/bot';
import { BotProvider } from '@/context/BotContext/BotProvider';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { TradeHeader } from './Header';

export const LayoutBox = () => {
  const { botId } = useParams();

  const [promiseBot] = useState(() => fetchBot(botId));
  return (
    <Suspense fallback={<Fallback label="正在读取AI..." startProgress={65} progress={90} />}>
      <BotProvider promiseBot={promiseBot}>
        <PaperProvider>
          <PositionProvider>
            <Suspense
              fallback={<Fallback label="正在加载页面..." startProgress={90} progress={100} />}
            >
              <Layout header={<TradeHeader />}>
                <Outlet />
              </Layout>
            </Suspense>
          </PositionProvider>
        </PaperProvider>
      </BotProvider>
    </Suspense>
  );
};

export default LayoutBox;
