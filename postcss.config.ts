import type { ProcessOptions } from 'postcss';

type PostcssConfig = {
  plugins: Record<string, ProcessOptions>;
};

const config: PostcssConfig = {
  plugins: {},
};

export default config;