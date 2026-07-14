import { Component as ReactComponent, type ReactNode } from 'react';
import { Button } from '@/components/Button';

export interface ErrorBoundaryProps {
  children: React.ReactNode;

  fallback?: (error: Error, reset: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends ReactComponent<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {

    console.error('[ErrorBoundary]', error, errorInfo.componentStack);
  }

  reset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError && this.state.error) {
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.reset);
      }

      return (
        <div className="min-h-screen flex items-center justify-center bg-base">
          <div className="text-center p-8">
            <h1 className="text-xl font-semibold text-on-surface mb-2">Error</h1>
            <p className="text-on-surface-tertiary mb-4">
              {import.meta.env.PROD
                ? 'Something went wrong. Please reload the page.'
                : this.state.error.message}
            </p>
            <Button
              variant="primary"
              size="small"
              responsive={false}
              onClick={() => window.location.reload()}
            >
              Reload
            </Button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
