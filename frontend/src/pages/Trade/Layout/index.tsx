import { PaperProvider } from '../context/PaperContext/PaperProvider';
import { BotProvider } from '../context/BotContext/BotProvider';
import { PositionProvider } from '../context/PositionContext/PositionProvider';
import { HeaderProvider } from './Header/HeaderContext/HeaderProvider';
import Layout from './Layout';

export const LayoutBox = () => {
  return (
    <PaperProvider>
      <BotProvider>
        <PositionProvider>
          <HeaderProvider>
            <Layout />
          </HeaderProvider>
        </PositionProvider>
      </BotProvider>
    </PaperProvider>
  );
};

export default LayoutBox;
