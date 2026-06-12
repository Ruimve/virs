import { type Component, type JSX, Show, createSignal, createEffect } from 'solid-js'

export type FlowStepStatus = 'pending' | 'active' | 'verifying' | 'done' | 'error'

export interface FlowStepProps {
  /** Step indicator: number or custom JSX (icon etc.) */
  step: number | JSX.Element
  /** Step title */
  title: string
  /** Optional subtitle / description */
  description?: string
  /** Current status */
  status: FlowStepStatus
  /** Summary shown when collapsed. Supports string or rich JSX */
  summary?: string | JSX.Element
  /** Whether done step can be expanded/collapsed by clicking. Default: true */
  editable?: boolean
  /** Override connector line visibility. Default: auto (hide when collapsed) */
  showLine?: boolean
  /** Custom indicator, overrides default */
  indicator?: JSX.Element
  /** Called when user toggles expand/collapse on a done step */
  onToggle?: (expanded: boolean) => void
  /** Step content */
  children?: JSX.Element
}

const FlowStep: Component<FlowStepProps> = (props) => {
  const [expanded, setExpanded] = createSignal(false)

  // Auto-expand when status becomes active/verifying/error
  createEffect(() => {
    if (props.status === 'active' || props.status === 'verifying' || props.status === 'error') {
      setExpanded(true)
    }
    // Auto-collapse when status becomes done
    if (props.status === 'done') {
      setExpanded(false)
    }
  })

  const isEditable = () => props.editable !== false && props.status === 'done'
  const isCollapsed = () => props.status === 'done' && !expanded()
  const showContent = () =>
    props.status === 'active' ||
    props.status === 'verifying' ||
    props.status === 'error' ||
    (props.status === 'done' && expanded())

  const handleHeaderClick = () => {
    if (!isEditable()) return
    const next = !expanded()
    setExpanded(next)
    props.onToggle?.(next)
  }

  // Connector line: auto or override
  const shouldShowLine = () => {
    if (props.showLine !== undefined) return props.showLine
    return !isCollapsed()
  }

  // Default indicator based on status
  const defaultIndicator = (): JSX.Element => {
    const stepContent = typeof props.step === 'number'
      ? <span class="text-[11px]">{props.step}</span>
      : props.step

    switch (props.status) {
      case 'pending':
        return (
          <div class="w-7 h-7 rounded-full border border-line-strong bg-surface-1 flex items-center justify-center text-on-surface-faint">
            {stepContent}
          </div>
        )
      case 'active':
        return (
          <div class="w-7 h-7 rounded-full bg-indigo-500/80 flex items-center justify-center text-white font-medium">
            {stepContent}
          </div>
        )
      case 'verifying':
        return (
          <div class="w-7 h-7 rounded-full bg-indigo-500/20 border border-indigo-500/30 flex items-center justify-center">
            <svg class="animate-spin w-3.5 h-3.5 text-indigo-400" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
          </div>
        )
      case 'done':
        return (
          <div class="w-7 h-7 rounded-full bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center">
            <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          </div>
        )
      case 'error':
        return (
          <div class="w-7 h-7 rounded-full bg-red-500/15 border border-red-500/30 flex items-center justify-center">
            <svg class="w-3.5 h-3.5 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
        )
    }
  }

  const titleColor = () => {
    switch (props.status) {
      case 'pending': return 'text-on-surface-faint'
      case 'done': return 'text-on-surface-tertiary'
      default: return 'text-on-surface'
    }
  }

  return (
    <div class="flex gap-3">
      {/* Left: indicator + connector line */}
      <div class="flex flex-col items-center">
        <div
          class={isEditable() ? 'cursor-pointer' : ''}
          onClick={handleHeaderClick}
        >
          {props.indicator ?? defaultIndicator()}
        </div>
        <Show when={shouldShowLine()}>
          <div class="w-px flex-1 min-h-[16px] bg-line-default mt-1" />
        </Show>
      </div>

      {/* Right: title + description + content/summary */}
      <div class="flex-1 pb-4">
        <div
          class={`flex items-center gap-2 ${isEditable() ? 'cursor-pointer group' : ''}`}
          onClick={handleHeaderClick}
        >
          <p class={`text-sm font-medium leading-7 ${titleColor()}`}>
            {props.title}
          </p>
          <Show when={isEditable()}>
            <span class="text-[10px] text-on-surface-muted group-hover:text-on-surface-tertiary transition-colors">
              {expanded() ? 'collapse' : 'edit'}
            </span>
          </Show>
        </div>

        {/* Description */}
        <Show when={props.description && (props.status !== 'done' || expanded())}>
          <p class="text-[12px] text-on-surface-muted -mt-1 mb-1">{props.description}</p>
        </Show>

        {/* Collapsed summary */}
        <Show when={isCollapsed() && props.summary}>
          <div class="text-[12px] text-on-surface-muted -mt-1">
            {typeof props.summary === 'string' ? props.summary : props.summary}
          </div>
        </Show>

        {/* Expanded content */}
        <Show when={showContent()}>
          <div class="mt-2">
            {props.children}
          </div>
        </Show>
      </div>
    </div>
  )
}

export default FlowStep
