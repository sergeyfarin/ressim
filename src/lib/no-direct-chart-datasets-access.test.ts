import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';

// Disallow direct indexing / iteration over `chart.data.datasets` in source files
const repoRoot = path.resolve(__dirname, '..', '..');

function listSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const name of fs.readdirSync(dir)) {
    const full = path.join(dir, name);
    const stat = fs.statSync(full);
    if (stat.isDirectory()) {
      if (name === 'node_modules' || name === 'dist' || name === 'pkg' || name === 'target') continue;
      out.push(...listSourceFiles(full));
    } else if (stat.isFile()) {
      if (/\.(ts|js|svelte)$/.test(full) && !/\.test\.(ts|js)$/.test(full)) {
        out.push(full);
      }
    }
  }
  return out;
}

describe('code-style: chart.data.datasets access', () => {
  it('only chart-helpers should index/iterate datasets directly', () => {
    const files = listSourceFiles(path.join(repoRoot, 'src'))
      .filter((f) => !/node_modules/.test(f));

    const badPatterns = [
      /for\s*\(const\s+dataset\s+of\s+chart\.data\.datasets\s*\)/g, // for-of iteration
      /chart\.data\.datasets\s*\.forEach\s*\(/g, // forEach
      /chart\.data\.datasets\s*\[/g, // direct bracket access
    ];

    const violations: string[] = [];

    for (const file of files) {
      // allowlist: chart-helpers (it is allowed to access datasets), and test files
      if (/chart-helpers\.ts$/.test(file)) continue;
      if (/\.test\.(ts|js)$/.test(file)) continue;

      const src = fs.readFileSync(file, 'utf8');
      for (const pat of badPatterns) {
        const m = src.match(pat);
        if (m && m.length > 0) {
          violations.push(`${path.relative(repoRoot, file)} -> ${pat}`);
        }
      }
    }

    if (violations.length > 0) {
      // make failure message actionable
      const message = ['Found direct `chart.data.datasets` uses (disallowed):', ...violations].join('\n');
      throw new Error(message);
    }

    expect(violations).toHaveLength(0);
  });
});
