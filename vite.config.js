import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
// import { execSync } from 'child_process';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
      svelte(),
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
