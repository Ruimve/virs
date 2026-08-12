import { Outlet } from 'react-router-dom';
import { AppShell } from '@/layout/AppShell';
import { ShellProvider } from '@/context/ShellContext/ShellProvider';
import { BotProvider } from '../context/BotContext/BotProvider';
import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { TradeHeader } from './Header';

export const LayoutBox = () => {
  return (
    <BotProvider>
      <PaperProvider>
        <PositionProvider>
          <ShellProvider>
            <AppShell header={<TradeHeader />}>
              <Outlet />
            </AppShell>
          </ShellProvider>
        </PositionProvider>
      </PaperProvider>
    </BotProvider>
  );
};

export default LayoutBox;
