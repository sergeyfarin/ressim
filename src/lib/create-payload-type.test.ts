import type { AppCreatePayload } from '../App.svelte';
import type { SimulatorCreatePayload } from './simulator-types';

// Compile-time assertion: AppCreatePayload must be assignable to SimulatorCreatePayload.
// If this breaks, `tsc --noEmit` will fail in CI.
type Assert<T extends true> = T;
type PayloadAssignable = [AppCreatePayload] extends [SimulatorCreatePayload] ? true : false;
// @ts-expect-error will surface a readable error if the assertion fails
type _checkPayload: Assert<PayloadAssignable> = true;