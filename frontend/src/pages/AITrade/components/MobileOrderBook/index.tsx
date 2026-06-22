import { useState } from 'react'
import type { OrderBookData } from '@/service/types'
import OrderBookPanel from '../OrderBookPanel'

interface MobileOrderBookProps {
  orderBook: OrderBookData
}

export default function MobileOrderBook({ orderBook }: MobileOrderBookProps) {
  const [open, setOpen] = useState(false)

  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between px-4 py-2 text-xs text-on-surface-tertiary hover:bg-surface-2/50 transition-colors"
      >
        <span className="font-medium">订单簿</span>
        <svg
          className={`w-3.5 h-3.5 transition-transform ${open ? 'rotate-180' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
      {open && (
        <div className="h-64 border-t border-line-subtle">
          <OrderBookPanel orderBook={orderBook} />
        </div>
      )}
    </div>
  )
}
