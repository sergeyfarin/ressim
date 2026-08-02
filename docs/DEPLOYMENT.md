# Deployment and rollback

`https://farin.nl/ressim/` is the production and canonical site. GitHub Pages at
`https://sergeyfarin.github.io/ressim/` is the latest validated `master` canary.

## Release flow

Every push to `master` runs `.github/workflows/publish-dist.yml` in this order:

1. Run `pnpm run validate:product` and build once with the Git commit embedded in
   `build-info.json`.
2. Deploy `dist/ressim` to GitHub Pages.
3. Smoke-test the deployed URL: direct load and refresh, asset errors, WASM/Worker readiness,
   one IMPES sensitivity, one FIM sensitivity, charts, and the WebGL 3D view.
4. Publish the exact tested archive as release `dist-<full-commit-sha>` with asset
   `ressim-dist-<full-commit-sha>.tar.gz`.
5. Send a `component-published` repository dispatch to `sergeyfarin/rekenraam-web`.
   The parent workflow advances only its `ressim` submodule to that SHA and commits the pointer;
   Netlify then publishes the parent commit to `farin.nl`.

The ResSim repository needs a fine-grained `REKENRAAM_WEB_TOKEN` Actions secret with permission
to dispatch workflows in `sergeyfarin/rekenraam-web`. Do not use a personal classic token with
broad account-wide write access.

The build served by either host exposes `/ressim/build-info.json`. Its `commit` must equal the
submodule SHA recorded by `rekenraam-web`.

## Rollback

Choose the last known-good `dist-<sha>` release, then update the parent submodule back to that SHA:

```bash
git -C ressim fetch origin <sha>
git -C ressim checkout <sha>
git add ressim
git commit -m "Rollback ResSim production to <sha>"
git push origin main
```

Netlify republishes the parent commit. `scripts/prepare-ressim.mjs` downloads the matching
`ressim-dist-<sha>.tar.gz` release asset, so rollback uses the same immutable build that passed
the Pages smoke test. GitHub Pages intentionally remains on latest validated `master`; rollback
of production does not rewrite the canary.

If Pages itself must be rolled back while `master` is being repaired, run the workflow manually
from the known-good commit/tag. Record that exceptional deployment in issue #8 because the next
successful `master` run will replace it.
