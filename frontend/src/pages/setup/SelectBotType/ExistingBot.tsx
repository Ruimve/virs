import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Alert } from '@/components/Alert';
import { Button } from '@/components/Button';

export const ExistingBot = memo(({ botId }: { botId: string }) => {
  const navigate = useNavigate();
  return (
    <div>
      <Alert type="warning" title="每个账号只能创建一个机器人，请先删除已有机器人。">
        <Button
          variant="accent-outline"
          size="small"
          responsive={false}
          onClick={() => {
            navigate(`/trade/bot/${botId}/bot`, { replace: true });
          }}
        >
          查看已有机器人
        </Button>
      </Alert>
    </div>
  );
});
