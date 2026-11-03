import { describe, it } from 'vitest';

describe.skip('retired suite', () => {
    it('mixed_suite', () => {});
});

describe('live suite', () => {
    it('mixed_suite', () => {});
});
