import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite';
import { promises as fs } from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
// import { execSync } from 'child_process';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
      svelte(),
      tailwindcss(),
      // write a simple redirect HTML file into the build output after bundling
      {
        name: 'root-redirect',
        async closeBundle() {
          try {
            const outDir = path.resolve(__dirname, 'dist')
            const target = '/ressim/'
            const html = `<!doctype html><html><head><meta charset="utf-8"/><meta http-equiv="refresh" content="0;url=${target}"><meta name="robots" content="noindex"><title>Redirecting...</title><script>location.replace('${target}')</script></head><body>Redirecting to <a href="${target}">${target}</a></body></html>`
            await fs.mkdir(outDir, { recursive: true })
            await fs.writeFile(path.join(outDir, 'index.html'), html, 'utf8')
            console.log('[vite] wrote root redirect to', path.join(outDir, 'index.html'))
          } catch (err) {
            console.error('[vite] failed to write root redirect', err)
          }
        }
      },
    // {
    //     name: 'wasm-auto-build',
    //     buildStart() {
    //         console.log('🦀 Building Rust → WASM...');
    //         execSync('cd src/lib/ressim && wasm-pack build --target web --out-dir ./pkg', { stdio: 'inherit' });
    //     }
    // }
  ],
  base: '/ressim/',
  build: {
    outDir: 'dist/ressim',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/three')) {
            return 'vendor-three'
          }
          if (id.includes('node_modules/chart.js')) {
            return 'vendor-chartjs'
          }
        },
      },
    },
  },
})
