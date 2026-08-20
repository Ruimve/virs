import { memo, useMemo } from 'react';

interface LeverageSliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
}

export const LeverageSlider = memo(
  ({ value, min, max, step = 1, onChange }: LeverageSliderProps) => {
    const pct = useMemo(() => ((value - min) / (max - min)) * 100, [value, min, max]);

    return (
      <div>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          className="leverage-slider w-full"
          style={{
            background: `linear-gradient(to right, var(--color-accent) ${pct}%, var(--color-surface-3) ${pct}%)`,
          }}
        />
        <div className="flex justify-between mt-1.5">
          <span className="font-mono text-2xs text-on-surface-muted">{min}x</span>
          <span className="font-mono text-2xs text-on-surface-muted">{max}x</span>
        </div>
      </div>
    );
  },
);
