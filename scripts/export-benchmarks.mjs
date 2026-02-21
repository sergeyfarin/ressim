import { execSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const rootDir = process.cwd();
const rustDir = resolve(rootDir, 'src/lib/ressim');
const outputDir = resolve(rootDir, 'public');
const outputFile = resolve(outputDir, 'benchmark-results.json');

const command = 'cargo test benchmark_buckley_leverett --release -- --nocapture';
const tolerances = {
  'BL-Case-A': 0.25,
  'BL-Case-B': 0.30,
  'BL-Case-A-Refined': 0.25,
  'BL-Case-B-Refined': 0.30,
};

const expectedNames = [
  'BL-Case-A',
  'BL-Case-B',
  'BL-Case-A-Refined',
  'BL-Case-B-Refined',
];

function runCommand() {
  try {
    return execSync(command, {
      cwd: rustDir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (error) {
    const stdout = typeof error?.stdout === 'string' ? error.stdout : '';
    const stderr = typeof error?.stderr === 'string' ? error.stderr : '';
    const combined = `${stdout}\n${stderr}`.trim();
    throw new Error(`Benchmark command failed. Output:\n${combined}`);
  }
}

function parseCases(testOutput) {
  const pattern = /^(BL-Case-[A-Z](?:-Refined)?): breakthrough_pv_sim=([0-9]*\.?[0-9]+), breakthrough_pv_ref=([0-9]*\.?[0-9]+), rel_err=([0-9]*\.?[0-9]+)/gm;
  const cases = [];
  let match;

  while ((match = pattern.exec(testOutput)) !== null) {
    const name = match[1];
    const pvBtSim = Number(match[2]);
    const pvBtRef = Number(match[3]);
    const relError = Number(match[4]);
    const tolerance = tolerances[name];

    if (!Number.isFinite(pvBtSim) || !Number.isFinite(pvBtRef) || !Number.isFinite(relError)) {
      continue;
    }

    cases.push({
      name,
      pvBtSim,
      pvBtRef,
      relError,
      tolerance,
      passes: relError <= tolerance,
    });
  }

  return cases;
}

function groupCasesByMode(cases) {
  const baseline = [];
  const refined = [];

  for (const row of cases) {
    if (row.name.endsWith('-Refined')) {
      refined.push(row);
    } else {
      baseline.push(row);
    }
  }

  const sortCaseNames = (left, right) => left.name.localeCompare(right.name);
  baseline.sort(sortCaseNames);
  refined.sort(sortCaseNames);

  return {
    baseline,
    refined,
  };
}

function main() {
  const testOutput = runCommand();
  const cases = parseCases(testOutput);

  if (cases.length === 0) {
    throw new Error('No benchmark cases were parsed from cargo test output.');
  }

  const missing = expectedNames.filter((name) => !cases.find((item) => item.name === name));
  if (missing.length > 0) {
    throw new Error(`Missing expected benchmark case(s): ${missing.join(', ')}`);
  }

  const modes = groupCasesByMode(cases);

  const artifact = {
    generatedAt: new Date().toISOString(),
    source: 'cargo-test',
    command,
    defaultMode: 'baseline',
    modes,
    cases: modes.baseline,
  };

  mkdirSync(outputDir, { recursive: true });
  writeFileSync(outputFile, `${JSON.stringify(artifact, null, 2)}\n`, 'utf8');

  console.log(`Wrote benchmark artifact: ${outputFile}`);
}

main();
