# Alt-Auth Architecture Notes (Elyrinth fork)

These notes document how account authentication is wired through the Modrinth App
so the alt-auth feature set (offline "pirate" accounts + Ely.by) can be re-applied
to a fresh upstream checkout in minutes. All line numbers refer to the state of the
repo when this file was written and will drift after upstream rebases — verify with
`rg`/`grep` before relying on them.

## Account model

- `AccountType` enum (`packages/app-lib/src/state/minecraft_auth.rs`, ~line 54):
  `Microsoft` (default, serde `"microsoft"`), `ElyBy` (serde `"elyby"`). Decision was
  made to keep the enum-dispatch approach (NOT a `trait AuthProvider` refactor) to keep
  the alt-auth diff minimal and rebase-friendly. `as_str()` / `from_str()` roundtrip.
- `Credentials` struct (~line 318): `offline_profile: MinecraftProfile` (serde rename
  `"profile"`), `account_type` (`#[serde(default)]`), `access_token`, `refresh_token`,
  `expires: DateTime<Utc>`, `active: bool`.
- DB persistence: `minecraft_users` table, `account_type` column stores `as_str()`.
  Serialization to frontend via custom `Serialize` impl (~line 850) that also emits
  `account_type`. Queries at `get_active`/`get_all`/`upsert` (~lines 674-831).

## Login flows

- `api/minecraft_auth.rs` is the thin API layer: `check_reachable`, `begin_login`,
  `elyby_begin_login`, `finish_login`, `get_default_user`, `set_default_user`,
  `remove_user`, `users`.
- `check_reachable` (~line 10): picks the ely.by `hasJoined` endpoint when the default
  user is ElyBy, otherwise Mojang's. Offline accounts must NOT be the default for this
  to matter (or we add an `Offline` arm that always reports reachable).
- `state/minecraft_auth.rs`:
  - `login_begin` (~145) / `elyby_login_begin` (~177) build a `MinecraftLoginFlow`
    (holds `account_type` + PKCE verifier + auth request URI).
  - `login_finish` (~206) dispatches to `finish_elyby_login` when `ElyBy`, else the
    Microsoft device-code path.
  - `finish_elyby_login` (~269): exchanges code at ely.by token endpoint, fetches
    account info, builds `Credentials` with `AccountType::ElyBy`, upserts.
- Tauri command `login(flow)` (`apps/app/src/api/auth.rs`): `flow == "elyby"` chooses
  `elyby_begin_login`, opens a webview window titled `signin`, polls the window URL
  for the `code` query param matching the redirect prefix
  (`https://elyrinth-modrinth/oauth` for ely.by, `https://login.live.com/oauth20_desktop.srf`
  for Microsoft), then calls `finish_login`.
- Frontend helper `apps/app-frontend/src/helpers/auth.js`: `login(flow='microsoft')`,
  `get_default_user`, `set_default_user`, `remove_user`, `users`, `check_reachable`.
- UI: `apps/app-frontend/src/components/ui/AccountsCard.vue` lists credentials and has
  the Microsoft + Ely.by login buttons. Injected app-wide via
  `provide('accountsCard')` in `App.vue`; consumed in `pages/Skins.vue` and
  `components/ui/minecraft-required-modal/MinecraftRequiredModal.vue`.

## Ely.by OAuth module

`packages/app-lib/src/state/elyby.rs` — self-contained, no upstream deps beyond
`minecraft_auth`'s error types:

- Endpoints: `ELYBY_OAUTH_AUTHORIZE_URL` (`https://account.ely.by/oauth2/v1`),
  `ELYBY_OAUTH_TOKEN_URL`, `ELYBY_ACCOUNT_INFO_URL`, `ELYBY_SESSION_JOIN_URL`.
- Client config from env: `ELYBY_CLIENT_ID` (default `elyrinth2`),
  `ELYBY_CLIENT_SECRET` (optional), `ELYBY_REDIRECT_URI` (default
  `https://elyrinth-modrinth/oauth`).
