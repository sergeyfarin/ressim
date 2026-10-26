import { defineConfig } from '@playwright/test'
import { buildLayout } from './scripts/build-layout.ts'

const layout = buildLayout()
const publicUrl = process.env.RESIM_PUBLIC_URL ?? `http://127.0.0.1:4173${layout.basePath}`

export default defineConfig({
  testDir: './tests/deployed',
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  timeout: 10 * 60 * 1000,
  expect: { timeout: 60_000 },
  reporter: process.env.CI ? [['line'], ['html', { open: 'never' }]] : 'list',
  use: {
    // Relative page/metadata URLs must resolve inside the application directory.
    baseURL: publicUrl.endsWith('/') ? publicUrl : `${publicUrl}/`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        browserName: 'chromium',
        launchOptions: {
          args: ['--use-gl=swiftshader', '--enable-webgl'],
        },
      },
    },
  ],
})
