import { defineConfig, configDefaults } from 'vitest/config'

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    // Agent worktrees under .claude/worktrees/ are full checkouts of this repo, so without
    // this the whole suite runs twice — doubling CPU load and timing out the heavier
    // scenario tests (observed: wf_tornado hitting the 30 s limit during `validate:product`).
    exclude: [...configDefaults.exclude, '.claude/worktrees/**', 'tmp/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      reportsDirectory: 'coverage',
      all: true,
      include: ['src/**/*.{ts,tsx,js,jsx,svelte}'],
    },
  },
})
