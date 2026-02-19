import daisyui from 'daisyui';
import type { Config } from 'tailwindcss';

const config: Config = {
  content: ['./index.html', './src/**/*.{svelte,js,ts,jsx,tsx}'],
  theme: {
    extend: {},
  },
  plugins: [daisyui],
};

export default config;