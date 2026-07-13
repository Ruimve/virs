import { memo } from 'react';

export interface LogoProps {
  className?: string;
  onClick?: () => void;
}

/**
 * VIRS Logo — flowing gradient text that shifts continuously.
 * Matches the global indigo/violet accent palette.
 *
 * When `onClick` is provided, the span becomes keyboard-accessible
 * (role="button", tabIndex, Enter/Space handling).
 */
export const Logo = memo(({ className = '', onClick }: LogoProps) => {
  const isInteractive = !!onClick;

  return (
    <span
      className={`text-sm md:text-base font-extralight tracking-[0.25em] ${className}`}
      style={{
        background: 'linear-gradient(90deg, #6366f1, #8b5cf6, #a78bfa, #c4b5fd, #8b5cf6, #6366f1)',
        backgroundSize: '300% 100%',
        WebkitBackgroundClip: 'text',
        WebkitTextFillColor: 'transparent',
        animation: 'virs-flow 6s linear infinite',
        ...(isInteractive ? { cursor: 'pointer' } : {}),
      }}
      onClick={onClick}
      onKeyDown={
        isInteractive
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onClick!();
              }
            }
          : undefined
      }
      role={isInteractive ? 'button' : undefined}
      tabIndex={isInteractive ? 0 : undefined}
      aria-label={isInteractive ? 'Open menu' : undefined}
    >
      VIRS
    </span>
  );
});
