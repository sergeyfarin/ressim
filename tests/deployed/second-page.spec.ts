import { expect, test } from '@playwright/test'

/**
 * S3's acceptance: a second page built only from workspace packages actually works in a browser.
 *
 * The build passing proves the modules resolve. It does not prove the page renders, that the
 * chart package draws when handed data by something other than the application, or that Tailwind
 * emitted the classes the packaged components ask for — and that last one fails *silently*, with
 * no error anywhere, which is why it is asserted here against computed style rather than inferred
 * from a byte count.
 *
 * Deliberately no simulator: this page composes `@ressim/analytical`, `@ressim/charts` and
 * `@ressim/primitives` and touches no worker, no WASM and no store. If this test ever needs to
 * wait for a simulation, the page under test has stopped being the thing S3 set out to show.
 */
test('the packages compose into a working standalone page', async ({ page }) => {
  const runtimeErrors: string[] = []
  const failedResponses: string[] = []

  page.on('pageerror', (error) => runtimeErrors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeErrors.push(message.text())
  })
  page.on('response', (response) => {
    if (response.status() >= 400) failedResponses.push(`${response.status()} ${response.url()}`)
  })

  await page.goto('fractional-flow.html')

  const root = page.getByTestId('fractional-flow-page')
  await expect(root).toBeVisible()

  // @ressim/analytical ran: Welge metrics for this rock/fluid pair are finite and in range.
  const shock = Number(await page.getByTestId('shock-sw').innerText())
  const pvi = Number(await page.getByTestId('breakthrough-pvi').innerText())
  expect(Number.isFinite(shock)).toBe(true)
  expect(shock).toBeGreaterThan(0.2) // above connate water
  expect(shock).toBeLessThan(0.8) // below 1 - residual oil
  expect(pvi).toBeGreaterThan(0)

  // @ressim/charts drew: a canvas exists and has been given real pixels.
  const canvas = page.locator('canvas').first()
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box?.width ?? 0).toBeGreaterThan(50)
  expect(box?.height ?? 0).toBeGreaterThan(50)

  // Tailwind emitted the packaged components' classes. `font-mono` resolving to a real monospace
  // stack is the check that the content glob still reached into `src/lib/<package>/` — a miss
  // here produces an unstyled page and no error at all.
  const fontFamily = await page
    .getByTestId('shock-sw')
    .evaluate((el) => getComputedStyle(el).fontFamily)
  expect(fontFamily.toLowerCase()).toContain('mono')

  // @ressim/primitives is wired: changing the control recomputes through the analytical package
  // and back out to the chart. A more viscous oil must move the shock front.
  await page.getByRole('button', { name: '20', exact: true }).click()
  await expect
    .poll(async () => Number(await page.getByTestId('shock-sw').innerText()))
    .not.toBe(shock)

  expect(runtimeErrors, `runtime errors on the second page:\n${runtimeErrors.join('\n')}`).toEqual([])
  expect(failedResponses, `failed requests:\n${failedResponses.join('\n')}`).toEqual([])
})
