import { expect, test, type Page } from '@playwright/test'

// The compositional scenario end to end in a real browser: the run-set queue drives the
// compositional engine, and the chart draws compositional quantities and no black-oil panel.

async function selectOnly(page: Page, dimension: string, variants: string[]): Promise<void> {
  await page.getByTestId('scenario-comp_co2_1d').click()
  await page.getByTestId(`sensitivity-${dimension}`).click()
  const buttons = page.locator('[data-testid^="variant-"]')
  for (let index = 0; index < await buttons.count(); index += 1) {
    const button = buttons.nth(index)
    const key = (await button.getAttribute('data-testid'))!.replace('variant-', '')
    const pressed = await button.getAttribute('aria-pressed') === 'true'
    if (pressed !== variants.includes(key)) await button.click()
  }
}

test('a compositional run set draws compositional panels and no black-oil ones', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  await page.goto('./', { waitUntil: 'networkidle' })
  await selectOnly(page, 'timestep', ['dt_coarse', 'dt_base'])
  await page.getByRole('button', { name: 'Run 2 Sensitivities', exact: true }).click()
  await expect(page.getByTestId('run-status')).toHaveText('Complete', { timeout: 180_000 })

  for (const title of ['CO2 In Place (mol)', 'C10 In Place (mol)', 'Average Vapour Saturation']) {
    await expect(page.getByText(title, { exact: true }).first()).toBeVisible()
  }
  for (const title of ['Oil Rate', 'GOR', 'p/z', 'Recovery Factor', 'Cum Oil']) {
    await expect(page.getByText(title, { exact: true })).toHaveCount(0)
  }
  // Both cases completed and are selectable by their own labels.
  await expect(page.locator('button[title="Hide 1D Compositional CO₂ Flood — 0.25 d"]')).toBeVisible()
  await expect(page.locator('button[title="Hide 1D Compositional CO₂ Flood — 0.05 d"]')).toBeVisible()

  // The spatial views offer each component's mole fraction, and no water on a water-free grid.
  const spatial = page.getByTestId('three-d-view-card')
  for (const label of ['z CO2', 'z C1', 'z C10']) {
    await expect(spatial.getByRole('button', { name: label, exact: true })).toBeVisible()
  }
  await expect(spatial.getByRole('button', { name: 'Water Sat', exact: true })).toHaveCount(0)
  await spatial.getByRole('button', { name: 'z CO2', exact: true }).click()
  await expect(spatial.getByText('Mole Fraction z CO2 Profile')).toBeVisible()
  expect(errors).toEqual([])
})

test('a compositional run set can be stopped mid-run', async ({ page }) => {
  await page.goto('./', { waitUntil: 'networkidle' })
  await selectOnly(page, 'grid_refinement', ['grid_40'])
  await page.getByRole('button', { name: /^Run/ }).click()
  await expect(page.getByTestId('run-status')).not.toHaveText('Complete')
  // Let it get into the batch, then stop: the worker must notice between report steps.
  await page.waitForTimeout(1_500)
  await page.getByRole('button', { name: /^Stop/ }).click()
  await expect(page.getByText(/Run set stopped after 0 completed run/)).toBeVisible({ timeout: 15_000 })
})

test('a compositional case that fails keeps its partial run and says why, without solver jargon', async ({ page }) => {
  // A real failure cannot be provoked on demand, so the worker's report of one is substituted at
  // the message boundary. Everything downstream of it — the store, the result, the warning — is real.
  const reason = 'The solver could not converge after 40 steps (10.000 days). Try a smaller timestep.'
  await page.addInitScript((message) => {
    const NativeWorker = window.Worker
    window.Worker = class extends NativeWorker {
      set onmessage(handler: ((event: MessageEvent) => void) | null) {
        super.onmessage = handler && ((event: MessageEvent) => {
          const data = event.data as { type?: string; reason?: string }
          handler(data?.type === 'compositionalStopped' && data.reason === 'completed'
            ? new MessageEvent('message', { data: { ...data, reason: 'failed', message } })
            : event)
        })
      }
      get onmessage() {
        return super.onmessage
      }
    }
  }, reason)
  await page.goto('./', { waitUntil: 'networkidle' })
  await selectOnly(page, 'timestep', ['dt_coarse'])
  await page.getByRole('button', { name: /^Run \d/ }).click()
  await expect(page.getByTestId('run-status')).toHaveText('Complete', { timeout: 180_000 })
  await expect(page.getByText(`0.25 d: ${reason}`).first()).toBeVisible()
  await expect(page.getByText('CO2 In Place (mol)', { exact: true }).first()).toBeVisible()
  await expect(page.getByText(/jacobian|residual|newton/i)).toHaveCount(0)
})
