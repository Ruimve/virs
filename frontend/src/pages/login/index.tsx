import { useCallback, useMemo, useState } from 'react';
import { login } from '@/context/AuthContext/auth';
import { InfoCircle } from '@/components/Icon';
import { Input } from '@/components/Input';
import Logo from '@/components/Logo';

const Login = () => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = useCallback(async (e: React.SubmitEvent, uname: string, pwd: string) => {
    e.preventDefault();

    setError('');
    setLoading(true);
    try {
      await login(uname, pwd);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  const errorMessage = useMemo(() => {
    if (error) {
      return (
        <div className="animate-error-enter flex items-center gap-2.5 p-3.5 bg-danger-bg border border-danger-border rounded-xl text-sm text-danger-text">
          <InfoCircle className="w-4 h-4 shrink-0" strokeWidth={1.5} />
          <span>{error}</span>
        </div>
      );
    }
    return null;
  }, [error]);

  return (
    <div className="min-h-screen bg-base flex items-center justify-center relative overflow-hidden terminal-grid-bg">
      {/* Ambient gradient orbs */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[700px] h-[700px] rounded-full bg-accent/[0.03] blur-[160px]" />
        <div className="absolute top-1/4 right-1/4 w-[300px] h-[300px] rounded-full bg-accent/[0.02] blur-[120px]" />
        <div className="absolute bottom-1/4 left-1/4 w-[250px] h-[250px] rounded-full bg-info/[0.02] blur-[100px]" />
      </div>

      <div className="w-full max-w-sm px-6 relative z-10">
        {/* Logo area */}
        <div className="text-center mb-14">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-gradient-to-br from-accent/15 to-accent-muted/10 border border-accent-muted/30 mb-7 backdrop-blur-sm">
            <span className="text-3xl font-extralight tracking-[0.3em] text-on-base">V</span>
          </div>
          <Logo className="block text-3xl mb-2" />
          <p className="text-[11px] tracking-[0.25em] text-on-surface-muted font-medium uppercase">
            Quantitative Trading System
          </p>
        </div>

        {/* Login form */}
        <form onSubmit={(e) => handleSubmit(e, username, password)} className="space-y-5">
          <Input
            type="text"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="Username"
            autoComplete="username"
            disabled={loading}
          />
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Password"
            autoComplete="current-password"
            disabled={loading}
          />
          {errorMessage}
          <button
            type="submit"
            disabled={loading}
            className="w-full py-3.5 px-4 bg-accent hover:bg-accent-hover text-white text-sm font-medium rounded-xl focus:outline-none focus:ring-2 focus:ring-accent-muted focus:ring-offset-2 focus:ring-offset-base disabled:opacity-40 disabled:cursor-not-allowed transition-all duration-300 shadow-lg shadow-accent/10 hover:shadow-accent/20 hover:shadow-xl"
          >
            {loading ? 'Signing in...' : 'Sign in'}
          </button>
        </form>

        {/* Footer */}
        <div className="mt-12 text-center">
          <p className="text-[10px] tracking-[0.2em] text-on-surface-faint uppercase">
            Quantitative Trading System
          </p>
        </div>
      </div>
    </div>
  );
};

export default Login;
