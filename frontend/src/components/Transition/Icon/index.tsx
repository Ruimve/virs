/**
 * Transition 图标族 —— 统一注册与导出。
 *
 * 设计语言：所有图标共享「外圈旋转弧 + 静态淡轨道 + 中央特征标 +
 * 单一 indigo 主色」的视觉语言，节奏异步，自动适配亮/暗主题。
 *
 * 图标清单：
 *  - loadingAssets  : V 字标 + 内圈点状环（全局 / 资产加载）
 *  - botLoading     : Bot 头部 + 天线脉冲（Bot 详情加载）
 *  - aiThinking     : 神经网络节点拓扑（AI 决策 / 分析加载）
 *  - tradeLoading   : 蜡烛 K 线 + 中央呼吸（交易记录加载）
 *  - llmLoading     : 芯片核心 + 4 对角放射（LLM 验证 / 推理加载）
 */
import { Icon as AssetLoading, type IconName as LoadAssetsIconName } from './AssetLoading';
import { Icon as BotLoading, type IconName as BotLoadingIconName } from './BotLoading';
import { Icon as AiThinking, type IconName as AiThinkingIconName } from './AiThinking';
import { Icon as TradeLoading, type IconName as TradeLoadingIconName } from './TradeLoading';
import { Icon as LlmLoading, type IconName as LlmLoadingIconName } from './LlmLoading';

export type TransitionIcon =
  | LoadAssetsIconName
  | BotLoadingIconName
  | AiThinkingIconName
  | TradeLoadingIconName
  | LlmLoadingIconName;

interface IconProps {
  size?: number;
}

export const iconMap: Record<TransitionIcon, React.FC<IconProps>> = {
  AssetLoading: AssetLoading,
  botLoading: BotLoading,
  aiThinking: AiThinking,
  tradeLoading: TradeLoading,
  llmLoading: LlmLoading,
};
