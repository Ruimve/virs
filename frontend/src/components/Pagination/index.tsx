import { memo } from 'react';

interface PaginationProps {
  total: number;
  page: number;
  totalPages: number;
  loading?: boolean;
  onPrev: () => void;
  onNext: () => void;
}

const Pagination = memo(({ total, page, totalPages, loading, onPrev, onNext }: PaginationProps) => {
  if (totalPages <= 1) return null;
  return (
    <div className="flex items-center justify-between px-5 py-3 border-t border-line-subtle text-xs">
      <span className="text-on-surface-tertiary">
        共 {total} 条 · 第 {page}/{totalPages} 页
      </span>
      <div className="flex items-center gap-2">
        <button
          onClick={onPrev}
          disabled={page <= 1 || loading}
          className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2 transition-colors"
        >
          上一页
        </button>
        <button
          onClick={onNext}
          disabled={page >= totalPages || loading}
          className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2 transition-colors"
        >
          下一页
        </button>
      </div>
    </div>
  );
});

export default Pagination;
