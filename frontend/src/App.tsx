import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ErrorBoundary } from './components/ErrorBoundary';
import Fallback from './components/Transition/Fallback';

const Login = lazy(() => import('./pages/Login'));

const Guard = lazy(() => import('./pages/Guard'));
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

const App = () => {
  return (
    <ErrorBoundary>
      <Suspense fallback={<Fallback />}>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<Login />} />
            <Route path="/" element={<Guard />}>
              <Route path="/setup" element={<SetupLayout />}>
                <Route path="/setup/bot-type" element={<SelectBotType />} />
                <Route path="/setup/llm" element={<ConfigureLlm />} />
                <Route path="/setup/exchange" element={<ConfigureExchange />} />
                <Route path="/setup/params" element={<ConfigureParams />} />
                <Route path="/setup/optimization" element={<ConfigureOptimization />} />
                <Route path="/setup/review" element={<ReviewLaunch />} />
              </Route>
              <Route path="/trade" element={<TradeLayout />}>
                <Route path="/trade/auto/:botId" element={<AutoBot />}>
                  <Route path="/trade/auto/:botId/bot" element={<AutoBotMain />} />
                  <Route path="/trade/auto/:botId/log" element={<AutoBotLog />} />
                  <Route path="/trade/auto/:botId/log/:logId" element={<AutoBotLogDetail />} />
                  <Route path="/trade/auto/:botId/trades" element={<AutoBotTrades />} />
                  <Route path="/trade/auto/:botId/system" element={<AutoBotSystem />} />
                </Route>
              </Route>
              <Route path="*" element={<Navigate to="/setup/bot-type" replace />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </Suspense>
    </ErrorBoundary>
  );
};

export default App;
