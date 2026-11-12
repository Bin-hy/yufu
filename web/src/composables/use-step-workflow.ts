import { shallowRef, readonly } from 'vue'
import { sessionEpoch } from '@/workspace/session'
export interface WorkflowStep {
    id: string
    label: string
    run: () => Promise<unknown>
}
export function createStepWorkflow(steps: WorkflowStep[]) {
    const completed = shallowRef<string[]>([]),
        epoch = sessionEpoch.value
    async function run() {
        for (const step of steps) {
            if (epoch !== sessionEpoch.value) throw Error('会话已改变')
            if (completed.value.includes(step.id)) continue
            await step.run()
            if (epoch !== sessionEpoch.value) throw Error('会话已改变')
            completed.value = [...completed.value, step.id]
        }
    }
    return { completed: readonly(completed), steps, run }
}
