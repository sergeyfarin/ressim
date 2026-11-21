import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const script = fileURLToPath(new URL('./benchmarks-nightly.sh', import.meta.url));
const canonicalHelper = fileURLToPath(new URL('./canonical-repo.sh', import.meta.url));
const canonical = 'sergeyfarin/ressim';
const issueTitle = 'Benchmark signals (nightly)';
const view = `view 56 ${canonical}`;
const edit = `edit 56 ${canonical}`;
const comment = `comment 56 ${canonical}`;
let fixture: string;

function git(...args: string[]) {
    execFileSync('git', args, { cwd: fixture });
}

beforeEach(() => {
    fixture = mkdtempSync(path.join(tmpdir(), 'ressim-benchmarks-nightly-'));
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
    writeFileSync(path.join(fixture, 'git-calls'), '');
    writeFileSync(path.join(fixture, 'issue-body'), '<!-- nightly-state: check=OK attention=0 -->\n');

    // The cron script puts system tools ahead of the caller's PATH. Bash functions
    // stub the commands before PATH lookup, while canonical-repo.sh still uses real Git.
    writeFileSync(path.join(fixture, 'stubs.sh'), `
git() {
    case "$3" in
        fetch|checkout|reset) printf '%s\n' "\${*:3}" >> "$GIT_CALLS"; return 0 ;;
        rev-parse) echo abc123; return 0 ;;
    esac
    command git "$@"
}
pnpm() { return 0; }
python3() { echo 'benchmark signals: 0 needing attention'; }
gh() {
    local action="$2" number="$3" target='' body='' fields=''
    shift 2
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --repo) target="$2"; shift 2 ;;
            --body) body="$2"; shift 2 ;;
            --json) fields="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    printf '%s %s %s\n' "$action" "$number" "$target" >> "$GH_CALLS"
    if [ "$action" = "$GH_FAIL" ]; then
        echo "stubbed gh $action failure" >&2
        return 7
    fi
    case "$action" in
        list) echo 56 ;;
        view)
            if [ "$fields" = title,state,body ]; then
                printf '%s\n%s\n' "$GH_STATE" "$GH_TITLE"
            fi
            cat "$GH_ISSUE_BODY"
            ;;
        edit) printf '%s\n' "$body" > "$GH_ISSUE_BODY" ;;
        comment) return 0 ;;
        *) return 1 ;;
    esac
}
`);
});

afterEach(() => {
    rmSync(fixture, { recursive: true, force: true });
});

function run({ fail = '', title = issueTitle, state = 'OPEN', previous, dryRun = false }: {
    fail?: string;
    title?: string;
    state?: string;
    previous?: string;
    dryRun?: boolean;
} = {}) {
    const issueBody = path.join(fixture, 'issue-body');
    if (previous !== undefined) writeFileSync(issueBody, `<!-- nightly-state: ${previous} -->\n`);
    const ghCalls = path.join(fixture, 'gh-calls');
    const gitCalls = path.join(fixture, 'git-calls');
    writeFileSync(ghCalls, '');
    writeFileSync(gitCalls, '');
    const env: NodeJS.ProcessEnv = {
        ...process.env,
        RESSIM_REPO: fixture,
        RESSIM_NIGHTLY_HOME: path.join(fixture, 'nightly'),
        BASH_ENV: path.join(fixture, 'stubs.sh'),
        GH_CALLS: ghCalls,
        GIT_CALLS: gitCalls,
        GH_FAIL: fail,
        GH_TITLE: title,
        GH_STATE: state,
        GH_ISSUE_BODY: issueBody,
    };
    delete env.NIGHTLY_DRY_RUN;
    if (dryRun) env.NIGHTLY_DRY_RUN = '1';
    const result = spawnSync('bash', [script], { env, encoding: 'utf8' });
    return {
        status: result.status,
        out: result.stdout,
        err: result.stderr,
        calls: readFileSync(ghCalls, 'utf8').trim().split('\n').filter(Boolean),
        gitCalls: readFileSync(gitCalls, 'utf8').trim().split('\n').filter(Boolean),
        issueBody: readFileSync(issueBody, 'utf8'),
    };
}

describe('benchmarks-nightly.sh', () => {
    it('exits non-zero and never creates or pins an issue when the lookup fails', () => {
        const result = run({ fail: 'view' });
        expect(result.err).toContain('stubbed gh view failure');
        expect(result.status).toBe(7);
        expect(result.calls).toEqual([view]);
    });

    it.each([
        { state: 'CLOSED' },
        { title: 'Renamed nightly issue' },
    ])('rejects a closed or renamed issue (%j)', (details) => {
        const result = run(details);
        expect(result.status).toBe(1);
        expect(result.err).toContain('expected open issue #56');
        expect(result.calls).toEqual([view]);
    });

    it('measures upstream/master in a fork and publishes to the same repository', () => {
        const result = run();
        expect(result.status).toBe(0);
        expect(result.gitCalls).toEqual([
            'fetch --quiet upstream +refs/heads/master:refs/remotes/upstream/master',
            'checkout --quiet --detach refs/remotes/upstream/master',
            'reset --quiet --hard refs/remotes/upstream/master',
        ]);
        expect(result.calls).toEqual([view, edit]);
        expect(result.issueBody).toContain('on `upstream/master`');
    });

    it('stops before any GitHub call when the canonical remote is outside GitHub', () => {
        git('remote', 'set-url', 'upstream', 'https://example.invalid/ressim.git');
        const result = run();
        expect(result).toMatchObject({ status: 1, calls: [], gitCalls: [] });
        expect(result.err).toContain('canonical-repo:');
    });

    it('retries a failed notification on the next run and then reports the state as unchanged', () => {
        const failed = run({ fail: 'comment', previous: 'check=FAILED attention=0' });
        expect(failed).toMatchObject({ status: 7, out: '', calls: [view, comment] });
        expect(failed.err).toContain('stubbed gh comment failure');
        expect(failed.issueBody).toContain('<!-- nightly-state: check=FAILED attention=0 -->');

        const retry = run();
        expect(retry).toMatchObject({ status: 0, calls: [view, comment, edit] });
        expect(retry.issueBody).toContain('<!-- nightly-state: check=OK attention=0 -->');
        expect(run()).toMatchObject({ status: 0, calls: [view, edit] });
    });

    it('keeps the previous state if the edit fails after a notification', () => {
        const result = run({ fail: 'edit', previous: 'check=FAILED attention=0' });
        expect(result).toMatchObject({ status: 7, out: '', calls: [view, comment, edit] });
        expect(result.err).toContain('stubbed gh edit failure');
        expect(result.issueBody).toContain('<!-- nightly-state: check=FAILED attention=0 -->');
    });

    it('prints the issue body and skips GitHub in a dry run', () => {
        git('remote', 'remove', 'upstream');
        const result = run({ dryRun: true });
        expect(result).toMatchObject({ status: 0, calls: [] });
        expect(result.gitCalls[0]).toBe('fetch --quiet origin +refs/heads/master:refs/remotes/origin/master');
        expect(result.out).toContain('on `origin/master`');
        expect(result.out).toContain('<!-- nightly-state: check=OK attention=0 -->');
        expect(result.out).toContain('benchmark check OK');
        expect(result.out).toContain('benchmark signals: 0 needing attention');
    });
});
