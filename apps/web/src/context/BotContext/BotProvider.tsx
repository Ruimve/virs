import { type ReactNode, use, useMemo } from 'react';
import { Navigate } from 'react-router-dom';
import type { BotDetail } from '@/service';
import { BotContext } from '.';

export const BotProvider = ({
  promiseBot,
  children,
}: {
  promiseBot: Promise<BotDetail | null>;
  children: ReactNode;
}) => {
  const result = use(promiseBot);

  const value = useMemo(() => result, [result]);

  if (!value) return <Navigate to="/setup/bot-type" replace />;

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};
