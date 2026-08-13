import { useActionState } from 'react';
import { ShieldCheck, Lock, User } from '@/components/Icon';
import { Button } from '@/components/Button';
import { Input } from '@/components/Input';
import { Logo } from '@/components/Logo';
import { Alert } from '@/components/Alert';
import { findActiveBot, login } from '@/service';
import { useNavigate } from 'react-router-dom';

const Login = () => {
  const navigate = useNavigate();
  const [error, submitAction, isPending] = useActionState<string | undefined | null, FormData>(
    async (_, formData) => {
      const username = formData.get('username') as string;
      const password = formData.get('password') as string;
      try {
        const result = await login(username, password);
        if (result?.success) {
          const bot = await findActiveBot();
          if (bot) {
            navigate(`/trade/bot/${bot.id}/bot`, { replace: true });
          } else {
            navigate('/setup/bot-type', { replace: true });
          }
        } else {
          return result?.message || 'Login failed';
        }
      } catch (e) {
        return (e as Error)?.message || 'Login failed';
      }
    },
    null,
  );

  return (
    <div className="min-h-dvh bg-base flex justify-center items-center terminal-grid-bg">
      <div className="w-full max-w-sm mx-6 z-10 bg-surface-1 border border-line-default rounded-xl p-8 shadow-sm">
        <div className="flex justify-center mb-6">
          <div className="p-[1.5px] rounded-18 bg-linear-to-br from-accent-muted to-ai-muted">
            <div className="bg-surface-1 rounded-[16.5px] p-3">
              <Logo size={48} />
            </div>
          </div>
        </div>

        <h1 className="text-center font-display text-2xl font-extralight tracking-hero-lg text-on-base mb-1 pl-[0.3em]">
          VIRS
        </h1>
        <p className="text-center text-2xs font-medium tracking-hero text-on-surface-muted uppercase mb-7 pl-[0.24em]">
          Quantitative Trading System
        </p>

        <form action={submitAction} className="space-y-4">
          <Input
            name="username"
            type="text"
            placeholder="Username"
            autoComplete="username"
            prefix={<User width={18} height={18} strokeWidth={1.5} />}
          />
          <Input
            name="password"
            type="password"
            placeholder="Password"
            autoComplete="password"
            prefix={<Lock width={18} height={18} strokeWidth={1.5} />}
          />
          {error && <Alert type="danger" title={error} className="animate-error-enter" />}
          <Button type="submit" variant="primary" loading={isPending} className="sm:w-full">
            {isPending ? 'Signing in...' : 'Sign in'}
          </Button>
        </form>
        <div className="mt-7 flex items-center gap-3">
          <div className="flex-1 h-px bg-line-default" />
          <span className="flex items-center gap-1.5 text-2xs font-medium tracking-caption text-on-surface-muted uppercase whitespace-nowrap">
            <ShieldCheck className="w-3 h-3" strokeWidth={1.5} />
            Secured by VIRS Engine
          </span>
          <div className="flex-1 h-px bg-line-default" />
        </div>
      </div>
    </div>
  );
};

export default Login;
