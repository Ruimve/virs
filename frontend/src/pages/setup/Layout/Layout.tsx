import { Outlet } from 'react-router-dom';
import Logo from '@/components/Logo';
import Theme from '@/components/Theme';
import { Check } from '@/components/Icon';
import { useWizard } from '../context/WizardContext';
import { TOTAL_SETUP_STEPS } from '../context/WizardContext/consts';

export const Layout = () => {
  const { wizard } = useWizard();

  return (
    <div className="h-dvh bg-base flex flex-col relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-accent/[0.03] blur-[120px]" />
      </div>

      {/* Top bar */}
      <div className="relative z-10 flex items-center justify-between px-4 md:px-8 h-14 md:h-16 border-b border-line-subtle">
        <Logo />

        {/* Step indicator */}
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-on-surface-tertiary md:hidden">
            Step {wizard.current_step - 1}/{TOTAL_SETUP_STEPS}
          </span>
          <div className="hidden md:flex items-center gap-2">
            {Array.from({ length: TOTAL_SETUP_STEPS }, (_, i) => {
              const stepNum = i + 1;
              const isActive = stepNum === wizard.current_step - 1;
              const isCompleted = stepNum < wizard.current_step - 1;
              return (
                <div key={i} className="flex items-center gap-2">
                  <div
                    className={`w-7 h-7 rounded-full flex items-center justify-center text-[11px] font-medium transition-all duration-300 ${
                      isActive
                        ? 'bg-accent/80 text-white'
                        : isCompleted
                          ? 'bg-accent-muted text-accent border border-accent-muted'
                          : 'bg-surface-1 text-on-surface-faint border border-line-default'
                    }`}
                  >
                    {isCompleted ? <Check className="w-3.5 h-3.5" strokeWidth={2.5} /> : stepNum}
                  </div>
                  {i < TOTAL_SETUP_STEPS - 1 && (
                    <div
                      className={`w-6 h-[1px] ${isCompleted ? 'bg-accent/40' : 'bg-line-default'}`}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="flex items-center gap-1">
          <Theme />
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 h-0">
        <Outlet />
      </div>
    </div>
  );
};

export default Layout;
