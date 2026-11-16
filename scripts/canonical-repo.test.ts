/**
 * `scripts/canonical-repo.sh` decides which repository every `gh` call in the agent skills
 * targets. Getting it wrong is silent: in a fork clone `gh`'s default repository is the fork, so
 * a pull request number, an issue search or an alert survey quietly answers about the wrong
 * project. These tests pin the rule (upstream when it exists, otherwise origin), the URL forms
 * it accepts, that `--push` follows where `git push` actually writes, that every failure exits
 * non-zero with nothing on stdout, so a `repo=$(...) && gh ...` chain stops instead of running
 * `gh -R ""`, and that no output ever repeats a credential carried in a remote URL.
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

const SECRET = 'DUMMYSECRET123';

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
        ['ssh://git@github.com:22/o/r.git', 'o/r'],
        ['ssh://git@ssh.github.com:443/o/r.git', 'o/r'],
        ['ssh://git@ssh.github.com/o/r', 'o/r'],
        [`https://x-access-token:${SECRET}@github.com/o/r.git`, 'o/r'],
        [`https://${SECRET}@github.com/o/r`, 'o/r'],
    ])('parses %s', (url, expected) => {
        git('remote', 'add', 'origin', url);
        const r = run();
        expect(r).toMatchObject({ status: 0, out: `${expected}\n`, err: '' });
    });

    it.each([
        ['no remote at all', null],
        ['a non-GitHub remote', 'https://gitlab.com/o/r.git'],
        ['a GitHub URL that is not a repository', 'https://github.com/o'],
        ['a local path', '/srv/git/ressim.git'],
        ['a lookalike host', 'https://github.com.evil.example/o/r.git'],
        ['a credential on a non-GitHub host', `https://x-access-token:${SECRET}@gitlab.com/o/r.git`],
        ['a credential on a GitHub URL that is not a repository', `https://u:${SECRET}@github.com/o`],
    ])('fails with nothing on stdout for %s, and never echoes a credential', (_label, url) => {
        if (url) git('remote', 'add', 'origin', url);
        const r = run();
        expect(r.status).not.toBe(0);
        expect(r.out).toBe('');
        expect(r.err).not.toContain(SECRET);
        if (url) expect(r.err).not.toContain(url);
    });

    it('--push reports where git push writes, not where it fetches from', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        expect(run('--push', 'origin')).toMatchObject({ status: 0, out: 'sergeyfarin/ressim\n' });
        git('remote', 'set-url', '--push', 'origin', 'git@github.com:Rekenraam-AI/ressim.git');
        expect(run('origin')).toMatchObject({ status: 0, out: 'sergeyfarin/ressim\n' });
        expect(run('--push', 'origin')).toMatchObject({ status: 0, out: 'Rekenraam-AI/ressim\n' });
    });

    it('--push fails when git push would write to more than one repository', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', 'https://github.com/Rekenraam-AI/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', `https://u:${SECRET}@mirror.example/r.git`);
        const r = run('--push', 'origin');
        expect(r).toMatchObject({ status: 1, out: '' });
        expect(r.err).toContain('push URL 2');
        expect(r.err).not.toContain(SECRET);
    });

    it('--push fails for two GitHub push destinations, and for two plain URLs', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', 'https://github.com/Rekenraam-AI/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', 'https://github.com/sergeyfarin/other.git');
        expect(run('--push', 'origin')).toMatchObject({ status: 1, out: '' });
        expect(run('--push', 'origin').err).toContain('pushes to 2 repositories');
        git('remote', 'remove', 'origin');
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('remote', 'set-url', '--add', 'origin', 'https://github.com/Rekenraam-AI/ressim.git');
        expect(run('--push', 'origin')).toMatchObject({ status: 1, out: '' });
        // `git fetch` uses the first URL only, so the fetch lookup still answers.
        expect(run('origin')).toMatchObject({ status: 0, out: 'sergeyfarin/ressim\n' });
    });

    it('--push accepts two push URLs that name the same repository', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', 'https://github.com/Rekenraam-AI/ressim.git');
        git('remote', 'set-url', '--add', '--push', 'origin', 'git@github.com:Rekenraam-AI/ressim.git');
        expect(run('--push', 'origin')).toMatchObject({ status: 0, out: 'Rekenraam-AI/ressim\n', err: '' });
    });

    it('--push honours pushInsteadOf', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('config', 'url.git@github.com:Rekenraam-AI/.pushInsteadOf', 'https://github.com/sergeyfarin/');
        expect(run('--push', 'origin')).toMatchObject({ status: 0, out: 'Rekenraam-AI/ressim\n' });
    });

    it('--push fails without echoing a credential when the push URL is not GitHub', () => {
        git('remote', 'add', 'origin', 'https://github.com/sergeyfarin/ressim.git');
        git('remote', 'set-url', '--push', 'origin', `https://u:${SECRET}@mirror.example/ressim.git`);
        const r = run('--push', 'origin');
        expect(r.status).toBe(1);
        expect(r.out).toBe('');
        expect(r.err).toContain('push URL');
        expect(r.err).not.toContain(SECRET);
    });

    it('fails for a named remote that does not exist, and for bad arguments', () => {
        git('remote', 'add', 'origin', 'https://github.com/o/r.git');
        expect(run('nope')).toMatchObject({ status: 2, out: '' });
        expect(run('--bogus').status).toBe(2);
        expect(run('a', 'b').status).toBe(2);
        expect(run('--push').status).toBe(2);
        expect(run('--push', '--remote').status).toBe(2);
        expect(run('--push', 'nope')).toMatchObject({ status: 2, out: '' });
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
