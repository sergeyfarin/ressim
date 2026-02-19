import type { SimulatorCreatePayload } from './simulator-types';
import type { buildCreatePayloadFromState } from './buildCreatePayload';

// Compile-time assertion: the helper's return type must be assignable to SimulatorCreatePayload.
// If this breaks, `tsc --noEmit` will fail in CI.
type AssertTrue<T extends true> = T;
type PayloadAssignable = [ReturnType<typeof buildCreatePayloadFromState>] extends [SimulatorCreatePayload] ? true : false;
// compile-time check — will produce a type error if `PayloadAssignable` is false
type _checkPayload = AssertTrue<PayloadAssignable>;