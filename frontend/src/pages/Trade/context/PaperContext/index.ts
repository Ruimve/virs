import { createContext, useContext } from 'react';

interface PaperContextType {
  enabled: boolean;
  loading: boolean;
  toggle: () => Promise<void>;
  refresh: () => Promise<void>;
}

export const PaperContext = createContext<PaperContextType | null>(null);

export function usePaper() {
  const ctx = useContext(PaperContext);
  if (!ctx) throw new Error('usePaper must be used within PaperProvider');
  return ctx;
}
