import { PaperProvider } from '../context/PaperContext'
import { BotProvider } from '../context/BotContext'
import { PositionProvider } from '../context/PositionContext'
import { HeaderProvider } from '../components/Header/context'
import Layout from './Layout'

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
  )
}

export default LayoutBox
