import { type Component, ErrorBoundary, lazy } from 'solid-js'
import { Route, Router, Navigate } from '@solidjs/router'
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
const GridDetailPage = lazy(() => import('./pages/ai-trade/grid-detail'))
const AutoDetailPage = lazy(() => import('./pages/ai-trade/auto-detail'))

const App: Component = () => {
  return (
    <ErrorBoundary fallback={(err) => (
      <div class="min-h-screen flex items-center justify-center bg-[#0a0a0f]">
        <div class="text-center p-8">
          <h1 class="text-xl font-semibold text-white/80 mb-2">Error</h1>
          <p class="text-white/40 mb-4">{err.message}</p>
          <button
            class="px-4 py-2 bg-indigo-500/80 text-white rounded-lg hover:bg-indigo-500 text-sm"
            onClick={() => window.location.reload()}
          >
            Reload
          </button>
        </div>
      </div>
    )}>
      <MarketProvider>
        <PaperProvider>
          <Router>
            <Route path="/" component={Loading} />
            <Route path="/login" component={Login} />
            <Route path="/check" component={ServiceCheck} />
            <Route path="/setup/bot-type" component={SelectBotType} />
            <Route path="/setup/llm" component={ConfigureLlm} />
            <Route path="/setup/exchange" component={SelectExchange} />
            <Route path="/setup/params" component={ConfigureParams} />
            <Route path="/setup/review" component={ReviewLaunch} />
            <Route path="/setup/health" component={HealthCheck} />
            <Route path="/trade/grid/:id" component={GridDetailPage} />
            <Route path="/trade/auto/:id" component={AutoDetailPage} />
            <Route path="/trade" component={() => <Navigate href="/" />} />
            <Route path="*" component={() => <Navigate href="/" />} />
          </Router>
        </PaperProvider>
      </MarketProvider>
    </ErrorBoundary>
  )
}

export default App
