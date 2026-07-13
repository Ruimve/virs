import { memo, useEffect, useRef, useState } from 'react';

interface FlashPriceProps {
  price: number;
  className?: string;
}

/**
 * Price display with directional color on change.
 * - Price up: text turns green
 * - Price down: text turns red
 * - No change: keeps the color provided via `className` (default on first render)
 *
 * The flash color is applied through inline style so it reliably overrides any
 * conflicting `text-*` color utility passed via `className`. Tailwind text-color
 * utilities share the same specificity, so without this the caller's base color
 * (e.g. `text-on-surface`) would win based on CSS source order and the up/down
 * color would never reliably render.
 */
export const FlashPrice = memo(({ price, className = '' }: FlashPriceProps) => {
  const prevRef = useRef<number>(price);
  const [flash, setFlash] = useState<'up' | 'down' | null>(null);

  useEffect(() => {
    if (price === prevRef.current) return;
    const prev = prevRef.current;
    prevRef.current = price;

    if (price > prev) setFlash('up');
    else if (price < prev) setFlash('down');
  }, [price]);

  const flashStyle =
    flash === 'up'
      ? { color: 'var(--color-success-text)' }
      : flash === 'down'
        ? { color: 'var(--color-danger-text)' }
        : undefined;

  return (
    <span className={`font-mono font-medium shrink-0 tabular-nums ${className}`} style={flashStyle}>
      {price > 0 ? price.toFixed(2) : '-'}
    </span>
  );
});
