import { type Component } from 'solid-js'
import { Route, Router, Navigate } from '@solidjs/router'
import { MarketProvider } from './lib/market-context'
import Login from './pages/Login'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import Strategies from './pages/Strategies'
import Market from './pages/Market'
import Backtest from './pages/Backtest'
import Trades from './pages/Trades'
import Credentials from './pages/Credentials'
import Users from './pages/Users'

const App: Component = () => {
  return (
    <MarketProvider>
      <Router root={Layout}>
        <Route path="/login" component={Login} />
        <Route path="/dashboard" component={Dashboard} />
        <Route path="/strategies" component={Strategies} />
        <Route path="/market" component={Market} />
        <Route path="/backtest" component={Backtest} />
        <Route path="/trades" component={Trades} />
        <Route path="/credentials" component={Credentials} />
        <Route path="/users" component={Users} />
        <Route path="/" component={() => <Navigate href="/dashboard" />} />
        <Route path="*" component={() => <Navigate href="/dashboard" />} />
      </Router>
    </MarketProvider>
  )
}

export default App
