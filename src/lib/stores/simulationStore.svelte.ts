/**
 * Transitional barrel — re-exports from the three focused stores created in Phase 6.
 *
 * During the migration window, existing consumers that import `createSimulationStore`
 * continue to work. After all consumers are updated to import directly from the
 * three sub-store files, this barrel will be deleted (Phase 7+).
 */
export { createParameterStore, type ParameterStore } from './parameterStore.svelte';
export { createRuntimeStore, type RuntimeStore } from './runtimeStore.svelte';
export { createNavigationStore, type NavigationStore } from './navigationStore.svelte';

import { createParameterStore } from './parameterStore.svelte';
import { createRuntimeStore } from './runtimeStore.svelte';
import { createNavigationStore } from './navigationStore.svelte';

export function createSimulationStore() {
    const params = createParameterStore();
    const runtime = createRuntimeStore(params);
    const nav = createNavigationStore(params, runtime);
    runtime.connectNavigation(nav);
    return { params, runtime, nav };
}

export type SimulationStore = ReturnType<typeof createSimulationStore>;
