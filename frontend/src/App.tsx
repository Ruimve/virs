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

const ChatBot = lazy(() => import('./pages/Trade/ChatBot'));
const ChatBotMain = lazy(() => import('./pages/Trade/ChatBot/Bot'));
const ChatBotLog = lazy(() => import('./pages/Trade/ChatBot/Log'));
const ChatBotLogDetail = lazy(() => import('./pages/Trade/ChatBot/Log/Detail'));
const ChatBotTrades = lazy(() => import('./pages/Trade/ChatBot/Trades'));
const ChatBotSystem = lazy(() => import('./pages/Trade/ChatBot/System'));

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
                <Route path="/trade/chat/:botId" element={<ChatBot />}>
                  <Route path="/trade/chat/:botId/bot" element={<ChatBotMain />} />
                  <Route path="/trade/chat/:botId/log" element={<ChatBotLog />} />
                  <Route path="/trade/chat/:botId/log/:logId" element={<ChatBotLogDetail />} />
                  <Route path="/trade/chat/:botId/trades" element={<ChatBotTrades />} />
                  <Route path="/trade/chat/:botId/system" element={<ChatBotSystem />} />
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
