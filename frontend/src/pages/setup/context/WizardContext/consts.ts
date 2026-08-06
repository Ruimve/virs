export const WizardStep = {
  Loading: 0,
  Login: 1,
  SelectBotType: 2,
  ConfigureLlm: 3,
  SelectExchange: 4,
  ConfigureParams: 5,
  ConfigureOptimization: 6,
  ReviewLaunch: 7,
  Trading: 8,
};

export type WizardStepValue = (typeof WizardStep)[keyof typeof WizardStep];

export interface WizardState {
  current_step: WizardStepValue;
  exchange: string;
  bot_type: 'auto';
  paper_mode: boolean;
  llm_provider: string;
  llm_model: string;
  bot_params: Record<string, string>;
  auto_optimize: boolean;
  bot_id: string;
}

export const STEP_LABELS: Record<number, string> = {
  [WizardStep.SelectBotType]: 'Strategy',
  [WizardStep.ConfigureLlm]: 'AI Model',
  [WizardStep.SelectExchange]: 'Exchange',
  [WizardStep.ConfigureParams]: 'Parameters',
  [WizardStep.ConfigureOptimization]: 'Optimization',
  [WizardStep.ReviewLaunch]: 'Review',
};
export const TOTAL_SETUP_STEPS = 6;

export const DEFAULT_STATE: WizardState = {
  current_step: WizardStep.Loading,
  exchange: '',
  bot_type: 'auto',
  paper_mode: true,
  llm_provider: '',
  llm_model: '',
  bot_params: {},
  auto_optimize: false,
  bot_id: '',
};
