import { Logo } from '@/components/Logo';

const Fallback = () => {
  return (
    <div className="loading-page min-h-dvh bg-base flex flex-col items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-90 h-90 md:w-125 md:h-125 rounded-full blur-glow opacity-55 bg-accent-glow-radial" />
        <div className="loading-grid absolute inset-0 opacity-60" />
      </div>

      <div className="relative flex flex-col items-center">
        <Logo size={80} className="mb-5.5 w-15 h-15 md:w-20 md:h-20 animate-logo-breathe" />

        <h1 className="font-display text-display md:text-4xl font-extralight tracking-hero-lg text-on-base leading-none select-none pl-[0.3em]">
          VIRS
        </h1>
        <p className="mt-2 text-2xs font-medium tracking-hero text-on-surface-muted select-none pl-[0.24em] uppercase">
          Quantitative Trading
        </p>

        <div className="mt-8 w-16 h-[1.5px] rounded-full overflow-hidden relative bg-line-default">
          <div className="absolute top-0 w-2/5 h-full rounded-full bg-accent animate-loading-sweep" />
        </div>
      </div>
    </div>
  );
};

export default Fallback;
