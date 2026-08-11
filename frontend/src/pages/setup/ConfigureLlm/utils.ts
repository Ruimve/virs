import {
  fetchAiBalance,
  fetchAiModels,
  saveAiCredential,
  testAiCredential,
  type DeepSeekModel,
} from '@/service';

interface CheckApiKeyResult {
  controllers: AbortController[];
  check: () => Promise<{
    success: boolean;
    message: string;
    models: DeepSeekModel[];
  } | null>;
}

export const checkApiKey = (apiKey: string): CheckApiKeyResult => {
  const saveController = new AbortController();
  const modelsController = new AbortController();
  const testController = new AbortController();
  const balanceController = new AbortController();

  return {
    controllers: [saveController, modelsController, testController, balanceController],
    check: async () => {
      try {
        // 保存凭证
        const saveResult = await saveAiCredential(
          {
            provider: 'deepseek',
            api_key: apiKey,
            is_default: true,
          },
          { signal: saveController.signal },
        );

        if (saveController.signal.aborted) return null;

        if (!saveResult.success) {
          return {
            success: false,
            message: 'Failed to save API key',
            models: [],
          };
        }

        // 测试凭证
        const testResult = await testAiCredential({ signal: testController.signal });
        if (testController.signal.aborted) return null;

        if (!testResult.success || !testResult.data?.connected) {
          return {
            success: false,
            message: 'Connection failed',
            models: [],
          };
        }

        // 获取余额信息
        const balResult = await fetchAiBalance({ signal: balanceController.signal });
        if (balanceController.signal.aborted) return null;

        if (!balResult.success) {
          return {
            success: false,
            message: 'Failed to fetch balance',
            models: [],
          };
        }

        // 获取 model 列表
        const modelsResult = await fetchAiModels({ signal: modelsController.signal });
        if (modelsController.signal.aborted) return null;

        if (!modelsResult.success) {
          return {
            success: false,
            message: 'Failed to fetch models list',
            models: [],
          };
        }

        const models = modelsResult.data?.models || [];
        let message = '';
        if (balResult.data?.balances?.length) {
          const bal = balResult.data?.balances?.[0];
          message = `Connected · ${bal?.total_balance} ${bal?.currency}`;
        } else {
          message = 'Connected';
        }

        return {
          success: true,
          message,
          models,
        };
      } catch (e) {
        if ((e as Error)?.name === 'AbortError') return null;
        return { success: false, message: 'Network error', models: [] };
      }
    },
  };
};
