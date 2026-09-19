#!/usr/bin/env node
/**
 * Reports cross-area coupling under `src/`, as the progress metric for the module split
 * (`docs/ARCHITECTURE_SPLIT_PLAN_2026-09-19.md`).
 *
 * `check-import-cycles.mjs` answers "is there a runtime cycle *right now*", which is a
 * pass/fail gate. This answers a different question: "which areas could be lifted into their
 * own package, and what is still in the way". A pair of areas that import each other's
 * *values* cannot be split without one of them vendoring the other, whether or not those
 * imports currently form a cycle the gate catches.
 *
 * Type-only edges are counted separately and are not obstacles: they are erased at build time,
 * so two packages may depend on each other's types through a shared declaration without any
 * runtime relationship. Test files are excluded — a test may reach anywhere, and a test edge
 * does not constrain what the published package contains.
 *
 * Parsing (file walk, comment stripping, specifier resolution, type-only detection) is
 * deliberately identical to `check-import-cycles.mjs`, so the two never disagree about what
 * an edge is.
 *
 * Run: node scripts/measure-module-coupling.mjs
 * Exit code is always 0 — this measures, it does not gate.
 */
import fs from 'node:fs';
import path from 'node:path';

const ROOT = 'src';
const SKIP_DIR = /node_modules|[/\\]ressim[/\\](pkg|src|target)/;
const SOURCE_FILE = /\.(ts|svelte)$/;
const TEST_FILE = /\.(test|spec)\.[tj]s$/;

function collectFiles(dir, out = []) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (!SKIP_DIR.test(full)) collectFiles(full, out);
        } else if (SOURCE_FILE.test(entry.name) && !entry.name.endsWith('.d.ts') && !TEST_FILE.test(entry.name)) {
            out.push(full);
        }
    }
    return out;
}

/** Blank out comments so prose mentioning `import ... from '...'` is not parsed as an edge. */
function stripComments(source) {
    return source
        .replace(/\/\*[\s\S]*?\*\//g, (match) => match.replace(/[^\n]/g, ' '))
        .replace(/(^|[^:\\])\/\/[^\n]*/g, (match, lead) => lead + ' '.repeat(match.length - lead.length));
}

function resolveSpecifier(fromFile, specifier) {
    if (!specifier.startsWith('.')) return null;
    const base = path.normalize(path.join(path.dirname(fromFile), specifier));
    const candidates = [
        base,
        `${base}.ts`,
        `${base}.svelte`,
        `${base}.svelte.ts`,
        path.join(base, 'index.ts'),
        path.join(base, 'index.svelte.ts'),
    ];
    for (const candidate of candidates) {
        if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
    }
    return null;
}

/** True when the edge is erased at build time. */
function isTypeOnlyEdge(typeKeyword, clause) {
    if (typeKeyword) return true;
    const named = clause.match(/^\{([\s\S]*)\}$/);
    if (!named) return false;
    const specifiers = named[1].split(',').map((s) => s.trim()).filter(Boolean);
    return specifiers.length > 0 && specifiers.every((s) => /^type\s/.test(s));
}

/** The candidate package an file belongs to. Directories under src/lib are the unit. */
function areaOf(file) {
    const p = file.replace(/\\/g, '/');
    if (/^src\/lib\/ressim\//.test(p)) return 'ressim(wasm)';
    const m = p.match(/^src\/lib\/([^/]+)\//);
    if (m) return m[1];
    if (p.startsWith('src/lib/')) return 'lib-root';
    return 'app';
}

const files = collectFiles(ROOT);
const value = new Map();
const typeOnly = new Map();
const witnesses = new Map();
const bump = (map, key) => map.set(key, (map.get(key) ?? 0) + 1);

const pattern = /\b(?:import|export)\s+(type\s+)?([\s\S]*?)\s*from\s*['"]([^'"]+)['"]/g;
for (const file of files) {
    const from = areaOf(file);
    const source = stripComments(fs.readFileSync(file, 'utf8'));
    let match;
    pattern.lastIndex = 0;
    while ((match = pattern.exec(source))) {
        const target = resolveSpecifier(file, match[3]);
        if (!target) continue;
        const to = areaOf(target);
        if (to === from) continue;
        const key = `${from} -> ${to}`;
        if (isTypeOnlyEdge(match[1], match[2])) {
            bump(typeOnly, key);
        } else {
            bump(value, key);
            if (!witnesses.has(key)) witnesses.set(key, []);
            witnesses.get(key).push(`${file} -> ${path.relative(ROOT, target)}`);
        }
    }
}

const mutual = [];
for (const key of value.keys()) {
    const [from, to] = key.split(' -> ');
    if (value.has(`${to} -> ${from}`) && from < to) mutual.push([from, to]);
}

console.log('Cross-area coupling under src/ (production files only)\n');
console.log(`  value edges     ${[...value.values()].reduce((a, b) => a + b, 0)} across ${value.size} ordered pairs`);
console.log(`  type-only edges ${[...typeOnly.values()].reduce((a, b) => a + b, 0)} across ${typeOnly.size} ordered pairs  (not obstacles)`);
console.log(`\n  MUTUAL value pairs: ${mutual.length}   <-- the split metric; target is 0\n`);

for (const [a, b] of mutual.sort()) {
    const ab = value.get(`${a} -> ${b}`) ?? 0;
    const ba = value.get(`${b} -> ${a}`) ?? 0;
    console.log(`  ${a} <-> ${b}   (${a}->${b}: ${ab}, ${b}->${a}: ${ba})`);
    for (const w of [...(witnesses.get(`${a} -> ${b}`) ?? []), ...(witnesses.get(`${b} -> ${a}`) ?? [])]) {
        console.log(`      ${w}`);
    }
}
if (mutual.length === 0) console.log('  none — every area could be lifted into its own package.');
