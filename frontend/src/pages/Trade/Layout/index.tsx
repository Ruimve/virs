import { Outlet } from 'react-router-dom';
import { Layout } from '@/layout';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { TradeHeader } from './Header';

export const LayoutBox = () => {
  return (
    <PaperProvider>
      <PositionProvider>
        <Layout header={<TradeHeader />}>
          <Outlet />
        </Layout>
      </PositionProvider>
    </PaperProvider>
  );
};

export default LayoutBox;
