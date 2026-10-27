# Deployment and rollback

The live application is [ResSim](https://ressim.farin.nl/), served at the root of its custom domain
by GitHub Pages. The canonical and social metadata use the same address. The release workflow
builds, deploys and smoke-tests the root layout; the legacy `/ressim/` build remains available
for separately managed hosting at the old path.

## Build layouts

`RESIM_BASE_PATH` accepts exactly `/ressim/` or `/`. Local commands still default to `/ressim/`
when it is unset; the production workflow explicitly selects `/`. Set it consistently for build,
preview, packaging, and browser validation. Invalid values fail immediately. Outputs are separate
so switching options cannot leave assets or a redirect from the other layout in the build.

| Setting | Application output | Build metadata | Archive | Archive contents |
| --- | --- | --- | --- | --- |
| `/ressim/` or unset | `dist/ressim` | `dist/ressim/build-info.json` | `ressim-dist-<sha>.tar.gz` | `ressim/` directory |
| `/` | `dist-root` | `dist-root/build-info.json` | `ressim-root-dist-<sha>.tar.gz` | Application files at archive root |

The legacy build also writes `dist/index.html`, a redirect to `/ressim/`, outside its archive
(the existing parent-site archive contract). The root build serves the application directly from
`dist-root/index.html` and emits no redirect. Both include the standalone `fractional-flow.html`
page, bundled fonts, favicon, social preview, Worker and WASM assets under the selected base.

`build-info.json` records `commit`, `ref`, `basePath`, `builtAt` and the source commit URL.
`pnpm run package:dist` requires a full `RESIM_BUILD_SHA` (or `GITHUB_SHA`, which takes precedence
in CI) and rejects a build whose commit or base path differs. It copies the same metadata to
`dist-build-meta.json` for legacy archives or `dist-root-build-meta.json` for root archives.
Different archive names prevent a root archive from replacing the legacy release asset for the
same SHA.

## Local and CI validation before deployment

From a clean, committed revision, using Node 24 and pnpm:

```bash
export RESIM_BUILD_SHA=$(git rev-parse HEAD)
export RESIM_BUILD_REF=$(git branch --show-current)
export RESIM_EXPECTED_SHA=$RESIM_BUILD_SHA

# Production root layout; includes the full Vitest suite and Rust IMPES gate.
export RESIM_BASE_PATH=/
pnpm run validate:product
pnpm run package:dist
bash scripts/smoke-preview.sh

# Optional legacy layout; reuses the same validated numerical engine.
export RESIM_BASE_PATH=/ressim/
pnpm run build
pnpm run package:dist
bash scripts/smoke-preview.sh
```

`smoke-preview.sh` starts and stops a loopback Vite preview and runs the existing Playwright
suite. For a server started separately, use the same `RESIM_BASE_PATH` for `pnpm run preview`
and `pnpm run test:deployed`. `RESIM_PUBLIC_URL` overrides the browser suite's address when
checking an actual host; the preview wrapper always tests localhost.
If port 4173 is occupied, set `RESIM_PREVIEW_PORT` for the wrapper; it refuses to test an existing
server in place of the build under test.

`.github/workflows/pr-tests.yml` runs on pull requests or manual dispatch and checks both layouts
without deploying. Existing solver, Buckley-Leverett, compositional thermodynamics, native binding
parity, benchmark and OPM artifact gates remain in place. Each bundle is packaged with an exact
SHA check and browser-tested for direct load/refresh, metadata and local social-image assets,
Worker/WASM, IMPES/FIM, compositional runs, charts, styling, 3D and cookie-free operation.
The browser checks compare `build-info.json` with `RESIM_EXPECTED_SHA` and the requested base path.

The application sets no cookies, uses no analytics and serves fonts locally. Hosting must preserve
that behavior; smoke-test the live HTTPS address as well as the local build. Numerical models,
dependencies and numerical acceptance tolerances are unchanged by the layout option.

## Current release flow

Every push to `master` runs `.github/workflows/publish-dist.yml`; it also supports manual dispatch:

1. Build WASM from the workflow revision and run `pnpm run validate:product` with
   `RESIM_BASE_PATH=/`.
2. Verify the embedded revision/base and package `ressim-root-dist-<full-commit-sha>.tar.gz`
   with `dist-root-build-meta.json`.
3. Upload and deploy the application directory returned by the packaging step (`dist-root`)
   to GitHub Pages. Archive and metadata upload paths also come from that step's outputs.
4. Run `pnpm run test:deployed` against `https://ressim.farin.nl/` with `RESIM_BASE_PATH=/`
   and `RESIM_EXPECTED_SHA` set to the exact workflow SHA. This includes the cookie checks.
5. After the deployed smoke suite passes, publish the tested archive and metadata as release
   `dist-<full-commit-sha>`.

There is no promotion or repository dispatch to `rekenraam-web`. The parent site's submodule and
hosting are managed separately.

## Live verification and hosting configuration

The [root cutover deployment](https://github.com/sergeyfarin/ressim/actions/runs/40402259099)
completed successfully. To verify a deployment independently, check out the intended deployed
revision and run:

```bash
RESIM_BASE_PATH=/ \
RESIM_PUBLIC_URL=https://ressim.farin.nl/ \
RESIM_EXPECTED_SHA="$(git rev-parse HEAD)" \
pnpm run test:deployed
```

Use the SHA of the intended release workflow, rather than deriving the expected SHA from the
live server. The suite checks `/build-info.json`, direct load/refresh, assets, Worker/WASM,
the second page, canonical/social metadata and cookie-free operation. A successful Pages upload
alone does not establish that the custom domain serves the intended revision.

The Pages custom domain, DNS and TLS are managed outside the build and packaging scripts.
Changes to the former `https://farin.nl/ressim/` hosting or its redirects belong to the parent
site's deployment configuration and remain separate from this workflow.

## Rollback

For a root-host regression, redeploy the last known-good **root** archive with its matching
metadata, verify `/build-info.json` and run the exact-SHA smoke suite. A legacy archive requires
legacy hosting at `/ressim/`; it cannot be unpacked as a root application without rebuilding.

For a workflow-based rollback, manually run the release workflow from a known-good commit/tag
that supports the root layout and the current custom-domain smoke checks. Record the exceptional
deployment in issue #8: the next successful `master` run replaces it. Rolling back ResSim leaves
the separately hosted parent site unchanged; its rollback belongs in its owning repository.
