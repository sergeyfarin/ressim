import { expect, test } from '@playwright/test'

// Exercise the actual Svelte selector, including the change from preview variant
// keys to completed run records. Model-only tests cannot detect lost visibility.
test('a hidden analytical case keeps its identity and color after completion', async ({ page }) => {
  await page.addInitScript(() => {
    const NativeWorker = window.Worker
    window.Worker = class extends NativeWorker {
      postMessage(message: unknown, options?: StructuredSerializeOptions | Transferable[]): void {
        const envelope = message as { type?: string; payload?: Record<string, unknown> }
        const outgoing = envelope?.type === 'run' && envelope.payload
          ? { ...envelope, payload: { ...envelope.payload, steps: 1 } }
          : message
        super.postMessage(outgoing, options as StructuredSerializeOptions)
      }
    }
  })
  await page.goto('./', { waitUntil: 'networkidle' })
  await page.getByTestId('scenario-dep_decline').click()
  await page.getByTestId('sensitivity-permeability').click()
  for (const key of ['perm_tight', 'perm_base', 'perm_good']) {
    const button = page.getByTestId(`variant-${key}`)
    if (await button.getAttribute('aria-pressed') !== 'true') await button.click()
  }
  const preview = page.locator('button[title^="Hide"][title*="tight"][title*="analytical preview"]')
  await expect(preview).toBeVisible()
  const color = await preview.locator('svg line').first().getAttribute('stroke')
  await preview.click()
  await page.getByRole('button', { name: 'Run 3 Sensitivities', exact: true }).click()
  await expect(page.getByTestId('run-status')).toHaveText('Complete', { timeout: 180_000 })
  const completed = page.locator('button[title^="Show"][title*="tight"]')
  await expect(completed).toBeVisible()
  await expect(completed.locator('svg line').first()).toHaveAttribute('stroke', color!)

  // Other studies reuse variant keys; visibility belongs to this case family.
  await page.getByTestId('scenario-dep_welltest').click()
  await page.getByTestId('sensitivity-permeability').click()
  for (const key of ['perm_tight', 'perm_base', 'perm_good']) {
    const button = page.getByTestId(`variant-${key}`)
    if (await button.getAttribute('aria-pressed') !== 'true') await button.click()
  }
  await expect(page.locator('button[title^="Hide"][title*="tight"][title*="analytical preview"]')).toBeVisible()
})
