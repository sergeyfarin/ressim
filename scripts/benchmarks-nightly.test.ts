import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const script = fileURLToPath(new URL('./benchmarks-nightly.sh', import.meta.url));
const canonicalHelper = fileURLToPath(new URL('./canonical-repo.sh', import.meta.url));
const canonical = 'sergeyfarin/ressim';
let fixture: string;

beforeEach(() => {
    fixture = mkdtempSync(path.join(tmpdir(), 'ressim-benchmarks-nightly-'));
    const git = (...args: string[]) => execFileSync('git', args, { cwd: fixture });
    git('init', '-q');
    git('remote', 'add', 'origin', 'https://github.com/fork/ressim.git');
    git('remote', 'add', 'upstream', 'https://dummy-token@github.com/sergeyfarin/ressim.git');
    mkdirSync(path.join(fixture, 'scripts'));
    writeFileSync(path.join(fixture, 'scripts/canonical-repo.sh'), readFileSync(canonicalHelper));

    const worktree = path.join(fixture, 'nightly/worktree');
    mkdirSync(path.join(worktree, 'scripts'), { recursive: true });
    writeFileSync(path.join(worktree, '.git'), '');
    writeFileSync(path.join(worktree, 'scripts/benchmarks.sh'), 'echo "benchmark check OK"\n');
    writeFileSync(path.join(fixture, 'gh-calls'), '');

    // The cron script puts system tools ahead of the caller's PATH. Bash functions
    // stub the commands before PATH lookup, while canonical-repo.sh still uses real Git.
    writeFileSync(path.join(fixture, 'stubs.sh'), `
git() {
    case "$3" in
        fetch|checkout|reset) return 0 ;;
        rev-parse) echo abc123; return 0 ;;
    esac
    command git "$@"
}
pnpm() { return 0; }
python3() { echo 'benchmark signals: 0 needing attention'; }
gh() {
    local action="$2" target=''
    shift 2
    while [ "$#" -gt 0 ]; do
        if [ "$1" = --repo ]; then
            target="$2"
            break
        fi
        shift
    done
    printf '%s %s\n' "$action" "$target" >> "$GH_CALLS"
    if [ "$action" = "$GH_FAIL" ]; then
        echo "stubbed gh $action failure" >&2
        return 7
    fi
    case "$action" in
        list) printf '%s\n' "$GH_MATCH" ;;
        view) printf '<!-- nightly-state: %s -->\n' "$GH_PREVIOUS" ;;
        edit|comment) return 0 ;;
        create) echo 'https://github.com/fork/ressim/issues/999' ;;
        pin) return 0 ;;
        *) return 1 ;;
    esac
}
`);
});

afterEach(() => {
    rmSync(fixture, { recursive: true, force: true });
});

function run({ fail = '', match = '56', previous = 'check=OK attention=0', dryRun = false } = {}) {
    const env: NodeJS.ProcessEnv = {
        ...process.env,
        RESSIM_REPO: fixture,
        RESSIM_NIGHTLY_HOME: path.join(fixture, 'nightly'),
        BASH_ENV: path.join(fixture, 'stubs.sh'),
        GH_CALLS: path.join(fixture, 'gh-calls'),
        GH_FAIL: fail,
        GH_MATCH: match,
        GH_PREVIOUS: previous,
    };
    delete env.NIGHTLY_DRY_RUN;
    if (dryRun) env.NIGHTLY_DRY_RUN = '1';
    const result = spawnSync('bash', [script], { env, encoding: 'utf8' });
    return {
        status: result.status,
        out: result.stdout,
        err: result.stderr,
        calls: readFileSync(path.join(fixture, 'gh-calls'), 'utf8').trim().split('\n').filter(Boolean),
    };
}

describe('benchmarks-nightly.sh', () => {
    it('exits non-zero and never creates or pins an issue when the lookup fails', () => {
        const result = run({ fail: 'list' });
        expect(result.err).toContain('stubbed gh list failure');
        expect(result.status).toBe(7);
        expect(result.calls).toEqual([`list ${canonical}`]);
    });

    it.each(['', '999', '56\n999'])('rejects an unexpected lookup result (%j)', (match) => {
        const result = run({ match });
        expect(result.status).toBe(1);
        expect(result.err).toContain('expected open issue #56');
        expect(result.calls).toEqual([`list ${canonical}`]);
    });

    it.each([
        ['check=OK attention=0', []],
        ['check=FAILED attention=0', [`comment ${canonical}`]],
    ])('updates the canonical repository and comments only on a state change (%s)', (previous, extraCalls) => {
        const result = run({ previous });
        expect(result.status).toBe(0);
        expect(result.calls).toEqual([
            `list ${canonical}`, `view ${canonical}`, `edit ${canonical}`, ...extraCalls,
        ]);
    });

    it.each([
        ['view', [`list ${canonical}`, `view ${canonical}`]],
        ['edit', [`list ${canonical}`, `view ${canonical}`, `edit ${canonical}`]],
        ['comment', [`list ${canonical}`, `view ${canonical}`, `edit ${canonical}`, `comment ${canonical}`]],
    ])('returns a failed %s without reporting a successful update', (fail, calls) => {
        const result = run({ fail, previous: 'check=FAILED attention=0' });
        expect(result).toMatchObject({ status: 7, out: '', calls });
        expect(result.err).toContain(`stubbed gh ${fail} failure`);
    });

    it('prints the issue body and skips GitHub in a dry run', () => {
        const result = run({ dryRun: true });
        expect(result).toMatchObject({ status: 0, calls: [] });
        expect(result.out).toContain('<!-- nightly-state: check=OK attention=0 -->');
        expect(result.out).toContain('benchmark check OK');
        expect(result.out).toContain('benchmark signals: 0 needing attention');
    });
});
