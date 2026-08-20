import { useMemo } from 'react';
import { Brand } from '@/components/Logo';
import { Theme } from '@/components/Theme';
import { Check } from '@/components/Icon';
import { TOTAL_SETUP_STEPS } from '../../context/WizardContext/define';
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
            className={`w-7 h-7 rounded-full flex items-center justify-center text-caption font-medium transition-all duration-300 ${
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
            <div className={`w-6 h-px ${isCompleted ? 'bg-accent/40' : 'bg-line-default'}`} />
          )}
        </div>
      );
    });
  }, [wizard.current_step]);

  return (
    <div className="relative z-10 flex items-center h-14 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      <div className="flex items-center gap-2 pl-3 md:pl-4 shrink-0">
        <div className="md:hidden">
          <Brand size={20} />
        </div>
        <div className="hidden md:block">
          <Brand size={24} />
        </div>
      </div>

      <div className="flex items-center justify-center flex-1 gap-2">
        <span className="text-caption text-on-surface-tertiary md:hidden">{simpleSteps}</span>
        <div className="hidden md:flex items-center gap-2">{fullSteps}</div>
      </div>
      <div className="flex items-center gap-2 pr-3 md:pr-4 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

export default Header;
