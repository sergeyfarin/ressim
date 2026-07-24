#!/usr/bin/env node
/**
 * Fails on runtime (value-level) import cycles under src/.
 *
 * A cycle is not itself a bug, but a cycle whose modules initialize top-level
 * `const`s from each other is: whichever module the bundler enters first sees
 * the other's bindings in their temporal dead zone and throws
 * "Cannot access 'X' before initialization" at load, blanking the app. That is
 * exactly what caseLibrary <-> caseCatalog did.
 *
 * Type-only edges (`import type`, and named blocks where every specifier is
 * `type X`) are erased at build time and cannot cause TDZ errors, so they are
 * not reported. Breaking a cycle usually means making the edge type-only or
 * moving the shared value into a leaf module.
 *
 * Run: node scripts/check-import-cycles.mjs
 */
import fs from 'node:fs';
import path from 'node:path';

const ROOT = 'src';
const SKIP_DIR = /node_modules|[/\\]ressim[/\\](pkg|src|target)/;
const SOURCE_FILE = /\.(ts|svelte)$/;

function collectFiles(dir, out = []) {
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

/** True when the edge is erased at build time and so cannot cause a TDZ error. */
function isTypeOnlyEdge(typeKeyword, clause) {
    if (typeKeyword) return true;
    const named = clause.match(/^\{([\s\S]*)\}$/);
    if (!named) return false;
    const specifiers = named[1].split(',').map((s) => s.trim()).filter(Boolean);
    return specifiers.length > 0 && specifiers.every((s) => /^type\s/.test(s));
}

function buildGraph(files) {
    const graph = new Map();
    // Static `import`/`export ... from`. Dynamic import() is deliberately excluded:
    // it defers evaluation, so it breaks rather than forms an initialization cycle.
    const pattern = /\b(?:import|export)\s+(type\s+)?([\s\S]*?)\s*from\s*['"]([^'"]+)['"]/g;
    for (const file of files) {
        const source = stripComments(fs.readFileSync(file, 'utf8'));
        const edges = new Set();
        let match;
        while ((match = pattern.exec(source))) {
            if (isTypeOnlyEdge(match[1], match[2])) continue;
            const target = resolveSpecifier(file, match[3]);
            if (target) edges.add(target);
        }
        graph.set(file, [...edges]);
    }
    return graph;
}

function findCycles(graph) {
    const cycles = [];
    const state = new Map();
    const stack = [];
    function visit(node) {
        state.set(node, 'open');
        stack.push(node);
        for (const target of graph.get(node) ?? []) {
            if (state.get(target) === 'open') {
                cycles.push([...stack.slice(stack.indexOf(target)), target]);
            } else if (!state.has(target)) {
                visit(target);
            }
        }
        stack.pop();
        state.set(node, 'done');
    }
    for (const file of graph.keys()) if (!state.has(file)) visit(file);

    const unique = new Map();
    for (const cycle of cycles) {
        const key = [...new Set(cycle)].sort().join('|');
        if (!unique.has(key)) unique.set(key, cycle);
    }
    return [...unique.values()];
}

const files = collectFiles(ROOT);
const cycles = findCycles(buildGraph(files));

if (cycles.length === 0) {
    console.log(`No runtime import cycles across ${files.length} files.`);
    process.exit(0);
}

console.error(`Found ${cycles.length} runtime import cycle(s) in ${files.length} files:\n`);
for (const cycle of cycles) {
    console.error(`  ${cycle.join('\n    -> ')}\n`);
}
console.error(
    'A value-level cycle risks "Cannot access X before initialization" at load.\n' +
    'Fix by making the edge type-only, or by moving the shared value to a leaf module.',
);
process.exit(1);
