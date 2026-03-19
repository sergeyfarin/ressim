import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

const scenarioPickerPath = path.join(__dirname, 'modes', 'ScenarioPicker.svelte');
const scenarioPickerSource = fs.readFileSync(scenarioPickerPath, 'utf8');

describe('scenario picker flows', () => {
  it('keeps validation warnings scoped to the scenario picker warning surface', () => {
    expect(scenarioPickerSource).toMatch(/<WarningPolicyPanel/);
    expect(scenarioPickerSource).toMatch(/groups=\{\["blockingValidation", "nonPhysical", "advisory"\]\}/);
    expect(scenarioPickerSource).toMatch(/blockingValidation: \["validation"\]/);
    expect(scenarioPickerSource).toMatch(/nonPhysical: \["validation"\]/);
    expect(scenarioPickerSource).toMatch(/advisory: \["validation"\]/);
  });

  it('shows scenario description and customize shortcut when a preset scenario is active', () => {
    expect(scenarioPickerSource).toMatch(/activeScenario\.description/);
    expect(scenarioPickerSource).toMatch(/Customize/);
    expect(scenarioPickerSource).toMatch(/onEnterCustomMode/);
  });

  it('shows sensitivity variant chips when the scenario has a sensitivity axis', () => {
    expect(scenarioPickerSource).toMatch(/activeDimension\.variants/);
    expect(scenarioPickerSource).toMatch(/onToggleVariant/);
    expect(scenarioPickerSource).toMatch(/ui-chip/);
    expect(scenarioPickerSource).toMatch(/validActiveVariantKeys\.includes/);
  });
});
