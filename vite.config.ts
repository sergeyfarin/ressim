import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { promises as fs } from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

const buildSha = process.env.RESIM_BUILD_SHA ?? process.env.GITHUB_SHA ?? 'local'
const buildRef = process.env.RESIM_BUILD_REF ?? process.env.GITHUB_REF_NAME ?? 'local'

export default defineConfig({
  plugins: [
    svelte(),
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
    {
      name: 'build-metadata',
      async closeBundle() {
        const outputPath = path.resolve(__dirname, 'dist/ressim/build-info.json')
        await fs.mkdir(path.dirname(outputPath), { recursive: true })
        await fs.writeFile(outputPath, `${JSON.stringify({
          commit: buildSha,
          ref: buildRef,
          builtAt: new Date().toISOString(),
          source: buildSha === 'local'
            ? null
            : `https://github.com/sergeyfarin/ressim/commit/${buildSha}`,
        }, null, 2)}\n`, 'utf8')
      },
    },
  ],
  base: '/ressim/',
  build: {
    chunkSizeWarningLimit: 600,
    outDir: 'dist/ressim',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id: string) {
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
