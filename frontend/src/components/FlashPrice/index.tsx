import { memo, useEffect, useRef, useState } from 'react';

interface FlashPriceProps {
  price: number;
  className?: string;
  /** How long the flash color stays before reverting to default (ms). Default: 1500 */
  flashDuration?: number;
}

/**
 * Price display with directional color on change.
 * - Price up: text turns green briefly, then reverts to className color
 * - Price down: text turns red briefly, then reverts to className color
 * - No change: keeps the color provided via `className`
 *
 * The flash color is applied through inline style so it reliably overrides any
 * conflicting `text-*` color utility passed via `className`. Tailwind text-color
 * utilities share the same specificity, so without this the caller's base color
 * (e.g. `text-on-surface`) would win based on CSS source order and the up/down
 * color would never reliably render.
 */
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

      // Always set up timer so flash reverts to default color after the duration.
      // If price didn't change but flashDuration did, this still ensures cleanup.
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
