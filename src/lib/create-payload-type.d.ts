import type { SimulatorCreatePayload } from './simulator-types';
import type { buildCreatePayloadFromState } from './buildCreatePayload';

// Compile-time assertion: the helper's return type must be assignable to SimulatorCreatePayload.
// This is a type-only file (.d.ts) so it won't be picked up by Vitest but will be
// validated by `tsc --noEmit` and in CI.
type AssertTrue<T extends true> = T;
type PayloadAssignable = [ReturnType<typeof buildCreatePayloadFromState>] extends [SimulatorCreatePayload] ? true : false;
type _checkPayload = AssertTrue<PayloadAssignable>;
