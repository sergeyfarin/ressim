import { describe, it } from 'vitest';

describe.each([1, 2])
    .skip('suite', () => {
        it('describe_each_skip', () => {});
    });
