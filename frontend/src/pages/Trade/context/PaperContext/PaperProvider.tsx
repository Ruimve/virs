import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { getPaperStatus } from '@/service';
import { PaperContext } from '.';

export function PaperProvider({ children }: { children: ReactNode }) {
  // null = engines not started yet (unknown mode), NOT the same as live trading
  const [enabled, setEnabled] = useState<boolean | null>(null);

  const refresh = useCallback(async () => {
    try {
      const res = await getPaperStatus();
      if (res.success && res.data) setEnabled(res.data.paper_mode);
    } catch {
      setEnabled(null);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const value = useMemo(() => {
    return {
      enabled,
      refresh,
    };
  }, [enabled, refresh]);

  return <PaperContext.Provider value={value}>{children}</PaperContext.Provider>;
}
