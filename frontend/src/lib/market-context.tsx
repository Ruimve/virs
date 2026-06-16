import { createContext, useContext, useState, type ReactNode } from 'react'

export type MarketType = 'perpetual' | 'spot'

interface MarketContextType {
  marketType: MarketType
  setMarketType: (t: MarketType) => void
  label: string
}

const MarketContext = createContext<MarketContextType | null>(null)

export function MarketProvider({ children }: { children: ReactNode }) {
  const [marketType, setMarketTypeState] = useState<MarketType>(
    (localStorage.getItem('virs_market_type') as MarketType) || 'perpetual'
  )

  const setMarketType = (t: MarketType) => {
    setMarketTypeState(t)
    localStorage.setItem('virs_market_type', t)
  }

  const label = marketType === 'perpetual' ? '永续合约' : '现货'

  return (
    <MarketContext.Provider value={{ marketType, setMarketType, label }}>
      {children}
    </MarketContext.Provider>
  )
}

export function useMarket() {
  const ctx = useContext(MarketContext)
  if (!ctx) throw new Error('useMarket must be used within MarketProvider')
  return ctx
}
