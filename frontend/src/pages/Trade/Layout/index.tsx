import { Outlet } from 'react-router-dom';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { BotProvider } from '../context/BotContext/BotProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { ShellProvider } from '@/layout/ShellContext';
import { AppShell } from '@/layout/AppShell';
import { TradeHeader } from './Header';

export const LayoutBox = () => {
  return (
    <PaperProvider>
      <BotProvider>
        <PositionProvider>
          <ShellProvider>
            <AppShell header={<TradeHeader />}>
              <Outlet />
            </AppShell>
          </ShellProvider>
        </PositionProvider>
      </BotProvider>
    </PaperProvider>
  );
};

export default LayoutBox;
