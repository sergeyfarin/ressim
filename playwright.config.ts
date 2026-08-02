import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/deployed',
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  timeout: 10 * 60 * 1000,
  expect: { timeout: 60_000 },
  reporter: process.env.CI ? [['line'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: process.env.RESIM_PUBLIC_URL ?? 'http://127.0.0.1:4173/ressim/',
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
