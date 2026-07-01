import { useMemo } from 'react';
import Logo from '@/components/Logo';
import Theme from '@/components/Theme';
import { Check } from '@/components/Icon';
import { TOTAL_SETUP_STEPS } from '../../context/WizardContext/consts';
import { useWizard } from '../../context/WizardContext';

const Header = () => {
  const { wizard } = useWizard();

  const simpleSteps = useMemo(() => {
    return `Step ${wizard.current_step - 1}/${TOTAL_SETUP_STEPS}`;
  }, [wizard.current_step]);

  const fullSteps = useMemo(() => {
    return Array.from({ length: TOTAL_SETUP_STEPS }, (_, i) => {
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
            <div className={`w-6 h-[1px] ${isCompleted ? 'bg-accent/40' : 'bg-line-default'}`} />
          )}
        </div>
      );
    });
  }, [wizard.current_step]);

  return (
    <div className="relative z-10 flex items-center h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      {/* Left: logo (clickable on mobile to open drawer) + bot info */}
      <div className="flex items-center gap-2 pl-4 md:pl-8 shrink-0">
        <Logo />
      </div>

      {/* Center: tabs (desktop) */}
      <div className="flex items-center justify-center flex-1 gap-2">
        <span className="text-[11px] text-on-surface-tertiary md:hidden">{simpleSteps}</span>
        <div className="hidden md:flex items-center gap-2">{fullSteps}</div>
      </div>
      {/* Right: actions (desktop) */}
      <div className="hidden md:flex items-center gap-2 pr-8 shrink-0">
        <Theme />
      </div>

      {/* Right: theme toggle (mobile) */}
      <div className="md:hidden flex items-center pr-4 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

export default Header;
