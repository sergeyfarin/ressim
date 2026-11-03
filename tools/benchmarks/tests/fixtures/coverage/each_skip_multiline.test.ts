import { it } from 'vitest';

it.each([1, 2])
    .skip('each_skip_multiline', () => {});
