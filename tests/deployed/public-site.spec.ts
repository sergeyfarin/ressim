import { expect, test, type Page } from '@playwright/test'

const expectedCommit = process.env.RESIM_EXPECTED_SHA

async function selectVariants(page: Page, dimension: string, variants: string[]): Promise<void> {
  await page.getByRole('button', { name: dimension, exact: true }).click()
  const wanted = new Set(variants)

  for (const button of await page.locator('button.ui-chip').all()) {
    const label = (await button.textContent())?.trim() ?? ''
    const selected = (await button.getAttribute('aria-pressed')) === 'true'
    if (selected !== wanted.has(label)) await button.click()
  }

  for (const variant of variants) {
    await expect(page.getByRole('button', { name: variant, exact: true })).toHaveAttribute('aria-pressed', 'true')
  }
}

test('public deployment loads assets and runs IMPES, FIM, charts, and 3D', async ({ page, request }) => {
  const runtimeErrors: string[] = []
  const failedResponses: string[] = []

  page.on('pageerror', (error) => runtimeErrors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeErrors.push(message.text())
  })
  page.on('response', (response) => {
    if (response.status() >= 400) failedResponses.push(`${response.status()} ${response.url()}`)
  })

  // Keep the smoke bounded while exercising the real sensitivity runner and deployed Worker.
  // Scientific horizons remain covered by validate:product; this gate proves that both solver
  // configurations can initialize and advance one report step in the published application.
  await page.addInitScript(() => {
    const NativeWorker = window.Worker
    const smoke = { createFimFlags: [] as boolean[], runCount: 0 }
    ;(window as typeof window & { __ressimSmoke?: typeof smoke }).__ressimSmoke = smoke

    window.Worker = class extends NativeWorker {
      postMessage(message: unknown, transferOrOptions?: StructuredSerializeOptions | Transferable[]): void {
        let outgoing = message
        if (message && typeof message === 'object') {
          const envelope = message as { type?: string; payload?: Record<string, unknown> }
          if (envelope.type === 'create') {
            smoke.createFimFlags.push(envelope.payload?.fimEnabled === true)
          } else if (envelope.type === 'run' && envelope.payload) {
            smoke.runCount += 1
            outgoing = { ...envelope, payload: { ...envelope.payload, steps: 1 } }
          }
        }
        super.postMessage(outgoing, transferOrOptions as StructuredSerializeOptions)
      }
    }
  })

  const buildInfoResponse = await request.get('build-info.json')
  expect(buildInfoResponse.ok()).toBeTruthy()
  const buildInfo = await buildInfoResponse.json() as { commit: string }
  if (expectedCommit) expect(buildInfo.commit).toBe(expectedCommit)

  await page.goto('./', { waitUntil: 'networkidle' })
  await expect(page).toHaveTitle(/ResSim/)
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', 'https://farin.nl/ressim/')
  await expect(page.getByTestId('run-status')).toHaveText('Ready')

  await page.reload({ waitUntil: 'networkidle' })
  await expect(page.getByTestId('run-status')).toHaveText('Ready')

  await selectVariants(page, 'Solver (FIM vs IMPES)', ['IMPES 5-day steps', 'FIM 5-day steps'])
  await page.getByRole('button', { name: 'Run 2 Sensitivities', exact: true }).click()
  await expect(page.getByTestId('run-status')).toHaveText('Complete', { timeout: 3 * 60 * 1000 })

  const smokeState = await page.evaluate(() => (
    window as typeof window & { __ressimSmoke?: { createFimFlags: boolean[]; runCount: number } }
  ).__ressimSmoke)
  expect(smokeState?.runCount).toBe(2)
  expect(smokeState?.createFimFlags).toEqual(expect.arrayContaining([false, true]))

  await expect.poll(async () => page.locator('canvas').count()).toBeGreaterThanOrEqual(3)
  await expect(page.getByTestId('three-d-view-card')).toBeVisible()
  await expect.poll(async () => page.evaluate(() => Boolean((window as typeof window & { __ressim?: unknown }).__ressim))).toBe(true)

  expect(failedResponses).toEqual([])
  expect(runtimeErrors).toEqual([])
})
