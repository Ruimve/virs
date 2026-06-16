/**
 * VIRS Logo — flowing gradient text that shifts continuously.
 * Matches the global indigo/violet accent palette.
 */
export function Logo({ className = '', onClick = () => {} }: { className?: string; onClick?: () => void }) {
  return (
    <span
      className={`text-sm md:text-base font-extralight tracking-[0.25em] ${className}`}
      style={{
        background: 'linear-gradient(90deg, #6366f1, #8b5cf6, #a78bfa, #c4b5fd, #8b5cf6, #6366f1)',
        backgroundSize: '300% 100%',
        WebkitBackgroundClip: 'text',
        WebkitTextFillColor: 'transparent',
        animation: 'virs-flow 6s linear infinite',
      }}
      onClick={onClick}
    >
      VIRS
    </span>
  )
}
