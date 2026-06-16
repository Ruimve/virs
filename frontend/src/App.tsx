import { Component as ReactComponent, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { MarketProvider } from './lib/market-context'
import { PaperProvider } from './lib/paper-context'
import Loading from './pages/loading'
import Login from './pages/login'

// Lazy-loaded pages
const ServiceCheck = lazy(() => import('./pages/service-check'))
const SelectBotType = lazy(() => import('./pages/setup/select-bot-type'))
const ConfigureLlm = lazy(() => import('./pages/setup/configure-llm'))
const SelectExchange = lazy(() => import('./pages/setup/select-exchange'))
const ConfigureParams = lazy(() => import('./pages/setup/configure-params'))
const ReviewLaunch = lazy(() => import('./pages/setup/review-launch'))
const HealthCheck = lazy(() => import('./pages/setup/health-check'))
const GridDetailPage = lazy(() => import('./pages/AITrade/Grid'))
const AutoDetailPage = lazy(() => import('./pages/AITrade/Auto'))
const AnalysisLogDetailPage = lazy(() => import('./pages/AITrade/analysis-log-detail'))

function SuspenseWrap({ children }: { children: React.ReactNode }) {
  return <Suspense fallback={<div className="min-h-screen bg-base flex items-center justify-center"><div className="text-on-surface-tertiary text-sm">Loading...</div></div>}>{children}</Suspense>
}

class ErrorBoundary extends ReactComponent<{ children: React.ReactNode }, { hasError: boolean; error: Error | null }> {
  state = { hasError: false, error: null as Error | null }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error }
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-base">
          <div className="text-center p-8">
            <h1 className="text-xl font-semibold text-on-surface mb-2">Error</h1>
            <p className="text-on-surface-tertiary mb-4">{this.state.error?.message}</p>
            <button
              className="px-4 py-2 bg-indigo-500/80 text-white rounded-lg hover:bg-indigo-500 text-sm"
              onClick={() => window.location.reload()}
            >
              Reload
            </button>
          </div>
        </div>
      )
    }
    return this.props.children
  }
}

function App() {
  return (
    <ErrorBoundary>
      <MarketProvider>
        <PaperProvider>
          <BrowserRouter>
            <SuspenseWrap>
              <Routes>
                <Route path="/" element={<Loading />} />
                <Route path="/login" element={<Login />} />
                <Route path="/check" element={<ServiceCheck />} />
                <Route path="/setup/bot-type" element={<SelectBotType />} />
                <Route path="/setup/llm" element={<ConfigureLlm />} />
                <Route path="/setup/exchange" element={<SelectExchange />} />
                <Route path="/setup/params" element={<ConfigureParams />} />
                <Route path="/setup/review" element={<ReviewLaunch />} />
                <Route path="/setup/health" element={<HealthCheck />} />
                <Route path="/trade/grid/:id" element={<GridDetailPage />} />
                <Route path="/trade/grid/:id/:tab" element={<GridDetailPage />} />
                <Route path="/trade/auto/:id" element={<AutoDetailPage />} />
                <Route path="/trade/auto/:id/:tab" element={<AutoDetailPage />} />
                <Route path="/trade/:botType/:botId/log/:logId" element={<AnalysisLogDetailPage />} />
                <Route path="/trade" element={<Navigate to="/" replace />} />
                <Route path="*" element={<Navigate to="/" replace />} />
              </Routes>
            </SuspenseWrap>
          </BrowserRouter>
        </PaperProvider>
      </MarketProvider>
    </ErrorBoundary>
  )
}

export default App
