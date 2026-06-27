import { Icon as LoadAssets, type IconName as LoadAssetsIconName } from './LoadAssets';

export type TransitionIcon = LoadAssetsIconName;

export const iconMap: Record<TransitionIcon, React.FC> = {
  loadingAssets: LoadAssets,
};
