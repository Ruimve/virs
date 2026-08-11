import { memo } from 'react';
import { Bot, Brain } from '@/components/Icon';
import { FormRow, FormCard } from '../components';

export const NoBot = memo(
  ({
    botType,
    changeBotType,
  }: {
    botType: 'auto' | 'manual';
    changeBotType: (value: 'auto' | 'manual') => void;
  }) => {
    return (
      <>
        <FormCard>
          <FormRow
            icon={<Brain width={20} height={20} strokeWidth={1.5} />}
            title="Auto Bot"
            description="AI-driven fully automated trading. Analyzes market conditions and executes trades autonomously."
            selected={botType === 'auto'}
            type="ai"
            onClick={() => changeBotType('auto')}
          />
          <FormRow
            icon={<Bot width={20} height={20} strokeWidth={1.5} />}
            title="Manual Bot"
            description="Manual trading with AI-assisted signals. You stay in control while AI provides insights."
            selected={botType === 'manual'}
            type="accent"
            onClick={() => changeBotType('manual')}
          />
        </FormCard>
        {botType === 'manual' && (
          <p className="text-xs text-on-surface-muted mt-3 px-1">Manual Bot is coming soon.</p>
        )}
      </>
    );
  },
);
