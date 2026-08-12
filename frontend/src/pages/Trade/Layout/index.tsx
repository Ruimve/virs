import { Outlet } from 'react-router-dom';
import { Layout } from '@/layout';
import { BotProvider } from '../context/BotContext/BotProvider';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { TradeHeader } from './Header';

export const LayoutBox = () => {
  return (
    <BotProvider>
      <PaperProvider>
        <PositionProvider>
          <Layout header={<TradeHeader />}>
            <Outlet />
          </Layout>
        </PositionProvider>
      </PaperProvider>
    </BotProvider>
  );
};

export default LayoutBox;
