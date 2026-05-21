import { type Component, ErrorBoundary } from 'solid-js'
import { Route, Router, Navigate } from '@solidjs/router'
import { MarketProvider } from './lib/market-context'
import Login from './pages/Login'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import Market from './pages/Market'
import GridPage from './pages/ai-trade/GridPage'
import GridDetailPage from './pages/ai-trade/GridDetailPage'
import AutoPage from './pages/ai-trade/AutoPage'
import Trades from './pages/Trades'
import Credentials from './pages/Credentials'
import AiCredentials from './pages/AiCredentials'
import Users from './pages/Users'

const App: Component = () => {
  return (
    <ErrorBoundary fallback={(err) => (
      <div class="min-h-screen flex items-center justify-center bg-gray-50">
        <div class="text-center p-8">
          <div class="text-6xl mb-4">⚠️</div>
          <h1 class="text-xl font-semibold text-gray-800 mb-2">页面加载出错</h1>
          <p class="text-gray-500 mb-4">{err.message}</p>
          <button
            class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600"
            onClick={() => window.location.reload()}
          >
            刷新页面
          </button>
        </div>
      </div>
    )}>
      <MarketProvider>
        <Router root={Layout}>
          <Route path="/login" component={Login} />
          <Route path="/dashboard" component={Dashboard} />
          <Route path="/market" component={Market} />
          <Route path="/grid" component={() => <Navigate href="/ai-trade/grid" />} />
          <Route path="/ai-trade/grid" component={GridPage} />
          <Route path="/ai-trade/grid/:id" component={GridDetailPage} />
          <Route path="/ai-trade/auto" component={AutoPage} />
          <Route path="/trades" component={Trades} />
          <Route path="/credentials" component={Credentials} />
        <Route path="/ai-credentials" component={AiCredentials} />
        <Route path="/users" component={Users} />
          <Route path="/" component={() => <Navigate href="/dashboard" />} />
          <Route path="*" component={() => <Navigate href="/dashboard" />} />
        </Router>
      </MarketProvider>
    </ErrorBoundary>
  )
}

export default App
