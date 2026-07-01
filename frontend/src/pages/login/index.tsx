import { useCallback, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '@/context/AuthContext';
import { Spinner, InfoCircle } from '@/components/Icon';

const Login = () => {
  const navigate = useNavigate();
  const { login } = useAuth();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = useCallback(
    async (e: React.SubmitEvent, uname: string, pwd: string) => {
      e.preventDefault();

      setError('');
      setLoading(true);
      try {
        const result = await login(uname, pwd);
        if (result.success) {
          navigate('/setup/bot-type', { replace: true });
          return;
        }
        setError(result.error || 'Login failed');
      } catch {
        setError('Network error, please try again');
      } finally {
        setLoading(false);
      }
    },
    [navigate, login],
  );

  const errorMessage = useMemo(() => {
    if (error) {
      return (
        <div className="flex items-center gap-2 p-3 bg-danger-bg border border-danger-border rounded-xl text-sm text-danger-text">
          <InfoCircle className="w-4 h-4 shrink-0" strokeWidth={1.5} />
          <span>{error}</span>
        </div>
      );
    }
    return null;
  }, [error]);

  return (
    <div className="min-h-screen bg-base flex items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-accent/3 blur-[120px]" />
      </div>

      <div className="w-full max-w-sm px-6 relative">
        <div className="text-center mb-12">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-accent/20 to-accent-muted/20 border border-accent-muted mb-6">
            <span className="text-2xl font-extralight tracking-[0.3em] text-on-base">V</span>
          </div>
          <h1 className="text-2xl font-extralight tracking-[0.4em] text-on-surface mb-1">VIRS</h1>
          <p className="text-[11px] tracking-[0.25em] text-on-surface-muted">
            QUANTITATIVE TRADING
          </p>
        </div>

        <form onSubmit={(e) => handleSubmit(e, username, password)} className="space-y-5">
          {errorMessage}
          <div>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full px-4 py-3 bg-surface-2 border border-line-strong rounded-xl text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-accent focus:bg-surface-3 transition-all duration-200"
              placeholder="Username"
              autoComplete="username"
              disabled={loading}
            />
          </div>

          <div>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full px-4 py-3 bg-surface-2 border border-line-strong rounded-xl text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-accent focus:bg-surface-3 transition-all duration-200"
              placeholder="Password"
              autoComplete="current-password"
              disabled={loading}
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full py-3 px-4 bg-accent/80 hover:bg-accent-hover text-white text-sm font-medium rounded-xl focus:outline-none focus:ring-2 focus:ring-accent-muted focus:ring-offset-2 focus:ring-offset-base disabled:opacity-40 disabled:cursor-not-allowed transition-all duration-200"
          >
            {loading ? (
              <span className="flex items-center justify-center gap-2">
                <Spinner className="w-4 h-4" />
                Signing in...
              </span>
            ) : (
              'Sign in'
            )}
          </button>
        </form>
      </div>
    </div>
  );
};

export default Login;
