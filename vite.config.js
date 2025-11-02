import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite';
// import { execSync } from 'child_process';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
      svelte(),
      tailwindcss(),
    // {
    //     name: 'wasm-auto-build',
    //     buildStart() {
    //         console.log('🦀 Building Rust → WASM...');
    //         execSync('cd src/lib/ressim && wasm-pack build --target web --out-dir ./pkg', { stdio: 'inherit' });
    //     }
    // }
  ],
  base: '/ressim/',
})
