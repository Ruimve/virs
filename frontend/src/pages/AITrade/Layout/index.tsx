import { HeaderProvider } from '../components/Header/context'
import { BotProvider } from '../context/BotContext'
import Layout from './Layout'

export const LayoutBox = () => {
  return (
    <BotProvider>
      <HeaderProvider>
        <Layout />
      </HeaderProvider>
    </BotProvider>
  )
}

export default LayoutBox
