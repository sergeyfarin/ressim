#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import process from 'node:process';

const ROOT = resolve(dirname(new URL(import.meta.url).pathname), '..');

function usage() {
  console.error('Usage: node scripts/opm-reference-fixture-check.mjs --case <case-key> [--infostep <path>]');
}

function parseArgs(argv) {
  const args = { caseKey: null, infostep: null };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--case') {
      args.caseKey = argv[index + 1] ?? null;
      index += 1;
    } else if (arg === '--infostep') {
      args.infostep = argv[index + 1] ?? null;
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (!args.caseKey) throw new Error('--case is required');
  return args;
}

function parseInfoStep(text) {
  const rows = [];
  for (const line of text.split(/\r?\n/)) {
    const fields = line.trim().split(/\s+/);
    if (fields.length !== 13 || !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(fields[0])) {
      continue;
    }
    const newton = Number(fields[10]);
    const converged = Number(fields[12]);
    if (Number.isInteger(newton) && Number.isInteger(converged)) {
      rows.push({ newton, converged });
    }
  }
  return rows;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const caseDir = resolve(ROOT, 'opm/reference-decks', args.caseKey);
  const manifest = JSON.parse(await readFile(resolve(caseDir, 'manifest.json'), 'utf8'));
  if (manifest.schemaVersion !== 1 || manifest.caseKey !== args.caseKey) {
    throw new Error(`Invalid manifest for ${args.caseKey}`);
  }

  const deckPath = resolve(caseDir, manifest.deck.path);
  const deck = await readFile(deckPath, 'utf8');
  const actualHash = createHash('sha256').update(deck).digest('hex');
  if (actualHash !== manifest.deck.sha256) {
    throw new Error(`Deck checksum mismatch: expected ${manifest.deck.sha256}, got ${actualHash}`);
  }
  for (const invariant of manifest.deckInvariants) {
    if (!deck.includes(invariant)) {
      throw new Error(`Deck is missing required mapped input: ${JSON.stringify(invariant)}`);
    }
  }

  if (args.infostep) {
    const rows = parseInfoStep(await readFile(resolve(args.infostep), 'utf8'));
    const expected = manifest.opm.expected;
    const actualIterations = rows.map((row) => row.newton);
    if (rows.length !== expected.substeps) {
      throw new Error(`Expected ${expected.substeps} OPM substeps, got ${rows.length}`);
    }
    if (JSON.stringify(actualIterations) !== JSON.stringify(expected.newtonIterations)) {
      throw new Error(`OPM Newton iterations differ: expected ${expected.newtonIterations}, got ${actualIterations}`);
    }
    if (rows.some((row) => row.converged !== 1)) {
      throw new Error('OPM reported a non-converged accepted substep');
    }
  }

  console.log(`OPM fixture verified: ${args.caseKey}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  usage();
  process.exitCode = 1;
});
