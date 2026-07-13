import { Component as ReactComponent } from 'react';
import { Button } from '@/components/Button';

export class ErrorBoundary extends ReactComponent<
  { children: React.ReactNode },
  { hasError: boolean; error: Error | null }
> {
  state = { hasError: false, error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-base">
          <div className="text-center p-8">
            <h1 className="text-xl font-semibold text-on-surface mb-2">Error</h1>
            <p className="text-on-surface-tertiary mb-4">{this.state.error?.message}</p>
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
