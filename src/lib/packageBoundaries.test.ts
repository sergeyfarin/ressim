/**
 * A workspace package must be entered by its name, not tunnelled into by relative path.
 *
 * Declaring `src/lib/charts` as `@ressim/charts` creates a name and a workspace link. It does
 * **not** create a boundary: `../charts/buildChartData` still resolves, still bundles, and still
 * passes every other gate. Without this test the packages are a naming convention that decays the
 * first time someone types a relative path out of habit, and the decay is invisible — nothing
 * fails.
 *
 * The rule is only about *entering* a package from outside. Inside a package, relative imports
 * are correct and are left alone; a package that imported itself by name would be the odd one.
 *
 * Discovered by walking for `package.json`, so a package added later is covered without editing
 * this file. Test files are included deliberately: a test that reaches around the boundary
 * documents an entry point the package does not actually offer.
 */
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const LIB = path.join('src', 'lib');
const SOURCE_FILE = /\.(ts|svelte)$/;
const SKIP_DIR = /node_modules|[/\\]ressim[/\\](pkg|src|target)/;

/** Workspace packages, by directory name, as declared on disk. */
function findPackages(): string[] {
    return fs
        .readdirSync(LIB, { withFileTypes: true })
        .filter((e) => e.isDirectory() && fs.existsSync(path.join(LIB, e.name, 'package.json')))
        .map((e) => e.name);
}

function collectFiles(dir: string, out: string[] = []): string[] {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (!SKIP_DIR.test(full)) collectFiles(full, out);
        } else if (SOURCE_FILE.test(entry.name) && !entry.name.endsWith('.d.ts')) {
            out.push(full);
        }
    }
    return out;
}

/** Blank out comments so prose naming a path is not read as an import. */
function stripComments(source: string): string {
    return source
        .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
        .replace(/(^|[^:\\])\/\/[^\n]*/g, (m, lead: string) => lead + ' '.repeat(m.length - lead.length));
}

describe('workspace package boundaries', () => {
    const packages = findPackages();

    it('finds the declared packages, so this test cannot pass by finding none', () => {
        expect(packages.length).toBeGreaterThan(0);
    });

    it('is entered by package name, never by a relative path from outside', () => {
        const violations: string[] = [];
        const importPattern = /\b(?:import|export)\s+(?:type\s+)?[\s\S]*?from\s*['"]([^'"]+)['"]/g;

        for (const file of collectFiles('src')) {
            const owner = file.replace(/\\/g, '/').match(/^src\/lib\/([^/]+)\//)?.[1];
            const source = stripComments(fs.readFileSync(file, 'utf8'));
            importPattern.lastIndex = 0;
            let match: RegExpExecArray | null;
            while ((match = importPattern.exec(source))) {
                const spec = match[1];
                if (!spec.startsWith('.')) continue;
                const target = path
                    .normalize(path.join(path.dirname(file), spec))
                    .replace(/\\/g, '/');
                const entered = target.match(/^src\/lib\/([^/]+)\//)?.[1];
                if (!entered || entered === owner) continue;
                if (packages.includes(entered)) {
                    violations.push(`${file}\n      imports '${spec}'  -> use '@ressim/${entered}/…'`);
                }
            }
        }

        expect(
            violations,
            `relative imports tunnelling into a workspace package:\n    ${violations.join('\n    ')}`,
        ).toEqual([]);
    });
});
