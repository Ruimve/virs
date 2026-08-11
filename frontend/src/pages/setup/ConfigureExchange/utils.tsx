import { saveCredential, testCredential } from '@/service';

export const normalizePemSecret = (raw: string): string => {
  const value = raw.trim();

  const beginIdx = value.search(/-----BEGIN [A-Z ]*PRIVATE KEY-----/);
  const endIdx = value.search(/-----END [A-Z ]*PRIVATE KEY-----/);
  if (beginIdx === -1 || endIdx === -1 || beginIdx >= endIdx) {
    return value;
  }

  const header = value.slice(beginIdx, value.indexOf('-----', beginIdx + 10) + 5);
  const footer = value.slice(endIdx, value.indexOf('-----', endIdx + 8) + 5);

  const bodyStart = value.indexOf(header) + header.length;
  const bodyEnd = value.indexOf(footer);
  const body = value.slice(bodyStart, bodyEnd).trim();

  if (body.includes('\n')) {
    return `${header}\n${body}\n${footer}`;
  }

  const lines: string[] = [];
  for (let i = 0; i < body.length; i += 64) {
    lines.push(body.slice(i, i + 64));
  }
  return `${header}\n${lines.join('\n')}\n${footer}`;
};

interface CheckApiKeyResult {
  controllers: AbortController[];
  check: () => Promise<{
    success: boolean;
    message: string;
  } | null>;
}

export const checkApiKey = (apiKey: string, apiSecret: string): CheckApiKeyResult => {
  const saveController = new AbortController();
  const testController = new AbortController();

  return {
    controllers: [saveController, testController],
    check: async () => {
      try {
        // 保存凭证
        const saveResult = await saveCredential({
          exchange: 'binance',
          api_key: apiKey,
          api_secret: apiSecret,
          label: 'binance verification',
        });
        if (saveController.signal.aborted) return null;

        if (!saveResult.success) {
          return { success: false, message: 'Failed to save credentials' };
        }

        // 测试凭证
        const testResult = await testCredential();
        if (testController.signal.aborted) return null;

        if (!testResult.success || !testResult.data?.connected) {
          return { success: false, message: 'Connection failed' };
        }

        return { success: true, message: 'Connected' };
      } catch (e) {
        if ((e as Error)?.name === 'AbortError') return null;
        return { success: false, message: 'Network error' };
      }
    },
  };
};
