import { expect, test, type Page } from '@playwright/test'

// The 3D view and the spatial profile open on the property each scenario declares. The profile's
// heading names that property, so it is what these tests read.

function profileHeading(page: Page) {
  return page.getByTestId('three-d-view-card').locator('h4').first()
}

test('each scenario opens the spatial views on its declared property', async ({ page }) => {
  await page.goto('./', { waitUntil: 'networkidle' })
  const expected: Array<[string, string]> = [
    ['wf_bl1d', 'Water Saturation Sw Profile'],
    // A gas reservoir names gas as its fluid but injects nothing: its blowdown is a pressure view.
    ['dep_gas_pz', 'Pressure (bar) Profile'],
    ['dep_welltest', 'Pressure (bar) Profile'],
    ['gas_injection', 'Gas Saturation Sg Profile'],
  ]
  for (const [key, heading] of expected) {
    await page.getByTestId(`scenario-${key}`).click()
    await expect(profileHeading(page)).toHaveText(heading)
  }
})

test('entering a scenario reapplies its default after a manual pick', async ({ page }) => {
  await page.goto('./', { waitUntil: 'networkidle' })
  await page.getByTestId('scenario-gas_injection').click()
  await page.getByTestId('three-d-view-card').getByRole('button', { name: 'Pressure', exact: true }).click()
  await expect(profileHeading(page)).toHaveText('Pressure (bar) Profile')
  // gas_drive declares the same default as gas_injection.
  await page.getByTestId('scenario-gas_drive').click()
  await expect(profileHeading(page)).toHaveText('Gas Saturation Sg Profile')
})
