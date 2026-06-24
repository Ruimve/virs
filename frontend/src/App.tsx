import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Loading from './pages/loading';
import Login from './pages/login';
import { ErrorBoundary } from './components/ErrorBoundary';

/** 向导 */
const SetupLayout = lazy(() => import('./pages/setup/Layout'));
const SelectBotType = lazy(() => import('./pages/setup/SelectBotType'));
const ConfigureLlm = lazy(() => import('./pages/setup/ConfigureLlm'));
const SelectExchange = lazy(() => import('./pages/setup/SelectExchange'));
const ConfigureParams = lazy(() => import('./pages/setup/ConfigureParams'));
const ReviewLaunch = lazy(() => import('./pages/setup/ReviewLaunch'));

/** 交易 */
const AITradeLayout = lazy(() => import('./pages/AITrade/Layout'));
/** 交易 - 自动交易 */
const AutoBot = lazy(() => import('./pages/AITrade/AutoBot'));
const AutoBotMain = lazy(() => import('./pages/AITrade/AutoBot/Bot'));
const AutoBotAILog = lazy(() => import('./pages/AITrade/AutoBot/AILog'));
const AutoBotAILogDetail = lazy(() => import('./pages/AITrade/AutoBot/AILog/Detail'));
const AutoBotTrades = lazy(() => import('./pages/AITrade/AutoBot/Trades'));
/** 交易 - 网格交易 */
const GridBot = lazy(() => import('./pages/AITrade/GridBot'));
const GridBotMain = lazy(() => import('./pages/AITrade/GridBot/Bot'));
const GridBotAILog = lazy(() => import('./pages/AITrade/GridBot/AILog'));
const GridBotAILogDetail = lazy(() => import('./pages/AITrade/GridBot/AILog/Detail'));
const GridBotTrades = lazy(() => import('./pages/AITrade/GridBot/Trades'));
const GridBotLevels = lazy(() => import('./pages/AITrade/GridBot/Levels'));

/** 交易 - 健康检查 */
const HealthCheck = lazy(() => import('./pages/AITrade/HealthCheck'));
/** 交易 -系统信息 */
const System = lazy(() => import('./pages/AITrade/System'));

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
  );
}

function App() {
  return (
    <ErrorBoundary>
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
              <Route path="/trade/auto/:botId" element={<AutoBot />}>
                <Route path="/trade/auto/:botId/bot" element={<AutoBotMain />} />
                <Route path="/trade/auto/:botId/log" element={<AutoBotAILog />} />
                <Route path="/trade/auto/:botId/log/:logId" element={<AutoBotAILogDetail />} />
                <Route path="/trade/auto/:botId/trades" element={<AutoBotTrades />} />
              </Route>
              <Route path="/trade/grid/:botId" element={<GridBot />}>
                <Route path="/trade/grid/:botId/bot" element={<GridBotMain />} />
                <Route path="/trade/grid/:botId/log" element={<GridBotAILog />} />
                <Route path="/trade/grid/:botId/log/:logId" element={<GridBotAILogDetail />} />
                <Route path="/trade/grid/:botId/trades" element={<GridBotTrades />} />
                <Route path="/trade/grid/:botId/levels" element={<GridBotLevels />} />
              </Route>
              <Route path="/trade/health/:botType/:botId" element={<HealthCheck />} />
              <Route path="/trade/system" element={<System />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </SuspenseWrap>
      </BrowserRouter>
    </ErrorBoundary>
  );
}

export default App;
