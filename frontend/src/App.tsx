import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Loading from './pages/Loading';
import Login from './pages/Login';
import { ErrorBoundary } from './components/ErrorBoundary';
import FullScreen from './components/Transition/FullScreen';
import { Icon } from './components/Transition/Icon/AssetLoading';
import { AuthProvider, ProtectedRoute } from './context/AuthContext/AuthProvider';

/** 向导 */
const SetupLayout = lazy(() => import('./pages/Setup/Layout'));
const SelectBotType = lazy(() => import('./pages/Setup/SelectBotType'));
const ConfigureLlm = lazy(() => import('./pages/Setup/ConfigureLlm'));
const ConfigureExchange = lazy(() => import('./pages/Setup/ConfigureExchange'));
const ConfigureParams = lazy(() => import('./pages/Setup/ConfigureParams'));
const ReviewLaunch = lazy(() => import('./pages/Setup/ReviewLaunch'));

/** 交易 */
const TradeLayout = lazy(() => import('./pages/Trade/Layout'));
/** 交易 - 自动交易 */
const AutoBot = lazy(() => import('./pages/Trade/AutoBot'));
const AutoBotMain = lazy(() => import('./pages/Trade/AutoBot/Bot'));
const AutoBotLog = lazy(() => import('./pages/Trade/AutoBot/Log'));
const AutoBotLogDetail = lazy(() => import('./pages/Trade/AutoBot/Log/Detail'));
const AutoBotTrades = lazy(() => import('./pages/Trade/AutoBot/Trades'));
const AutoBotSystem = lazy(() => import('./pages/Trade/AutoBot/System'));
/** 交易 - 网格交易 */
const GridBot = lazy(() => import('./pages/Trade/GridBot'));
const GridBotMain = lazy(() => import('./pages/Trade/GridBot/Bot'));
const GridBotLog = lazy(() => import('./pages/Trade/GridBot/Log'));
const GridBotLogDetail = lazy(() => import('./pages/Trade/GridBot/Log/Detail'));
const GridBotTrades = lazy(() => import('./pages/Trade/GridBot/Trades'));
const GridBotLevels = lazy(() => import('./pages/Trade/GridBot/Levels'));
const GridBotSystem = lazy(() => import('./pages/Trade/GridBot/System'));

/** 交易 - 健康检查 */
const HealthCheck = lazy(() => import('./pages/Trade/HealthCheck'));
/** 交易 -系统信息 */

const SuspenseWrap = ({ children }: { children: React.ReactNode }) => {
  return <Suspense fallback={<FullScreen icon={<Icon />} />}>{children}</Suspense>;
};

const App = () => {
  return (
    <ErrorBoundary>
      <BrowserRouter>
        <AuthProvider>
          <SuspenseWrap>
            <Routes>
              <Route path="/" element={<Loading />} />
              <Route path="/login" element={<Login />} />
              <Route
                path="/setup"
                element={
                  <ProtectedRoute>
                    <SetupLayout />
                  </ProtectedRoute>
                }
              >
                <Route path="/setup/bot-type" element={<SelectBotType />} />
                <Route path="/setup/llm" element={<ConfigureLlm />} />
                <Route path="/setup/exchange" element={<ConfigureExchange />} />
                <Route path="/setup/params" element={<ConfigureParams />} />
                <Route path="/setup/review" element={<ReviewLaunch />} />
              </Route>
              <Route
                path="/trade"
                element={
                  <ProtectedRoute>
                    <TradeLayout />
                  </ProtectedRoute>
                }
              >
                <Route path="/trade/auto/:botId" element={<AutoBot />}>
                  <Route path="/trade/auto/:botId/bot" element={<AutoBotMain />} />
                  <Route path="/trade/auto/:botId/log" element={<AutoBotLog />} />
                  <Route path="/trade/auto/:botId/log/:logId" element={<AutoBotLogDetail />} />
                  <Route path="/trade/auto/:botId/trades" element={<AutoBotTrades />} />
                  <Route path="/trade/auto/:botId/system" element={<AutoBotSystem />} />
                </Route>
                <Route path="/trade/grid/:botId" element={<GridBot />}>
                  <Route path="/trade/grid/:botId/bot" element={<GridBotMain />} />
                  <Route path="/trade/grid/:botId/log" element={<GridBotLog />} />
                  <Route path="/trade/grid/:botId/log/:logId" element={<GridBotLogDetail />} />
                  <Route path="/trade/grid/:botId/trades" element={<GridBotTrades />} />
                  <Route path="/trade/grid/:botId/levels" element={<GridBotLevels />} />
                  <Route path="/trade/grid/:botId/system" element={<GridBotSystem />} />
                </Route>
                <Route path="/trade/:botType/:botId/health" element={<HealthCheck />} />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </SuspenseWrap>
        </AuthProvider>
      </BrowserRouter>
    </ErrorBoundary>
  );
};

export default App;
