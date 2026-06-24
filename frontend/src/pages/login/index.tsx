import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { login } from '../../service';

function Login() {
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleChangeUsername = (e: React.ChangeEvent<HTMLInputElement>) => {
    setUsername(e.target.value);
  };

  const handleChangePassword = (e: React.ChangeEvent<HTMLInputElement>) => {
    setPassword(e.target.value);
  };

  const handleSubmit = async (e: React.SubmitEvent) => {
    e.preventDefault();
    setError('');

    if (!username.trim() || !password.trim()) {
      setError('Please enter username and password');
      return;
    }

    setLoading(true);
    try {
      const result = await login(username.trim(), password.trim());
      if (result.success) {
        navigate('/setup/bot-type', { replace: true });
      } else {
        setError(result.error || 'Login failed');
      }
    } catch {
      setError('Network error, please try again');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-base flex items-center justify-center relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      <div className="w-full max-w-sm px-6 relative">
        <div className="text-center mb-12">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-violet-500/20 border border-indigo-500/20 mb-6">
            <span className="text-2xl font-extralight tracking-[0.3em] text-on-base">V</span>
          </div>
          <h1 className="text-2xl font-extralight tracking-[0.4em] text-on-surface mb-1">VIRS</h1>
          <p className="text-[11px] tracking-[0.25em] text-on-surface-muted">
            QUANTITATIVE TRADING
          </p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-5">
          {error && (
            <div className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">
              <svg
                className="w-4 h-4 shrink-0"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth="1.5"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span>{error}</span>
            </div>
          )}

          <div>
            <input
              type="text"
              value={username}
              onChange={handleChangeUsername}
              className="w-full px-4 py-3 bg-surface-2 border border-line-strong rounded-xl text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 focus:bg-surface-3 transition-all duration-200"
              placeholder="Username"
              autoComplete="username"
              disabled={loading}
            />
          </div>

          <div>
            <input
              type="password"
              value={password}
              onChange={handleChangePassword}
              className="w-full px-4 py-3 bg-surface-2 border border-line-strong rounded-xl text-sm text-on-base placeholder-placeholder focus:outline-none focus:border-indigo-500/40 focus:bg-surface-3 transition-all duration-200"
              placeholder="Password"
              autoComplete="current-password"
              disabled={loading}
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full py-3 px-4 bg-indigo-500/80 hover:bg-indigo-500 text-white text-sm font-medium rounded-xl focus:outline-none focus:ring-2 focus:ring-indigo-500/30 focus:ring-offset-2 focus:ring-offset-base disabled:opacity-40 disabled:cursor-not-allowed transition-all duration-200"
          >
            {loading ? (
              <span className="flex items-center justify-center gap-2">
                <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                  <circle
                    className="opacity-25"
                    cx="12"
                    cy="12"
                    r="10"
                    stroke="currentColor"
                    strokeWidth="4"
                  />
                  <path
                    className="opacity-75"
                    fill="currentColor"
                    d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                  />
                </svg>
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
}

export default Login;
