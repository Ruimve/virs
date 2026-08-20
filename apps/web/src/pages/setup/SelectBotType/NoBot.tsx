import { memo } from 'react';
import { Bot, Brain } from '@/components/Icon';
import { FormRow, FormCard } from '../components';

export const NoBot = memo(
  ({
    botType,
    changeBotType,
  }: {
    botType: 'chat' | 'agent';
    changeBotType: (value: 'chat' | 'agent') => void;
  }) => {
    return (
      <>
        <FormCard>
          <FormRow
            icon={<Brain width={20} height={20} strokeWidth={1.5} />}
            title="Chat"
            description="AI-driven conversational trading. Analyzes market conditions and executes trades autonomously."
            selected={botType === 'chat'}
            type="ai"
            onClick={() => changeBotType('chat')}
          />
          <FormRow
            icon={<Bot width={20} height={20} strokeWidth={1.5} />}
            title="Agent"
            description="Autonomous agent that executes complex trading strategies with minimal supervision."
            selected={botType === 'agent'}
            type="accent"
            onClick={() => changeBotType('agent')}
          />
        </FormCard>
        {botType === 'agent' && (
          <p className="text-xs text-on-surface-muted mt-3 px-1">Agent is coming soon.</p>
        )}
      </>
    );
  },
);