- `elyby_authorize_url`, `elyby_exchange_code`, `elyby_refresh_token`,
  `elyby_account_info` — all return `MinecraftAuthenticationError`.
- `ElyByAccount { id, username, uuid }` — note uuid comes as a string.

## Session join + launch args

- Session join (`packages/app-lib/src/api/instance/run.rs` ~line 225): when launching
  a server-play instance, POSTs `accessToken`/`selectedProfile`/`serverId` to
  `ELYBY_SESSION_JOIN_URL` for ElyBy accounts, else Mojang's sessionserver. Offline
  accounts must SKIP the join entirely (they carry no online session).
- JVM args (`packages/app-lib/src/launcher/args.rs`):
  - `get_jvm_arguments` (~112, indexed even-line ~1086 caller) takes `account_type`
    (~127) and appends `-javaagent` pointing at an authlib-injector jar for ElyBy
    (~185-198, guarded by a test `elyby_uses_authlib_injector`).
  - `get_minecraft_arguments` (~273) reads `credentials.access_token` and
    `credentials.maybe_online_profile()`; `parse_minecraft_argument` (~347) substitutes
    `${accessToken}`, `${auth_access_token}`, `${auth_session}`, `${auth_player_name}`,
    `${auth_uuid}`, `${uuid}`, and hardcodes `${user_type}` = `"msa"` (~371).
  - For offline accounts: token is a fixed fake value (e.g. `"0"`), `user_type` should
    be `"legacy"`, and the profile UUID is derived deterministically from the name.
- `maybe_online_profile` (`state/minecraft_auth.rs` ~612): falls back to
  `offline_profile` when online fetch fails. ElyBy short-circuits via
  `elyby_account_info`. Offline accounts must return the offline profile immediately
  without any network.

## Offline account algorithm

- Java `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))` — an MD5
  hash set to UUID version 3 / variant IETF. `md-5` crate is already in `Cargo.lock`;
  `uuid` v3 feature is NOT enabled, so compute manually:
  digest = md5(`OfflinePlayer:` + name); set
  `digest[6] = (digest[6] & 0x0f) | 0x30` (version 3);
  `digest[8] = (digest[8] & 0x3f) | 0x80` (variant 10xx).
- Launch for offline accounts: `accessToken = "0"`, `user.type = "legacy"`, uuid above.
  These accounts only join servers with `online-mode=false` (server-side offline).

## Proposed alt-auth seam (new functionality → new files)

- `packages/app-lib/src/state/offline.rs` — `offline_uuid(name)`,
  `create_offline_credentials(name)`.
- Tauri commands `apps/app/src/api/auth_offline.rs` (`executor`, `get_users`,
  `set_default_user`, `remove_user` reuse existing minecraft_auth fns) + registration
  in `apps/app/src/main.rs` invoke handler.
- `AccountType::Offline` variant + `from_str` "offline" arm (marked `// BEGIN ALT-AUTH`).
- Frontend: `AddOfflineAccountModal.vue`, `AddElyByAccountModal.vue`,
  `ProviderBadge.vue`, wiring in `AccountsCard.vue` with `<!-- BEGIN ALT-AUTH -->`
  markers, i18n keys in en-US.
- Every edit to an existing upstream file is wrapped in `BEGIN ALT-AUTH` / `END ALT-AUTH`
  markers so patches stay extractable.

## Testing

- Unit tests live in the same files (`#[cfg(test)]` modules). Run with
  `cargo test -p theseus` against the package (package name in `packages/app-lib/Cargo.toml`).
- sqlx offline cache in `packages/app-lib/.sqlx` is checked in; regenerate with
  `cargo sqlx prepare` after adding/removing queries (local sqlite DB under
  `packages/app-lib/.sqlx/generated/`, gitignored). CI runs with `SQLX_OFFLINE=true`.
- Frontend lint: `pnpm prepr:frontend:app`. API-client tests: `pnpm --filter @modrinth/api-client test`.