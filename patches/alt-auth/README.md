# Alt-Auth Patches (Elyrinth fork)

Regenerable patch series capturing the Elyrinth fork's divergence from upstream
`modrinth/code` for the alt-auth feature set (offline "pirate" accounts + ely.by).

Each `.patch` is the merge-base diff (`<base>...HEAD`) of one feature area:

| Patch               | Area                    |
| ------------------- | ----------------------- |
| `01-app.patch`      | `apps/app` (Tauri)      |
| `02-app-frontend.patch` | `apps/app-frontend` (UI) |
| `03-app-lib.patch`  | `packages/app-lib` (Rust core) |
| `04-api-client.patch` | `packages/api-client` (feature chain) |

Fork-only files that are not part of the feature set (docs, scripts, `.github`,
`pnpm-lock.yaml`) are deliberately excluded.

## Regenerate

```bash
git fetch upstream main
scripts/generate-patches.sh upstream/main
```

Run this after every upstream rebase so the series tracks the current base.
`MANIFEST.txt` records the base commit and generation time.

## Apply to a fresh upstream checkout

```bash
git apply --3way patches/alt-auth/*.patch
pnpm install   # lockfile is not patched; resolve on install
```

## ALT-AUTH markers

Edits to existing upstream source files are wrapped in
`// BEGIN ALT-AUTH` / `// END ALT-AUTH` (Rust) or
`<!-- BEGIN ALT-AUTH -->` / `<!-- END ALT-AUTH -->` (Vue) so the feature
hunks stay greppable and rebase-friendly. New files introduced by the fork
carry no markers. See `docs/internal/AUTH_ARCHITECTURE_NOTES.md`.

## CI

`.github/workflows/watch-upstream.yml` regenerates this series automatically
whenever upstream `main` advances and opens a sync PR.