import { memo, useCallback, useEffect, useState } from 'react';
import { FormField } from '../../components';
import { fetchPositionMode } from '@/service';

interface PositionModeProps {
  onCheck: (success: boolean) => void;
}

export const PositionMode = memo(({ onCheck }: PositionModeProps) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const getPositionMode = useCallback(async () => {
    try {
      setLoading(true);
      const result = await fetchPositionMode();
      if (!result.success || !result.data) {
        setError('Failed to fetch position mode');
        return onCheck(false);
      }
      const { supported, mode } = result.data;
      if (!supported) {
        setError('当前不支持持仓模式查询，请确认网络状态。');
        return onCheck(false);
      }

      if (mode !== 'hedge') {
        setError(
          '当前为单向持仓模式。请在 Binance APP > 合约 > 设置 > 持仓模式 中切换到双向持仓后重新验证。',
        );
        return onCheck(false);
      }

      setError(null);
      return onCheck(true);
    } catch {
      setError('Network error');
      onCheck(false);
    } finally {
      setLoading(false);
    }
  }, [onCheck]);
  useEffect(() => {
    getPositionMode();
  }, [getPositionMode]);

  const renderContent = useCallback(() => {
    if (loading) {
      return <p className="text-xs text-on-surface-tertiary">Checking position mode...</p>;
    }

    if (error) {
      return <p className="text-xs text-danger-text">{error}</p>;
    }

    return <p className="text-xs text-success-text">Hedge Mode (双向持仓) ✓</p>;
  }, [loading, error]);
  return (
    <FormField label="Position Mode" noBorder>
      {renderContent()}
    </FormField>
  );
});
