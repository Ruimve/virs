import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ErrorBoundary } from './components/ErrorBoundary';
import Fallback from './components/Transition/Fallback';
import Guard from './pages/Guard';

const Login = lazy(() => import('./pages/Login'));
const SetupLayout = lazy(() => import('./pages/Setup/Layout'));
const SelectBotType = lazy(() => import('./pages/Setup/SelectBotType'));
const ConfigureLlm = lazy(() => import('./pages/Setup/ConfigureLlm'));
const ConfigureExchange = lazy(() => import('./pages/Setup/ConfigureExchange'));
const ConfigureParams = lazy(() => import('./pages/Setup/ConfigureParams'));
const ConfigureOptimization = lazy(() => import('./pages/Setup/ConfigureOptimization'));
const ReviewLaunch = lazy(() => import('./pages/Setup/ReviewLaunch'));

const TradeLayout = lazy(() => import('./pages/Trade/Layout'));

const Bot = lazy(() => import('./pages/Trade/Bot'));
const BotMain = lazy(() => import('./pages/Trade/Bot/Bot'));
const BotLog = lazy(() => import('./pages/Trade/Bot/Log'));
const BotLogDetail = lazy(() => import('./pages/Trade/Bot/Log/Detail'));
const BotTrades = lazy(() => import('./pages/Trade/Bot/Trades'));
const BotSystem = lazy(() => import('./pages/Trade/Bot/System'));

const App = () => {
  return (
    <ErrorBoundary>
      <Suspense fallback={<Fallback label="正在下载资源..." progress={30} />}>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<Login />} />
            <Route path="/" element={<Guard />}>
              <Route index element={<Navigate to="/setup/bot-type" replace />} />
              <Route path="/setup" element={<SetupLayout />}>
                <Route path="/setup/bot-type" element={<SelectBotType />} />
                <Route path="/setup/llm" element={<ConfigureLlm />} />
                <Route path="/setup/exchange" element={<ConfigureExchange />} />
                <Route path="/setup/params" element={<ConfigureParams />} />
                <Route path="/setup/optimization" element={<ConfigureOptimization />} />
                <Route path="/setup/review" element={<ReviewLaunch />} />
              </Route>
              <Route path="/trade" element={<TradeLayout />}>
                <Route path="/trade/bot/:botId" element={<Bot />}>
                  <Route path="/trade/bot/:botId/bot" element={<BotMain />} />
                  <Route path="/trade/bot/:botId/log" element={<BotLog />} />
                  <Route path="/trade/bot/:botId/log/:logId" element={<BotLogDetail />} />
                  <Route path="/trade/bot/:botId/trades" element={<BotTrades />} />
                  <Route path="/trade/bot/:botId/system" element={<BotSystem />} />
                </Route>
              </Route>
            </Route>
          </Routes>
        </BrowserRouter>
      </Suspense>
    </ErrorBoundary>
  );
};

export default App;
