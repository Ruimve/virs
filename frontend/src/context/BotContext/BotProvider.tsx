import { type ReactNode, use, useMemo } from 'react';
import { BotContext, type BotContextType } from '.';

export const BotProvider = ({
  promiseBot,
  children,
}: {
  promiseBot: Promise<BotContextType>;
  children: ReactNode;
}) => {
  const { bot, strategy } = use(promiseBot);

  const value = useMemo(() => {
    return {
      bot,
      strategy,
    };
  }, [bot, strategy]);

  return <BotContext.Provider value={value}>{children}</BotContext.Provider>;
};
