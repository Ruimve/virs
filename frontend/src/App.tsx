import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ErrorBoundary } from './components/ErrorBoundary';
import { FullScreen } from './components/Transition/FullScreen';
import { AssetLoading } from './components/Transition/Icon';
import { AuthProvider } from './context/AuthContext/AuthProvider';

const Loading = lazy(() => import('./pages/Loading'));

const Login = lazy(() => import('./pages/Login'));

const SetupLayout = lazy(() => import('./pages/Setup/Layout'));
const SelectBotType = lazy(() => import('./pages/Setup/SelectBotType'));
const ConfigureLlm = lazy(() => import('./pages/Setup/ConfigureLlm'));
const ConfigureExchange = lazy(() => import('./pages/Setup/ConfigureExchange'));
const ConfigureParams = lazy(() => import('./pages/Setup/ConfigureParams'));
const ConfigureOptimization = lazy(() => import('./pages/Setup/ConfigureOptimization'));
const ReviewLaunch = lazy(() => import('./pages/Setup/ReviewLaunch'));

const TradeLayout = lazy(() => import('./pages/Trade/Layout'));

const AutoBot = lazy(() => import('./pages/Trade/AutoBot'));
const AutoBotMain = lazy(() => import('./pages/Trade/AutoBot/Bot'));
const AutoBotLog = lazy(() => import('./pages/Trade/AutoBot/Log'));
const AutoBotLogDetail = lazy(() => import('./pages/Trade/AutoBot/Log/Detail'));
const AutoBotTrades = lazy(() => import('./pages/Trade/AutoBot/Trades'));
const AutoBotSystem = lazy(() => import('./pages/Trade/AutoBot/System'));

const HealthCheck = lazy(() => import('./pages/Trade/HealthCheck'));

const App = () => {
  return (
    <ErrorBoundary>
      <Suspense fallback={<FullScreen icon={<AssetLoading />} />}>
        <BrowserRouter>
          <Routes>
            <Route
              path="/"
              element={
                <AuthProvider>
                  <Loading />
                </AuthProvider>
              }
            />
            <Route path="/login" element={<Login />} />
            <Route
              path="/setup"
              element={
                <AuthProvider>
                  <SetupLayout />
                </AuthProvider>
              }
            >
              <Route path="/setup/bot-type" element={<SelectBotType />} />
              <Route path="/setup/llm" element={<ConfigureLlm />} />
              <Route path="/setup/exchange" element={<ConfigureExchange />} />
              <Route path="/setup/params" element={<ConfigureParams />} />
              <Route path="/setup/optimization" element={<ConfigureOptimization />} />
              <Route path="/setup/review" element={<ReviewLaunch />} />
            </Route>
            <Route
              path="/trade"
              element={
                <AuthProvider>
                  <TradeLayout />
                </AuthProvider>
              }
            >
              <Route path="/trade/auto/:botId" element={<AutoBot />}>
                <Route path="/trade/auto/:botId/bot" element={<AutoBotMain />} />
                <Route path="/trade/auto/:botId/log" element={<AutoBotLog />} />
                <Route path="/trade/auto/:botId/log/:logId" element={<AutoBotLogDetail />} />
                <Route path="/trade/auto/:botId/trades" element={<AutoBotTrades />} />
                <Route path="/trade/auto/:botId/system" element={<AutoBotSystem />} />
              </Route>
              <Route path="/trade/:botType/:botId/health" element={<HealthCheck />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </BrowserRouter>
      </Suspense>
    </ErrorBoundary>
  );
};

export default App;
