/**
 * `scripts/canonical-repo.sh` decides which repository every `gh` call in the agent skills
 * targets. Getting it wrong is silent: in a fork clone `gh`'s default repository is the fork, so
 * a pull request number, an issue search or an alert survey quietly answers about the wrong
 * project. These tests pin the rule (upstream when it exists, otherwise origin), the URL forms
 * it accepts, and that every failure exits non-zero with nothing on stdout, so a
 * `repo=$(...) && gh ...` chain stops instead of running `gh -R ""`.
 *
 * Each case runs the real script in a throwaway repository; no network is used.
 */
import { describe, expect, it, beforeEach, afterEach } from 'vitest';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(new URL('./canonical-repo.sh', import.meta.url));

let repo: string;

function git(...args: string[]) {
    execFileSync('git', args, { cwd: repo, stdio: 'ignore' });
}

function run(...args: string[]) {
    const r = spawnSync('bash', [SCRIPT, ...args], { cwd: repo, encoding: 'utf8' });
    return { status: r.status, out: r.stdout, err: r.stderr };
}

beforeEach(() => {
    repo = mkdtempSync(path.join(tmpdir(), 'ressim-canonical-repo-'));
    git('init', '-q');
});

afterEach(() => {
    rmSync(repo, { recursive: true, force: true });
});

describe('canonical-repo.sh', () => {
    it('uses origin in a direct clone', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        expect(run()).toMatchObject({ status: 0, out: 'sergeyfarin/ressim\n' });
        expect(run('--remote')).toMatchObject({ status: 0, out: 'origin\n' });
    });

    it('prefers upstream in a fork clone, and still reports the fork by name', () => {
        git('remote', 'add', 'origin', 'git@github.com:Rekenraam-AI/ressim.git');
        git('remote', 'add', 'upstream', 'https://github.com/sergeyfarin/ressim');
        expect(run()).toMatchObject({ status: 0, out: 'sergeyfarin/ressim\n' });
        expect(run('--remote')).toMatchObject({ status: 0, out: 'upstream\n' });
        expect(run('origin')).toMatchObject({ status: 0, out: 'Rekenraam-AI/ressim\n' });
    });

    it.each([
        ['https://github.com/o/r.git', 'o/r'],
        ['https://github.com/o/r', 'o/r'],
        ['https://github.com/o/r/', 'o/r'],
        ['git@github.com:o/r.git', 'o/r'],
        ['ssh://git@github.com/o/r.git', 'o/r'],
    ])('parses %s', (url, expected) => {
        git('remote', 'add', 'origin', url);
        expect(run()).toMatchObject({ status: 0, out: `${expected}\n` });
    });

    it.each([
        ['no remote at all', null],
        ['a non-GitHub remote', 'https://gitlab.com/o/r.git'],
        ['a GitHub URL that is not a repository', 'https://github.com/o'],
        ['a local path', '/srv/git/ressim.git'],
    ])('fails with nothing on stdout for %s', (_label, url) => {
        if (url) git('remote', 'add', 'origin', url);
        const r = run();
        expect(r.status).not.toBe(0);
        expect(r.out).toBe('');
    });

    it('fails for a named remote that does not exist, and for bad arguments', () => {
        git('remote', 'add', 'origin', 'https://github.com/o/r.git');
        expect(run('nope')).toMatchObject({ status: 2, out: '' });
        expect(run('--bogus').status).toBe(2);
        expect(run('a', 'b').status).toBe(2);
    });

    it('stops a && chain before gh would run with an empty repository', () => {
        const r = spawnSync('bash', ['-c', `repo=$(bash "${SCRIPT}") && echo "gh -R $repo"`], {
            cwd: repo,
            encoding: 'utf8',
        });
        expect(r.status).not.toBe(0);
        expect(r.stdout).not.toContain('gh -R');
    });
});
