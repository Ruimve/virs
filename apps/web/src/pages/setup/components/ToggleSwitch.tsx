import { memo } from 'react';

interface ToggleSwitchProps {
  on: boolean;
  onClick: () => void;
  warning?: boolean;
}

export const ToggleSwitch = memo(({ on, onClick, warning }: ToggleSwitchProps) => (
  <button
    type="button"
    onClick={onClick}
    className={`relative w-11 h-6.5 rounded-full border-none cursor-pointer transition-colors shrink-0 ${
      on ? (warning ? 'bg-warning' : 'bg-accent') : 'bg-surface-3'
    }`}
  >
    <span
      className={`absolute top-0.5 left-0.5 w-5.5 h-5.5 rounded-full bg-white shadow-sm transition-transform ${
        on ? 'translate-x-4.5' : ''
      }`}
    />
  </button>
));
