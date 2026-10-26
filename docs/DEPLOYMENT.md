# Deployment and rollback

The live production address remains `https://farin.nl/ressim/`. The prepared destination and
canonical/social metadata address is `https://ressim.farin.nl/`. This preparation does not switch
hosting. GitHub Pages at `https://sergeyfarin.github.io/ressim/` remains the latest validated
`master` canary, using the legacy subpath build.

## Build layouts

`RESIM_BASE_PATH` accepts exactly `/ressim/` (the default) or `/`. Set it consistently for build,
preview, packaging, and browser validation. Invalid values fail immediately. Outputs are separate
so switching options cannot leave assets or a redirect from the other layout in the candidate.

| Setting | Application output | Build metadata | Archive | Archive contents |
| --- | --- | --- | --- | --- |
| `/ressim/` or unset | `dist/ressim` | `dist/ressim/build-info.json` | `ressim-dist-<sha>.tar.gz` | `ressim/` directory |
| `/` | `dist-root` | `dist-root/build-info.json` | `ressim-root-dist-<sha>.tar.gz` | Application files at archive root |

The legacy build also writes `dist/index.html`, a redirect to `/ressim/`, outside its archive
(the existing parent-site archive contract). The root build serves the application directly from
`dist-root/index.html` and emits no redirect. Both include the standalone `fractional-flow.html`
page, bundled fonts, favicon, social preview, Worker and WASM assets under the selected base.

`build-info.json` records `commit`, `ref`, `basePath`, `builtAt` and the source commit URL.
`pnpm run package:dist` requires a full `RESIM_BUILD_SHA` (or `GITHUB_SHA`) and rejects a build whose
commit or base path differs. It copies the same metadata to `dist-build-meta.json` for legacy
archives or `dist-root-build-meta.json` for root archives. Different archive names prevent a root
candidate from replacing the legacy release asset for the same SHA.

## Local and CI validation before deployment

From a clean, committed revision, using Node 24 and pnpm:

```bash
export RESIM_BUILD_SHA=$(git rev-parse HEAD)
export RESIM_BUILD_REF=$(git branch --show-current)
export RESIM_EXPECTED_SHA=$RESIM_BUILD_SHA

# Existing layout; includes the full Vitest suite and Rust IMPES gate.
export RESIM_BASE_PATH=/ressim/
pnpm run validate:product
pnpm run package:dist
bash scripts/smoke-preview.sh

# Subdomain candidate; reuses the same numerical engine.
export RESIM_BASE_PATH=/
pnpm run build
pnpm run package:dist
bash scripts/smoke-preview.sh
```

`smoke-preview.sh` starts and stops a loopback Vite preview and runs the existing Playwright
suite. For a server started separately, use the same `RESIM_BASE_PATH` for `pnpm run preview`
and `pnpm run test:deployed`. `RESIM_PUBLIC_URL` overrides the browser suite's address when
checking an actual host; the preview wrapper always tests localhost.
If port 4173 is occupied, set `RESIM_PREVIEW_PORT` for the wrapper; it refuses to test an existing
server in place of the candidate.

`.github/workflows/pr-tests.yml` runs on pull requests or manual dispatch and checks both layouts
without deploying. Existing solver, Buckley-Leverett, compositional thermodynamics, native binding
parity, benchmark and OPM artifact gates remain in place. Each bundle is packaged with an exact
SHA check and browser-tested for direct load/refresh, metadata and local social-image assets,
Worker/WASM, IMPES/FIM, compositional runs, charts, styling, 3D and cookie-free operation.
The browser checks compare `build-info.json` with `RESIM_EXPECTED_SHA` and the requested base path.

The application sets no cookies, uses no analytics and serves fonts locally. Hosting must preserve
that behavior; smoke-test the final HTTPS address as well as the local candidate. Numerical models,
dependencies and numerical acceptance tolerances are unchanged by the layout option.

## Current release flow

Every push to `master` runs `.github/workflows/publish-dist.yml`:

1. Run `pnpm run validate:product` with the explicit legacy base `/ressim/`.
2. Verify the embedded revision/base and package the SHA-addressed legacy archive.
3. Deploy `dist/ressim` to GitHub Pages.
4. Smoke-test the deployed Pages URL against the exact workflow SHA.
5. Publish the tested archive and metadata as release `dist-<full-commit-sha>`.

There is no promotion or repository dispatch to `rekenraam-web`. Publishing a ResSim release does
not advance that repository's submodule or switch production hosting.

## Remaining cutover steps — only after explicit authorization

1. Choose the release SHA and run the full PR/manual validation workflow, including the root
   browser gate. Keep the known-good legacy release available for rollback.
2. Switch the hosting workflow to `RESIM_BASE_PATH=/`. The existing release workflow takes its
   Pages directory, archive and metadata paths from the verified packaging step, so this selects
   `dist-root`, `ressim-root-dist-<sha>.tar.gz` and `dist-root-build-meta.json` together. Keep
   exact-SHA verification, numerical gates and deployed smoke checks. Do not restore parent-site
   promotion jobs.
3. Configure the chosen host's custom domain, DNS and TLS for `ressim.farin.nl`. These settings
   are intentionally outside the preparation change. Do not add a `CNAME` file to the legacy
   Pages canary during preparation.
4. Serve the root artifact and run `RESIM_PUBLIC_URL=https://ressim.farin.nl/` with
   `RESIM_EXPECTED_SHA=<full-sha>` through `pnpm run test:deployed`. Confirm `/build-info.json`
   matches, assets/Worker/WASM and the second page load, and hosting introduces no cookies.
5. Redirect the old production `/ressim/` URLs to the new host, preserving asset and second-page
   paths as needed. Update the README live link and this document's production status after the
   final host passes. Check canonical/social previews at the new address.

## Rollback

For a root-host regression, redeploy the last known-good **root** archive with its matching
metadata, verify `/build-info.json` and run the exact-SHA smoke suite. A legacy archive requires
legacy hosting at `/ressim/`; it cannot be unpacked as a root application without rebuilding.
Keep the old production path available until the root cutover has passed validation.

To restore a known-good Pages canary, manually run the release workflow from that commit/tag with
the matching layout. Record the exceptional deployment in issue #8: the next successful `master`
run replaces it. Any rollback of the separately hosted parent site remains a separate, explicitly
authorized operation in its owning repository.
