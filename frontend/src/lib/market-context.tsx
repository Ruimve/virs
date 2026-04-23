import { createContext, useContext, createSignal, type JSX } from 'solid-js'

export type MarketType = 'perpetual' | 'spot'

interface MarketContextType {
  marketType: () => MarketType
  setMarketType: (t: MarketType) => void
  label: () => string
}

const MarketContext = createContext<MarketContextType>()

export function MarketProvider(props: { children: JSX.Element }) {
  const [marketType, setMarketType] = createSignal<MarketType>(
    (localStorage.getItem('virs_market_type') as MarketType) || 'perpetual'
  )

  const handleSet = (t: MarketType) => {
    setMarketType(t)
    localStorage.setItem('virs_market_type', t)
  }

  const label = () => marketType() === 'perpetual' ? '永续合约' : '现货'

  return (
    <MarketContext.Provider value={{ marketType, setMarketType: handleSet, label }}>
      {props.children}
    </MarketContext.Provider>
  )
}

export function useMarket() {
  const ctx = useContext(MarketContext)
  if (!ctx) throw new Error('useMarket must be used within MarketProvider')
  return ctx
}
