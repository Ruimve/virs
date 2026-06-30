import type { FlowStepStatus } from '@/components/FlowStep';

export type CheckKey = 'llm' | 'exchange' | 'api' | 'system' | 'trading';

export interface CheckData {
  providers?: string[];
  exchange?: string;
  version?: string;
  cpuPct?: number;
  memPct?: number;
  coreCount?: number;
  host?: string;
  paperMode?: boolean;
  message?: string;
}

export interface CheckItem {
  key: CheckKey;
  label: string;
  description: string;
  status: FlowStepStatus;
  detail: string;
  data?: CheckData;
}
