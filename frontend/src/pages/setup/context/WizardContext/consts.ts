export const WizardStep = {
  Loading: 0,
  Login: 1,
  SelectBotType: 2,
  ConfigureLlm: 3,
  SelectExchange: 4,
  ConfigureParams: 5,
  ReviewLaunch: 6,
  Trading: 7,
};

export type WizardStepValue = (typeof WizardStep)[keyof typeof WizardStep];

// ── 向导状态 ──
export interface WizardState {
  current_step: WizardStepValue;
  exchange: string;
  market_type: 'perpetual' | 'spot';
  bot_type: 'grid' | 'auto' | '';
  paper_mode: boolean;
  llm_provider: string;
  llm_model: string;
  bot_params: Record<string, unknown>;
  bot_id: string;
}

export const STEP_LABELS: Record<number, string> = {
  [WizardStep.SelectBotType]: 'Strategy',
  [WizardStep.ConfigureLlm]: 'AI Model',
  [WizardStep.SelectExchange]: 'Exchange',
  [WizardStep.ConfigureParams]: 'Parameters',
  [WizardStep.ReviewLaunch]: 'Review',
};
export const TOTAL_SETUP_STEPS = 5;

export const DEFAULT_STATE: WizardState = {
  current_step: WizardStep.Loading,
  exchange: '',
  market_type: 'perpetual',
  bot_type: '',
  paper_mode: true,
  llm_provider: '',
  llm_model: '',
  bot_params: {},
  bot_id: '',
};
