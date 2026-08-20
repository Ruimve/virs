import { memo, useCallback, useEffect, useState } from 'react';
import { FormField } from '../../components';
import { checkPermissions, type PermissionItem } from '@/service';
import { Check, Warning, Close } from '@/components/Icon';

const StatusIcon = {
  ok: <Check width={14} height={14} strokeWidth={2.5} className="text-success-text" />,
  warn: <Warning width={14} height={14} strokeWidth={2} className="text-warning-text" />,
  error: <Close width={14} height={14} strokeWidth={2} className="text-danger-text" />,
};

const StatusColor = {
  ok: 'text-success-text',
  warn: 'text-warning-text',
  error: 'text-danger-text',
};

interface PermissionProps {
  onCheck: (success: boolean) => void;
}

export const Permission = memo(({ onCheck }: PermissionProps) => {
  const [loading, setLoading] = useState(false);
  const [permissions, setPermissions] = useState<PermissionItem[]>([]);
  const [error, setError] = useState<string | null>(null);

  const getPermissions = useCallback(async () => {
    try {
      setLoading(true);
      const result = await checkPermissions();
      if (!result.success) {
        setError('Failed to check permissions');
        return onCheck(false);
      }
      setError(null);
      const permissions = result.data?.permissions || [];
      setPermissions(permissions);

      const allOk = permissions.every((p) => p.status === 'ok' || p.status === 'warn');
      if (!allOk) {
        return onCheck(false);
      }
      onCheck(true);
    } catch {
      setError('Network error');
      onCheck(false);
    } finally {
      setLoading(false);
    }
  }, [onCheck]);

  useEffect(() => {
    getPermissions();
  }, [getPermissions]);

  const renderContent = useCallback(() => {
    if (loading) {
      return <p className="text-xs text-on-surface-tertiary">Checking permissions...</p>;
    }

    if (error) {
      return <p className="text-xs text-danger-text">{error}</p>;
    }

    return permissions.map((permission) => (
      <div
        key={permission.name}
        className="flex items-center justify-between py-1.5 first:pt-0 last:pb-0"
      >
        <div className="flex items-center gap-2">
          {StatusIcon[permission.status]}
          <span className="text-xs text-on-surface-tertiary">{permission.label}</span>
        </div>
        <span className={`text-caption ${StatusColor[permission.status]}`}>
          {permission.detail}
        </span>
      </div>
    ));
  }, [loading, error, permissions]);

  return <FormField label="Permissions">{renderContent()}</FormField>;
});
