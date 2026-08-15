import { Suspense, useState } from 'react';
import { Outlet, useParams } from 'react-router-dom';
import { AuthProvider } from '@/context/AuthContext/AuthProvider';
import Fallback from '@/components/Transition/Fallback';
import { getUser } from '@/context/AuthContext/auth';
import { fetchBot } from '@/context/BotContext/bot';
import { BotProvider } from '@/context/BotContext/BotProvider';

const Guard = () => {
  const { botId } = useParams();

  const [promiseUser] = useState(() => getUser());
  const [promiseBot] = useState(() => fetchBot(botId));

  return (
    <Suspense fallback={<Fallback label="正在登录账户" startProgress={45} progress={85} />}>
      <AuthProvider promiseUser={promiseUser}>
        <BotProvider promiseBot={promiseBot}>
          <Outlet />
        </BotProvider>
      </AuthProvider>
    </Suspense>
  );
};

export default Guard;
