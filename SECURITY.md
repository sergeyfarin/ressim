# Security policy

## Reporting a vulnerability

Please do not open a public issue, pull request or discussion for a suspected vulnerability.

Report it privately through GitHub's private vulnerability reporting:
<https://github.com/sergeyfarin/ressim/security/advisories/new>
(the **Report a vulnerability** button on the repository's *Security* tab).

Include what you found, how to reproduce it, and the revision you tested. The report stays
private between you and the maintainer until a fix is available.

## Scope

ResSim runs entirely in the browser: the simulation engine is WebAssembly, the deployed site
sets no cookies, uses no analytics and has no server-side component or user accounts. Reports
about the build and release pipeline, the published site, and the dependencies it ships are in
scope. Scientific limits of the models are not security issues; they are described in
`docs/SCIENTIFIC_LIMITATIONS.md`.

## For contributors and coding agents

A suspected vulnerability is never described in a public issue, pull request, commit message or
comment before it is fixed. Advisories GitHub has already published (Dependabot alerts) may be
tracked publicly by advisory ID and dependency only; whether the vulnerable code is reachable
here is discussed privately.
