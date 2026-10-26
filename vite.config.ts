import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { promises as fs } from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'
import { buildLayout } from './scripts/build-layout.ts'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

const buildSha = process.env.RESIM_BUILD_SHA ?? process.env.GITHUB_SHA ?? 'local'
const buildRef = process.env.RESIM_BUILD_REF ?? process.env.GITHUB_REF_NAME ?? 'local'
const layout = buildLayout()

export default defineConfig({
  plugins: [
    svelte(),
    // Only the legacy layout needs a redirect beside its application directory.
    {
      name: 'root-redirect',
      apply: 'build',
      async closeBundle() {
        if (layout.basePath === '/') return
        const outDir = path.resolve(__dirname, layout.outDir, '..')
        const target = layout.basePath
        const html = `<!doctype html><html><head><meta charset="utf-8"/><meta http-equiv="refresh" content="0;url=${target}"><meta name="robots" content="noindex"><title>Redirecting...</title><script>location.replace('${target}')</script></head><body>Redirecting to <a href="${target}">${target}</a></body></html>`
        await fs.mkdir(outDir, { recursive: true })
        await fs.writeFile(path.join(outDir, 'index.html'), html, 'utf8')
      }
    },
    {
      name: 'build-metadata',
      generateBundle() {
        this.emitFile({ type: 'asset', fileName: 'build-info.json', source: `${JSON.stringify({
          commit: buildSha,
          ref: buildRef,
          basePath: layout.basePath,
          builtAt: new Date().toISOString(),
          source: buildSha === 'local'
            ? null
            : `https://github.com/sergeyfarin/ressim/commit/${buildSha}`,
        }, null, 2)}\n` })
      },
    },
  ],
  base: layout.basePath,
  build: {
    chunkSizeWarningLimit: 600,
    outDir: layout.outDir,
    emptyOutDir: true,
    rollupOptions: {
      // Multi-page. `index.html` is the application; `fractional-flow.html` is a second entry
      // point built only from workspace packages, which is what makes "the frontend can be
      // composed into more than one page" checkable rather than asserted.
      input: {
        index: path.resolve(__dirname, 'index.html'),
        'fractional-flow': path.resolve(__dirname, 'fractional-flow.html'),
      },
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
