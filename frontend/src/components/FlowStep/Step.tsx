import { useState, useEffect, type ReactNode } from 'react'

export type FlowStepStatus = 'pending' | 'active' | 'verifying' | 'done' | 'error'

export interface FlowStepProps {
  step: number | ReactNode
  title: string
  description?: string
  status: FlowStepStatus
  summary?: string | ReactNode
  editable?: boolean
  showLine?: boolean
  indicator?: ReactNode
  onToggle?: (expanded: boolean) => void
  children?: ReactNode
}

function FlowStep({
  step,
  title,
  description,
  status,
  summary,
  editable,
  showLine,
  indicator,
  onToggle,
  children,
}: FlowStepProps) {
  const [expanded, setExpanded] = useState(false)

  // Auto-expand when status becomes active/verifying/error, auto-collapse when done
  useEffect(() => {
    if (status === 'active' || status === 'verifying' || status === 'error') {
      setExpanded(true)
    }
    if (status === 'done') {
      setExpanded(false)
    }
  }, [status])

  const isEditable = editable !== false && status === 'done'
  const isCollapsed = status === 'done' && !expanded
  const showContent =
    status === 'active' ||
    status === 'verifying' ||
    status === 'error' ||
    (status === 'done' && expanded)

  const handleHeaderClick = () => {
    if (!isEditable) return
    const next = !expanded
    setExpanded(next)
    onToggle?.(next)
  }

  const shouldShowLine = showLine !== undefined ? showLine : !isCollapsed

  const defaultIndicator = (): ReactNode => {
    const stepContent =
      typeof step === 'number' ? <span className="text-[11px]">{step}</span> : step

    switch (status) {
      case 'pending':
        return (
          <div className="w-7 h-7 rounded-full border border-line-strong bg-surface-1 flex items-center justify-center text-on-surface-faint">
            {stepContent}
          </div>
        )
      case 'active':
        return (
          <div className="w-7 h-7 rounded-full bg-indigo-500/80 flex items-center justify-center text-white font-medium">
            {stepContent}
          </div>
        )
      case 'verifying':
        return (
          <div className="w-7 h-7 rounded-full bg-indigo-500/20 border border-indigo-500/30 flex items-center justify-center">
            <svg
              className="animate-spin w-3.5 h-3.5 text-indigo-400"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
              />
            </svg>
          </div>
        )
      case 'done':
        return (
          <div className="w-7 h-7 rounded-full bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center">
            <svg
              className="w-3.5 h-3.5 text-emerald-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          </div>
        )
      case 'error':
        return (
          <div className="w-7 h-7 rounded-full bg-red-500/15 border border-red-500/30 flex items-center justify-center">
            <svg
              className="w-3.5 h-3.5 text-red-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2.5"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
        )
    }
  }

  const titleColor = () => {
    switch (status) {
      case 'pending':
        return 'text-on-surface-faint'
      case 'done':
        return 'text-on-surface-tertiary'
      default:
        return 'text-on-surface'
    }
  }

  return (
    <div className="flex gap-3">
      <div className="flex flex-col items-center">
        <div className={isEditable ? 'cursor-pointer' : ''} onClick={handleHeaderClick}>
          {indicator ?? defaultIndicator()}
        </div>
        {shouldShowLine && <div className="w-px flex-1 min-h-[16px] bg-line-default mt-1" />}
      </div>

      <div className="flex-1 pb-4">
        <div
          className={`flex items-center gap-2 ${isEditable ? 'cursor-pointer group' : ''}`}
          onClick={handleHeaderClick}
        >
          <p className={`text-sm font-medium leading-7 ${titleColor()}`}>{title}</p>
          {isEditable && (
            <span className="text-[10px] text-on-surface-muted group-hover:text-on-surface-tertiary transition-colors">
              {expanded ? 'collapse' : 'edit'}
            </span>
          )}
        </div>

        {description && (status !== 'done' || expanded) && (
          <p className="text-[12px] text-on-surface-muted -mt-1 mb-1">{description}</p>
        )}

        {isCollapsed && summary && (
          <div className="text-[12px] text-on-surface-muted -mt-1">{summary}</div>
        )}

        {showContent && <div className="mt-2">{children}</div>}
      </div>
    </div>
  )
}

export default FlowStep
