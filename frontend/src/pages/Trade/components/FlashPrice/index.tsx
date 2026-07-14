import { memo, useEffect, useRef, useState } from 'react';

interface FlashPriceProps {
  price: number;
  className?: string;

  flashDuration?: number;
}


export const FlashPrice = memo(
  ({ price, className = '', flashDuration = 1500 }: FlashPriceProps) => {
    const prevRef = useRef<number>(price);
    const [flash, setFlash] = useState<'up' | 'down' | null>(null);

    useEffect(() => {
      if (price !== prevRef.current) {
        const prev = prevRef.current;
        prevRef.current = price;
        if (price > prev) setFlash('up');
        else if (price < prev) setFlash('down');
      }


      const timer = setTimeout(() => setFlash(null), flashDuration);
      return () => clearTimeout(timer);
    }, [price, flashDuration]);

    const flashStyle =
      flash === 'up'
        ? { color: 'var(--color-success-text)' }
        : flash === 'down'
          ? { color: 'var(--color-danger-text)' }
          : undefined;

    return (
      <span
        className={`font-mono font-medium shrink-0 tabular-nums ${className}`}
        style={flashStyle}
      >
        {price > 0 ? price.toFixed(2) : '-'}
      </span>
    );
  },
);
