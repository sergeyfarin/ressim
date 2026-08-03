import { expect, test, type Page } from '@playwright/test'

const expectedCommit = process.env.RESIM_EXPECTED_SHA

async function selectVariants(page: Page, dimensionKey: string, variantKeys: string[]): Promise<void> {
  const dimension = page.getByTestId(`sensitivity-${dimensionKey}`)
  await expect(dimension).toBeVisible({ timeout: 15_000 })
  await dimension.click()
  const wanted = new Set(variantKeys)

  for (const button of await page.locator('[data-testid^="variant-"]').all()) {
    const key = (await button.getAttribute('data-testid'))?.replace('variant-', '') ?? ''
    const selected = (await button.getAttribute('aria-pressed')) === 'true'
    if (selected !== wanted.has(key)) await button.click()
  }

  for (const variantKey of variantKeys) {
    await expect(page.getByTestId(`variant-${variantKey}`)).toHaveAttribute('aria-pressed', 'true')
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

  const numericalConvergenceScenario = page.getByTestId('scenario-wf_numerics')
  await expect(numericalConvergenceScenario).toBeVisible({ timeout: 15_000 })
  await numericalConvergenceScenario.click()
  await selectVariants(page, 'solver_formulation', ['solver_impes_base', 'solver_fim_base'])
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
