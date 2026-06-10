import { type Component, type JSX, For, Show } from 'solid-js'
import FlowStep, { type FlowStepStatus } from './Step'

export type { FlowStepStatus }

export interface FlowStepConfig {
  /** Unique key for this step */
  key: string
  /** Step title */
  title: string
  /** Optional subtitle / description */
  description?: string
  /** Whether done step can be expanded/collapsed. Default: true */
  editable?: boolean
  /** Override connector line visibility */
  showLine?: boolean
  /** Custom indicator */
  indicator?: JSX.Element
  /** Step number override (default: auto-increment) */
  step?: number
  /** Render step content */
  render?: () => JSX.Element
}

export interface FlowStepsProps {
  /** Step configurations */
  steps: FlowStepConfig[]
  /** Status map: key → status */
  statuses: Record<string, FlowStepStatus>
  /** Summary map: key → summary (string or JSX) */
  summaries?: Record<string, string | JSX.Element>
  /** Toggle callback map: key → onToggle */
  onToggles?: Record<string, (expanded: boolean) => void>
}

const FlowSteps: Component<FlowStepsProps> = (props) => {
  return (
    <div class="space-y-1">
      <For each={props.steps}>
        {(config, index) => {
          const status = () => props.statuses[config.key] ?? 'pending'
          const summary = () => props.summaries?.[config.key]
          const onToggle = () => props.onToggles?.[config.key]

          return (
            <FlowStep
              step={config.step ?? index() + 1}
              title={config.title}
              description={config.description}
              status={status()}
              summary={summary()}
              editable={config.editable}
              showLine={config.showLine}
              indicator={config.indicator}
              onToggle={onToggle()}
            >
              <Show when={config.render}>
                {config.render!()}
              </Show>
            </FlowStep>
          )
        }}
      </For>
    </div>
  )
}

export default FlowSteps
