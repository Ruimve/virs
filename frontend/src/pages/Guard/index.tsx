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
    <Suspense fallback={<Fallback label="正在检查账户..." startProgress={35} progress={60} />}>
      <AuthProvider promiseUser={promiseUser}>
        <Suspense fallback={<Fallback label="正在读取AI..." startProgress={65} progress={90} />}>
          <BotProvider promiseBot={promiseBot}>
            <Suspense
              fallback={<Fallback label="正在加载页面..." startProgress={95} progress={100} />}
            >
              <Outlet />
            </Suspense>
          </BotProvider>
        </Suspense>
      </AuthProvider>
    </Suspense>
  );
};

export default Guard;
