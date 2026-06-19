import { lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { PaperProvider } from './lib/paper-context'
import Loading from './pages/loading'
import Login from './pages/login'
import { ErrorBoundary } from './components/ErrorBoundary'

// Lazy-loaded pages
const SetupLayout = lazy(() => import('./pages/setup/Layout'))
const SelectBotType = lazy(() => import('./pages/setup/SelectBotType'))
const ConfigureLlm = lazy(() => import('./pages/setup/ConfigureLlm'))
const SelectExchange = lazy(() => import('./pages/setup/SelectExchange'))
const ConfigureParams = lazy(() => import('./pages/setup/ConfigureParams'))
const ReviewLaunch = lazy(() => import('./pages/setup/ReviewLaunch'))
const AITradeLayout = lazy(() => import('./pages/AITrade/Layout'))
const HealthCheckPage = lazy(() => import('./pages/AITrade/HealthCheck'))
const GridDetailPage = lazy(() => import('./pages/AITrade/Grid'))
const AutoDetailPage = lazy(() => import('./pages/AITrade/Auto'))
const AnalysisLogDetailPage = lazy(() => import('./pages/AITrade/LlmLog'))

function SuspenseWrap({ children }: { children: React.ReactNode }) {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen bg-base flex items-center justify-center">
          <div className="text-on-surface-tertiary text-sm">Loading...</div>
        </div>
      }
    >
      {children}
    </Suspense>
  )
}

function App() {
  return (
    <ErrorBoundary>
      <PaperProvider>
        <BrowserRouter>
          <SuspenseWrap>
            <Routes>
              <Route path="/" element={<Loading />} />
              <Route path="/login" element={<Login />} />
              <Route path="/setup" element={<SetupLayout />}>
                <Route path="/setup/bot-type" element={<SelectBotType />} />
                <Route path="/setup/llm" element={<ConfigureLlm />} />
                <Route path="/setup/exchange" element={<SelectExchange />} />
                <Route path="/setup/params" element={<ConfigureParams />} />
                <Route path="/setup/review" element={<ReviewLaunch />} />
              </Route>
              <Route path="/trade" element={<AITradeLayout />}>
                <Route path="/trade/health/:botType/:botId" element={<HealthCheckPage />} />
                <Route path="/trade/grid/:botId" element={<GridDetailPage />} />
                <Route path="/trade/grid/:botId/:tab" element={<GridDetailPage />} />
                <Route path="/trade/auto/:botId" element={<AutoDetailPage />} />
                <Route path="/trade/auto/:botId/:tab" element={<AutoDetailPage />} />
                <Route
                  path="/trade/:botType/:botId/log/:logId"
                  element={<AnalysisLogDetailPage />}
                />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </SuspenseWrap>
        </BrowserRouter>
      </PaperProvider>
    </ErrorBoundary>
  )
}

export default App
