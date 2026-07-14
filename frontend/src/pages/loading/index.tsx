import { useState, useEffect, useMemo, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { findActiveBot } from '@/service';
import { AssetLoading } from '@/components/Transition/Icon';

/**
 * Loading —— 应用启动加载页。
 *
 * 设计原则：
 *  1. 真实状态驱动：阶段切换由实际异步操作触发，无 mock 进度
 *  2. 视觉连贯：复用 AssetLoading 图标，与全局过渡页统一语言
 *  3. 极简留白：单一 accent 主色，黑白主题自适应
 *
 * 认证会话由 AuthProvider 统一恢复，本页只读取 useAuth() 状态做路由决策。
 */

type Stage = 'auth' | 'session' | 'routing';

const STAGE_LABEL: Record<Stage, string> = {
  auth: 'Verifying identity',
  session: 'Restoring session',
  routing: 'Routing to workspace',
};

const STAGE_ORDER: Stage[] = ['auth', 'session', 'routing'];

const Loading = () => {
  const navigate = useNavigate();
  const [stage, setStage] = useState<Stage>('auth');

  const startStage = useCallback(async () => {
    try {
      setStage('session');
      const bot = await findActiveBot();
      setStage('routing');
      if (bot) {
        navigate(`/trade/${bot.bot_type}/${bot.id}/bot`, { replace: true });
      } else {
        navigate('/setup/bot-type', { replace: true });
      }
    } catch {
      navigate('/setup/bot-type', { replace: true });
    }
  }, [navigate]);

  // 会话恢复就绪后，依据登录状态与活跃 bot 做路由决策
  useEffect(() => {
    startStage();
  }, [startStage, navigate]);

  const stageOrders = useMemo(() => {
    const currentIdx = STAGE_ORDER.indexOf(stage);
    return STAGE_ORDER.map((s, i) => (
      <span
        key={s}
        className={`loading-dot h-1 rounded-full transition-all duration-500 ${
          i < currentIdx
            ? 'w-4 bg-accent'
            : i === currentIdx
              ? 'w-6 bg-accent'
              : 'w-1 bg-line-default'
        }`}
        style={{ transitionDelay: `${i * 60}ms` }}
      />
    ));
  }, [stage]);

  return (
    <div className="loading-page min-h-dvh bg-base flex flex-col items-center justify-center relative overflow-hidden">
      {/* 氛围层：accent glow + 极淡量化网格 */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[640px] h-[640px] rounded-full bg-accent/5 blur-[140px]" />
        <div className="loading-grid absolute inset-0 opacity-[0.04]" />
      </div>

      {/* 主体 */}
      <div className="relative flex flex-col items-center">
        {/* 加载图标 */}
        <div className="loading-icon-wrap mb-8">
          <AssetLoading size={112} />
        </div>

        {/* 品牌字标 */}
        <h1 className="loading-brand text-[22px] font-extralight tracking-[0.5em] text-on-base mb-2 select-none pl-[0.5em]">
          VIRS
        </h1>
        <p className="text-2xs tracking-[0.32em] text-on-surface-muted mb-8 select-none pl-[0.32em] uppercase">
          Quantitative Trading
        </p>

        {/* 阶段指示器 */}
        <div className="flex flex-col items-center gap-5">
          {/* 三点阶段指示 */}
          <div className="flex items-center gap-2.5">{stageOrders}</div>

          {/* 当前状态文字（淡入淡出） */}
          <div className="relative overflow-hidden">
            <span
              key={stage}
              className="loading-stage-text text-caption tracking-[0.2em] text-on-surface-tertiary uppercase font-mono"
            >
              {STAGE_LABEL[stage]}
            </span>
          </div>
        </div>
      </div>

      <style>{`
        .loading-page {
          font-family: var(--font-sans, ui-sans-serif, system-ui, -apple-system, sans-serif);
        }

        .loading-grid {
          background-image:
            linear-gradient(to right, var(--color-line-default) 1px, transparent 1px),
            linear-gradient(to bottom, var(--color-line-default) 1px, transparent 1px);
          background-size: 48px 48px;
          mask-image: radial-gradient(circle at center, black 30%, transparent 75%);
          -webkit-mask-image: radial-gradient(circle at center, black 30%, transparent 75%);
        }

        .loading-icon-wrap {
          animation: loading-fade-in 0.6s ease-out both;
        }

        .loading-brand {
          animation: loading-fade-in 0.8s ease-out 0.1s both;
        }

        .loading-stage-text {
          animation: loading-stage-fade 0.4s ease-out both;
        }

        @keyframes loading-fade-in {
          from { opacity: 0; transform: translateY(8px); }
          to { opacity: 1; transform: translateY(0); }
        }

        @keyframes loading-stage-fade {
          from { opacity: 0; transform: translateY(4px); }
          to { opacity: 1; transform: translateY(0); }
        }

        @media (prefers-reduced-motion: reduce) {
          .loading-icon-wrap,
          .loading-brand,
          .loading-stage-text {
            animation: none;
          }
        }
      `}</style>
    </div>
  );
};

export default Loading;
