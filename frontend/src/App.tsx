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
const ReviewLaunch = lazy(() => import('./pages/Setup/ReviewLaunch'));


const TradeLayout = lazy(() => import('./pages/Trade/Layout'));

const AutoBot = lazy(() => import('./pages/Trade/AutoBot'));
const AutoBotMain = lazy(() => import('./pages/Trade/AutoBot/Bot'));
const AutoBotLog = lazy(() => import('./pages/Trade/AutoBot/Log'));
const AutoBotLogDetail = lazy(() => import('./pages/Trade/AutoBot/Log/Detail'));
const AutoBotTrades = lazy(() => import('./pages/Trade/AutoBot/Trades'));
const AutoBotSystem = lazy(() => import('./pages/Trade/AutoBot/System'));

const GridBot = lazy(() => import('./pages/Trade/GridBot'));
const GridBotMain = lazy(() => import('./pages/Trade/GridBot/Bot'));
const GridBotLog = lazy(() => import('./pages/Trade/GridBot/Log'));
const GridBotLogDetail = lazy(() => import('./pages/Trade/GridBot/Log/Detail'));
const GridBotTrades = lazy(() => import('./pages/Trade/GridBot/Trades'));
const GridBotLevels = lazy(() => import('./pages/Trade/GridBot/Levels'));
const GridBotSystem = lazy(() => import('./pages/Trade/GridBot/System'));


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
        </BrowserRouter>
      </Suspense>
    </ErrorBoundary>
  );
};

export default App;
