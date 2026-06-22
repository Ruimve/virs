import { HeaderProvider } from '../components/Header/context'
import { BotProvider } from '../context/BotContext'
import { PaperProvider } from '../context/PaperContext'
import Layout from './Layout'

export const LayoutBox = () => {
  return (
    <PaperProvider>
      <BotProvider>
        <HeaderProvider>
          <Layout />
        </HeaderProvider>
      </BotProvider>
    </PaperProvider>
  )
}

export default LayoutBox
