# Elyrinth: Offline Mode + ely.by Login — Implementation Plan

Date: 2026-09-06
Status: Approved for implementation
Spec: `docs/superpowers/specs/2026-09-06-elyrinth-offline-elyby-design.md`

## Overview

Two feature blocks for the Modrinth desktop app (fork "Elyrinth", monorepo `modrinth/code`):

1. **Full login via ely.by** — a second account type alongside Microsoft, usable for launching instances (with authlib-injector) and joining ely.by-compatible servers.
2. **Full offline operation** — launching installed instances offline, browsing cached search/project data offline, and queueing install/update actions that auto-run when connectivity returns.

User decisions locked in during planning:
- Approach A: hybrid — Rust core (theseus / `packages/app-lib`) + thin Tauri seams (`apps/app`) + frontend (`apps/app-frontend` + `packages/api-client`).
- Offline flags live on the **runtime-only** `State.offline: AtomicBool` (user-confirmed). No settings-table migration, no SQL churn in the giant `Settings::get/update` blocks.
- Whole login timeout / profiles / skins for ely.by accounts in v1 are out of scope (design section 6).

Key facts verified during fact-gathering (with `file:line` anchors):

- Auth logic is 100% Rust: `packages/app-lib/src/state/minecraft_auth.rs`.
  - `MinecraftAuthStep` enum: line 36.
  - `MinecraftLoginFlow { verifier, challenge, session_id, auth_request_uri }`: line 103.
  - `login_begin(exec)`: line 111 (Microsoft Sisu redirect only).
  - `login_finish(code, flow, exec)`: line 142.
  - `Credentials { offline_profile, access_token: String, refresh_token: String, expires: DateTime<Utc>, active: bool }`: line 201. `impl Credentials` (upsert/get/get_all/refresh/online_profile): line 267.
  - `RequestWithDate<T>`: line 828. `auth_retry` helper: line 1467. `INSECURE_REQWEST_CLIENT` from `crate::util::fetch`.
  - `MinecraftProfile { id: Uuid, name, skins, capes, fetch_time }`: line 1306.
- API seam: `packages/app-lib/src/api/minecraft_auth.rs` — `check_reachable` (mojang hasJoined, line 10), `begin_login`, `finish_login`, `get_default_user`, `set_default_user`, `remove_user`, `users`.
- Tauri plugin: `apps/app/src/api/auth.rs` — plugin `"auth"`, command `login(flow)` opens webview `signin` and polls for `code` in the redirect URL; `check_reachable`, `get_default_user`, `set_default_user`, `remove_user`, `get_users`.
- Tauri plugin registration: `apps/app/src/main.rs` lines 249–272 (`.plugin(api::auth::init())` at 250; list ends `.plugin(api::worlds::init())` at 271). Commands via `tauri::generate_handler!` at 273.
- Session join: `packages/app-lib/src/api/instance/run.rs` line 226 `POST sessionserver.mojang.com/session/minecraft/join`.
- Launcher args: `packages/app-lib/src/launcher/args.rs` — `get_jvm_arguments(..., agent_path: &Path, ...)`; `${user_type}` == `"msa"`, `${auth_xuid}` == `"0"` (hardcoded).
- Launch path: `packages/app-lib/src/launcher/mod.rs` — `launch_minecraft` at 788; `resolve_minecraft_manifest` at 277 (uses `CachedEntry`); `download_version_info` at 853/870 (reads local json first); `download_minecraft` at 423/504 only used during install. Launch does NOT download the jar.
- `CachedEntry` / `CacheValue`: `packages/app-lib/src/state/cache.rs` (default behaviour `StaleWhileRevalidateSkipOffline`).
- Cache plugin (browsing): `apps/app/src/api/cache.rs` — plugin `"cache"`, commands `get_project(_v3)(_many)`, `get_version(_many)`, `get_user(_many)`, `get_team(_many)`, `get_organization(_many)`, `get_search_results(_v3)(_many)`. Frontend calls via `apps/app-frontend/src/helpers/cache.js`.
- api-client feature chain: `packages/api-client/src/features/retry.ts` (pattern to copy); `TauriModrinthClient extends XHRUploadClient` in `packages/api-client/src/platform/tauri.ts` (features array ~line 34); client constructed in `apps/app-frontend/src/App.vue:301`.
- Migrations: `packages/app-lib/migrations/*.sql`, applied by `sqlx::migrate!()` in `packages/app-lib/src/state/db.rs:38`. Style: `{YYYY}{MM}{DD}{HHMMSS}_description.sql`.
- Tests exist in `packages/app-lib` as `#[test]` / `#[tokio::test]` (e.g. `api/server_address.rs:171`, `api/minecraft_skins/png_util.rs:375`). No test infra in `apps/app`. `apps/app-frontend` uses vitest + @vue/test-utils.
- Git: repo on branch `main`, HEAD `e22a0a2` (design doc). Commit messages style: `feat:`, `fix:`, `docs:`. `.repowise/` must never be committed.

ely.by endpoints (verified from https://docs.ely.by/ ... in spec):

- Authorize: `GET https://account.ely.by/oauth2/v1` with `client_id`, `redirect_uri`, `response_type=code`, `scope=account_info minecraft_server_session offline_access` (space-separated, URL-encoded), `state`, optional `code_challenge`/`code_challenge_method=S256`.
- Token: `POST https://account.ely.by/api/oauth2/v1/token` form-encoded (`grant_type=authorization_code|refresh_token`, `code`, `redirect_uri`, `code_verifier`, `client_id`, optional `client_secret`). Response `{ token_type, expires_in, access_token, refresh_token, scope }`.
- Account info: `GET https://account.ely.by/api/account/v1/info` with `Authorization: Bearer <access_token>`. Response contains `id` (int), `username`, `uuid` (hyphenated UUID).
- Session join (for osancan online servers): `POST https://sessionserver.ely.by/session/minecraft/join` — same body as Mojang's join endpoint.
- Sessionserver ely.by `/session/minecraft/hasJoined` is server-side only (not used by the launcher).

The ely.by `access_token` IS a valid Minecraft token (Yggdrasil-compatible). The account UUID from account info is the player UUID.

## Task List

- Task 1 — `AccountType` enum + DB column + `Credentials` serde/DB support
- Task 2 — ely.by OAuth2 HTTP module (`packages/app-lib/src/state/elyby.rs`)
- Task 3 — ely.by login flow: `MinecraftLoginFlow` extension, `elyby_login_begin`, `login_finish` branch, `Credentials::refresh`/`online_profile` branches
- Task 4 — account-aware session join (`run.rs`) + authlib-injector JVM args (`args.rs`) + `check_reachable`
- Task 5 — Tauri seam: `login(flow)` in `apps/app/src/api/auth.rs` + new `elyby_begin_login` + frontend button in `AccountsCard.vue`
- Task 6 — connectivity state: `State.offline: AtomicBool` + `connectivity` module + `apps/app/src/api/connectivity.rs` plugin
- Task 7 — offline gating: `CachedEntry` offline guard + friendly `LauncherError` on offline launch with missing files + `run.rs` pre-flight
- Task 8 — offline cache backend: `api_cache` table + `state/api_cache.rs` + `apps/app/src/api/api_cache.rs` plugin
- Task 9 — `OfflineCacheFeature` in `packages/api-client` + wiring into `TauriModrinthClient` + UI hook-up
- Task 10 — offline queue backend: `offline_queue` table + `state/offline_queue.rs` + `apps/app/src/api/queue.rs` plugin
- Task 11 — frontend queue: `useConnectivity` composable, queue interception in `providers/content-install.ts`, queue UI section
- Task 12 — final verification: Rust tests, vitest, `pnpm prepr:frontend:app`, manual checks, wrap-up

---

## Task 1 — `AccountType` enum + DB column + `Credentials` serde/DB support

### Rationale

The database and `Credentials` must record which identity provider a user signed in with, so refresh/session-join/args can be dispatched correctly per account.

### Task list

- [ ] Read `packages/app-lib/migrations/20240711194701_init.sql` to see the current `minecraft_users` table columns (needed for the right `ALTER TABLE` and `INSERT`/`SELECT` edits).
- [ ] Add `AccountType` enum.
- [ ] Add migration `packages/app-lib/migrations/20260906120000_elyby-account-type.sql`.
- [ ] Thread `account_type` through `Credentials`' DB queries.
- [ ] Add serde/DB round-trip test.

### Code changes

**A. New migration file** `packages/app-lib/migrations/20260906120000_elyby-account-type.sql`:

```sql
ALTER TABLE minecraft_users ADD COLUMN account_type TEXT NOT NULL DEFAULT 'microsoft';
```

**B. Enum** in `packages/app-lib/src/state/minecraft_auth.rs` (place above `MinecraftLoginFlow`, after the `MinecraftAuthStep` enum):

```rust
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum AccountType {
    Microsoft,
    ElyBy,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::ElyBy => "elyby",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "elyby" => Self::ElyBy,
            _ => Self::Microsoft,
        }
    }
}
```

Note: `#[sqlx(rename_all = "snake_case")]` requires the `sqlx::Type` derive. If the crate's `sqlx` feature `derive` is unavailable for enums on TEXT columns, fall back to storing the `String` variant name explicitly in queries (keep `AccountType` `Deserialize`/`Serialize` only) — the plan's SQL uses `account_type` as a plain `TEXT` column either way.

**C. `Credentials` struct** — add field:

```rust
pub account_type: AccountType,
```

**D. DB queries in `impl Credentials`** (line 267+). Every `sqlx::query!` that reads the `minecraft_users` table must add `account_type`, and every write must set it. Exactly which blocks to touch (read the file section before editing): `Credentials::get`, `Credentials::get_all`, `Credentials::get_default_credential`, `Credentials::upsert` (check whether these live in `impl Credentials` directly or in a `CredentialsDb` helper; adjust accordingly). Map with `AccountType::from_str(&res.account_type)` on read; pass `self.account_type.as_str()` on write.

**E. `MinecraftLoginFlow` struct** — add field now (used by Task 3):

```rust
pub struct MinecraftLoginFlow {
    pub account_type: AccountType,
    pub verifier: String,
    pub challenge: String,
    pub session_id: String,
    pub auth_request_uri: String,
}
```

Update the Microsoft `login_begin` constructor (line 130) to set `account_type: AccountType::Microsoft`.

### Tests

In `packages/app-lib/src/state/minecraft_auth.rs`, add a `#[cfg(test)] mod tests` (or extend existing):

```rust
#[test]
fn account_type_as_str_roundtrip() {
    assert_eq!(AccountType::Microsoft.as_str(), "microsoft");
    assert_eq!(AccountType::ElyBy.as_str(), "elyby");
    assert_eq!(AccountType::from_str("elyby"), AccountType::ElyBy);
    assert_eq!(AccountType::from_str("microsoft"), AccountType::Microsoft);
    assert_eq!(AccountType::from_str("bogus"), AccountType::Microsoft);
}

#[test]
fn account_type_serde_roundtrip() {
    let json = serde_json::to_string(&AccountType::ElyBy).unwrap();
    assert_eq!(json, "\"elyby\"");
    let back: AccountType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, AccountType::ElyBy);
}
```

### Out-of-scope

- Any logic change to Microsoft flow.
- Backfilling `minecraft_users` beyond the `DEFAULT 'microsoft'` in the migration.

### Final checklist

- [ ] `cargo check -p theseus` passes.
- [ ] `cargo test -p theseus` passes (new tests green).

---

## Task 2 — ely.by OAuth2 HTTP module

### Rationale

A self-contained HTTP module keeps the ely.by wire protocol distinct from the Xbox/Microsoft "send_signed_request" machinery. It reuses the existing `MinecraftAuthenticationError`/`MinecraftAuthStep` error plumbing and the constant retry helper.

### Task list

- [ ] Add `MinecraftAuthStep` variants for ely.by.
- [ ] Create `packages/app-lib/src/state/elyby.rs` with token/refresh/account-info calls.
- [ ] Register the module in `packages/app-lib/src/state/mod.rs`.
- [ ] Unit tests parsing recorded ely.by JSON responses.

### Code changes

**A. Extend `MinecraftAuthStep`** in `packages/app-lib/src/state/minecraft_auth.rs:36`:

```rust
// (existing variants unchanged)
ElyByAuthorize,
ElyByToken,
ElyByRefresh,
ElyByAccountInfo,
```

The derived `Serialize`/`Debug`/`Display` (whatever it derives) will pick these up automatically. Skip the step in `ErrorKind::from`/`From<MinecraftAuthenticationError>` mapping if it uses a match — add arms.

**B. New module** `packages/app-lib/src/state/elyby.rs`:

```rust
use chrono::{DateTime, Utc, Duration};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::minecraft_auth::{
    MinecraftAuthenticationError, MinecraftAuthStep,
};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;

pub const ELYBY_OAUTH_AUTHORIZE_URL: &str = "https://account.ely.by/oauth2/v1";
pub const ELYBY_OAUTH_TOKEN_URL: &str = "https://account.ely.by/api/oauth2/v1/token";
pub const ELYBY_ACCOUNT_INFO_URL: &str = "https://account.ely.by/api/account/v1/info";
pub const ELYBY_SESSION_JOIN_URL: &str = "https://sessionserver.ely.by/session/minecraft/join";

pub const ELYBY_SCOPE: &str = "account_info minecraft_server_session offline_access";
const ELYBY_CLIENT_ID: &str = "elyrinth2";
pub const ELYBY_DEFAULT_REDIRECT_URI: &str = "https://elyrinth-modrinth/oauth";

pub fn elyby_client_id() -> String {
    std::env::var("ELYBY_CLIENT_ID").unwrap_or_else(|_| ELYBY_CLIENT_ID.to_string())
}

pub fn elyby_client_secret() -> Option<String> {
    std::env::var("ELYBY_CLIENT_SECRET").ok().filter(|v| !v.is_empty())
}

pub fn elyby_redirect_uri() -> String {
    std::env::var("ELYBY_REDIRECT_URI")
        .unwrap_or_else(|_| ELYBY_DEFAULT_REDIRECT_URI.to_string())
}

pub fn elyby_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: Option<&str>,
) -> Result<url::Url, MinecraftAuthenticationError> {
    let mut url = url::Url::parse(ELYBY_OAUTH_AUTHORIZE_URL)
        .map_err(|source| MinecraftAuthenticationError::Other(source.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", ELYBY_SCOPE)
        .append_pair("state", state);
    if let Some(challenge) = challenge {
        url.query_pairs_mut()
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }
    Ok(url)
}

#[derive(Deserialize, Debug)]
pub struct ElyByTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

impl ElyByTokenResponse {
    pub fn expires_at(&self, base: DateTime<Utc>) -> DateTime<Utc> {
        base + Duration::seconds(self.expires_in as i64)
    }
}

#[derive(Deserialize, Debug)]
pub struct ElyByAccount {
    pub id: i64,
    pub username: String,
    pub uuid: String,
}

async fn elyby_token_request(
    form: &HashMap<&str, String>,
    step: MinecraftAuthStep,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let status;
    let text;
    {
        let res = super::minecraft_auth::auth_retry(|| {
            INSECURE_REQWEST_CLIENT
                .post(ELYBY_OAUTH_TOKEN_URL)
                .header("Accept", "application/json")
                .form(form)
                .send()
        })
        .await
        .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;
        status = res.status();
        text = res.text().await.map_err(|source| {
            MinecraftAuthenticationError::Request { source, step }
        })?;
    }
    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source, raw: text, step, status_code: status,
        }
    })
}

pub async fn elyby_exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let mut form = HashMap::new();
    form.insert("client_id", client_id.to_string());
    form.insert("grant_type", "authorization_code".to_string());
    form.insert("code", code.to_string());
    form.insert("redirect_uri", redirect_uri.to_string());
    form.insert("code_verifier", verifier.to_string());
    if let Some(secret) = client_secret {
        form.insert("client_secret", secret.to_string());
    }
    elyby_token_request(&form, MinecraftAuthStep::ElyByToken).await
}

pub async fn elyby_refresh_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<ElyByTokenResponse, MinecraftAuthenticationError> {
    let mut form = HashMap::new();
    form.insert("client_id", client_id.to_string());
    form.insert("grant_type", "refresh_token".to_string());
    form.insert("refresh_token", refresh_token.to_string());
    if let Some(secret) = client_secret {
        form.insert("client_secret", secret.to_string());
    }
    elyby_token_request(&form, MinecraftAuthStep::ElyByRefresh).await
}

pub async fn elyby_account_info(
    access_token: &str,
) -> Result<ElyByAccount, MinecraftAuthenticationError> {
    let res = super::minecraft_auth::auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .get(ELYBY_ACCOUNT_INFO_URL)
            .header("Accept", "application/json")
            .bearer_auth(access_token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::ElyByAccountInfo,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::ElyByAccountInfo,
        }
    })?;

    if let StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN = status {
        return Err(MinecraftAuthenticationError::InvalidToken);
    }

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source, raw: text, step: MinecraftAuthStep::ElyByAccountInfo,
            status_code: status,
        }
    })
}
```

Check before writing: `MinecraftAuthenticationError` variant names actually present in `minecraft_auth.rs` (e.g. `Request`, `DeserializeResponse`, `InvalidToken`, `SerializeBody`) — adapt the enum arms to the real names; the code above matches what was seen at lines 847/971/1413.

**C. Register module** in `packages/app-lib/src/state/mod.rs` (visible `pub mod elyby;` or `pub(super)` if only used from state/api layers).

### Tests

`#[cfg(test)] mod tests` in `elyby.rs`:

```rust
#[test]
fn authorize_url_builds_correct_query() {
    let url = elyby_authorize_url("elyrinth2", "https://elyrinth-modrinth/oauth", "abc123", Some("challenge")).unwrap();
    let pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    assert!(pairs.contains(&("client_id".into(), "elyrinth2".into())));
    assert!(pairs.contains(&("response_type".into(), "code".into())));
    assert!(pairs.contains(&("state".into(), "abc123".into())));
    assert!(pairs.contains(&("code_challenge".into(), "challenge".into())));
}

#[test]
fn parses_token_response() {
    let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT","scope":"account_info minecraft_server_session offline_access"}"#;
    let parsed: ElyByTokenResponse = serde_json::from_str(body).unwrap();
    assert_eq!(parsed.access_token, "AT");
    assert_eq!(parsed.refresh_token, "RT");
}

#[test]
fn parses_account_info() {
    let body = r#"{"id":123,"username":"TestUser","uuid":"6e1c2b99-0000-0000-0000-000000000000"}"#;
    let parsed: ElyByAccount = serde_json::from_str(body).unwrap();
    assert_eq!(parsed.username, "TestUser");
}
```

### Out-of-scope

- ely.by skin/cape API (design scope decision).
- Retrying non-connect errors (auth errors must surface immediately).

### Final checklist

- [ ] `cargo check -p theseus` and `cargo test -p theseus` pass.

---

## Task 3 — ely.by login flow: flow extension, `begin`/`finish` branch, `refresh`/`online_profile` branches

### Rationale

The login state machine (begin → webview → finish) and the credential refresh/profile path must dispatch on `account_type`.

### Task list

- [ ] `login_begin` — keep Microsoft path; add `elyby_login_begin(exec)`.
- [ ] `login_finish` — branch on `flow.account_type`.
- [ ] `Credentials::refresh` — branch on `self.account_type`.
- [ ] `Credentials::online_profile` — for ely.by, build profile from account info (id + username, no skins).
- [ ] Export new api-level fn in `packages/app-lib/src/api/minecraft_auth.rs`.

### Code changes

**A. `state/minecraft_auth.rs` — add state-level begin fn** (next to `login_begin`):

```rust
#[tracing::instrument]
pub async fn elyby_login_begin(
    _exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<MinecraftLoginFlow> {
    // PKCE
    let verifier = generate_oauth_challenge(); // 64 hex chars; OK as verifier
    let digest = sha2::Sha256::digest(&verifier);
    let challenge = BASE64_URL_SAFE_NO_PAD.encode(digest);
    let state = generate_oauth_challenge();

    let client_id = super::elyby::elyby_client_id();
    let redirect_uri = super::elyby::elyby_redirect_uri();

    let auth_request_uri = super::elyby::elyby_authorize_url(
        &client_id,
        &redirect_uri,
        &state,
        Some(&challenge),
    )?
    .to_string();

    Ok(MinecraftLoginFlow {
        account_type: AccountType::ElyBy,
        verifier,
        challenge,
        session_id: String::new(),
        auth_request_uri,
    })
}
```

(If `generate_oauth_challenge` returns hex chars only, PKCE requirement "43–128 ASCII chars, unreserved" is satisfied since hex is unreserved.)

**B. `login_finish`** — add a branch at the top:

```rust
if flow.account_type == AccountType::ElyBy {
    return finish_elyby_login(code, flow, exec).await;
}
```

New helper `finish_elyby_login`:

```rust
async fn finish_elyby_login(
    code: &str,
    flow: MinecraftLoginFlow,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<Credentials> {
    let client_id = super::elyby::elyby_client_id();
    let client_secret = super::elyby::elyby_client_secret();
    let redirect_uri = super::elyby::elyby_redirect_uri();

    let token = super::elyby::elyby_exchange_code(
        code, &flow.verifier, &redirect_uri, &client_id, client_secret.as_deref(),
    )
    .await
    .map_err(crate::ErrorKind::from)?;

    let account = super::elyby::elyby_account_info(&token.access_token)
        .await
        .map_err(crate::ErrorKind::from)?;

    let uuid = uuid::Uuid::parse_str(&account.uuid)
        .map_err(|source| crate::ErrorKind::OtherError(format!("Invalid ely.by UUID: {source}")))?;
    let now = Utc::now();

    let credentials = Credentials {
        offline_profile: MinecraftProfile {
            id: uuid,
            name: account.username,
            ..MinecraftProfile::default()
        },
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires: token.expires_at(now),
        active: true,
        account_type: AccountType::ElyBy,
    };

    credentials.upsert(exec).await?;
    Ok(credentials)
}
```

**C. `Credentials::refresh`** — at the top, before the Microsoft path:

```rust
if self.account_type == AccountType::ElyBy {
    return self.refresh_elyby(exec).await;
}
```

```rust
async fn refresh_elyby(
    &mut self,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<()> {
    let client_id = super::elyby::elyby_client_id();
    let client_secret = super::elyby::elyby_client_secret();
    let token = super::elyby::elyby_refresh_token(
        &self.refresh_token, &client_id, client_secret.as_deref(),
    )
    .await
    .map_err(crate::ErrorKind::from)?;

    self.access_token = token.access_token;
    self.refresh_token = token.refresh_token;
    self.expires = token.expires_at(Utc::now());
    self.upsert(exec).await?;
    Ok(())
}
```

**D. `Credentials::online_profile`** — locate the existing `online_profile()` implementation (Microsoft fetches `minecraftservices.com/minecraft/profile`). Add a branch at the top:

```rust
if self.account_type == AccountType::ElyBy
    && let Ok(account) = super::elyby::elyby_account_info(&self.access_token).await
    && let Ok(id) = uuid::Uuid::parse_str(&account.uuid)
{
    return Some(MinecraftProfile {
        id,
        name: account.username,
        ..MinecraftProfile::default()
    });
}
```

(Returns `None` on failure so the existing callers rely on `offline_profile`.)

**E. api seam** `packages/app-lib/src/api/minecraft_auth.rs` — add:

```rust
#[tracing::instrument]
pub async fn elyby_begin_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::elyby_login_begin(&state.pool).await
}
```

### Tests

- Unit test in `minecraft_auth.rs` (no network): `elyby_login_begin` on an in-memory sqlite pool produces a flow with `account_type == ElyBy`, an `auth_request_uri` containing `account.ely.by/oauth2/v1`, `client_id=elyrinth2`, `redirect_uri=...` and `code_challenge=`. Build the pool with `SqliteConnectOptions::new().in_memory(true)`. Requires `sqlx::migrate!()` not to be run (schema needed by `elyby_login_begin` is none — it only errors on pool access; acceptable to skip pool and call the pure parts instead: factor the URL construction so the test can call `elyby_authorize_url` directly — already covered in Task 2).

### Out-of-scope

- Offline (cracked) accounts.
- Device-token/Xbox machinery for ely.by (not needed — ely.by is a pure OAuth2 provider).

### Final checklist

- [ ] `cargo check -p theseus`, `cargo test -p theseus` pass.

---

## Task 4 — account-aware session join + authlib-injector JVM args + `check_reachable`

### Rationale

Online servers require a session-join POST; ely.by accounts must target `sessionserver.ely.by` and, even when not joining, instances launched with an ely.by account need authlib-injector as the javaagent so the vanilla client trusts the ely.by session.

### Task list

- [ ] `run.rs` — branch the join URL on `account_type`.
- [ ] `args.rs` — inject authlib-injector javaagent for ely.by users.
- [ ] `api/minecraft_auth.rs::check_reachable` — hit ely.by sessionserver for an ely.by default user (best-effort).

### Code changes

**A. `packages/app-lib/src/api/instance/run.rs` line 226.** The join body (`server_hash`, `profile_id`, `access_token`) is identical for both providers; only the endpoint differs. Replace the hardcoded URL:

```rust
let session_join_url = if credentials.account_type == AccountType::ElyBy {
    crate::state::elyby::ELYBY_SESSION_JOIN_URL
} else {
    "https://sessionserver.mojang.com/session/minecraft/join"
};
// existing .post(session_join_url) call
```

Ensure `AccountType` is imported in `run.rs` (`use crate::state::{...AccountType}` or via prelude).

**B. `packages/app-lib/src/launcher/args.rs`.** In `get_jvm_arguments`, where `${user_type}` and `${auth_xuid}` are currently hardcoded (`"msa"`, `"0"`), add an account-type parameter. Check the current signature — it takes a credentials/profile today (read the fn before editing). Add:

```rust
let account_type = launch_credentials.as_ref().map(|c| c.account_type).unwrap_or(AccountType::Microsoft);
```

and when building agent args:

```rust
if account_type == AccountType::ElyBy
    && let Some(agent_path) = agent_path
{
    args.push(format!(
        "-javaagent:{}=https://authserver.ely.by",
        agent_path.to_string_lossy()
    ));
    args.push("-Dauthlibinjector.side=client".to_string());
}
```

(`agent_path` already exists as a parameter for Microsoft's native auth agent; ely.by reuses the javaagent slot with a different `=` argument.) `user_type` stays `"msa"` for ely.by (minimal churn, verified launch-compatible).

**C. `check_reachable`** (`packages/app-lib/src/api/minecraft_auth.rs:10`) — currently always pings mojang. Change to also accept the instance's default user account type, or simply additionally probe `https://sessionserver.ely.by/session/minecraft/hasJoined` only if there is an ely.by default user:

```rust
pub async fn check_reachable() -> crate::Result<()> {
    let state = State::get().await?;
    let default = Credentials::get_default_credential(&state.pool).await?;
    let url = default
        .as_ref()
        .filter(|user| user.account_type == AccountType::ElyBy)
        .map(|_| "https://sessionserver.ely.by/session/minecraft/hasJoined")
        .unwrap_or("https://sessionserver.mojang.com/session/minecraft/hasJoined");
    let resp = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    if resp.status() == StatusCode::NO_CONTENT {
        return Ok(());
    }
    resp.error_for_status()?;
    Ok(())
}
```

### Tests

- `args.rs`: existing tests (if any) must still pass; add a test that ely.by account produces the `-javaagent:...=https://authserver.ely.by` arg and microsoft does not — pure function, no network.
- `run.rs`: no network test; verify via `cargo check`.

### Out-of-scope

- Vanilla online-mode servers for ely.by accounts (server side must be authlib-injector/ely.by compatible; a Mojang `hasJoined` check will reject them — expected).
- `user_type`/`auth_xuid` changes for ely.by.

### Final checklist

- [ ] `cargo check -p theseus` and `cargo test -p theseus` pass.
- [ ] Manual later: launching an instance on ely.by account shows authlib-injector args in logs.

---

## Task 5 — Tauri seam: `login(flow)` + new `elyby_begin_login` + frontend button

### Rationale

The webview login flow (`apps/app/src/api/auth.rs`) reuses the existing Microsoft flow mechanics; we need a flow argument and an ely.by button pairing.

### Task list

- [ ] `apps/app/src/api/auth.rs`: `login(flow)` accepts an account flow selector; add `elyby_begin_login` path wired to `api::minecraft_auth::elyby_begin_login`.
- [ ] Frontend: add "Войти через ely.by" in the accounts flow UI.

### Code changes

**A. `apps/app/src/api/auth.rs`** (read the full current file first — it defines `login(flow)` which opens the `signin` webview and polls for the code). Add a typed flow argument to the frontend-invoked command:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub flow: String, // "microsoft" | "elyby"
}
```

In `login`, dispatch:

```rust
let login_flow = if request.flow == "elyby" {
    api::minecraft_auth::elyby_begin_login().await?
} else {
    api::minecraft_auth::begin_login().await?
};
```

Keep the rest (webview `signin` window, redirect detection, `finish_login` with the received `code`) identical — `finish_login` already branches on `flow.account_type` (Task 3). The webview must not be reused: the current implementation creates a fresh `signin` window per login (verify and preserve).

**B. Frontend** — read `apps/app-frontend/src/helpers/auth.js` and the accounts UI component (e.g. `src/components/AccountsCard/`. The accounts card has a "Sign in" section invoking `login`). Add a second button next to the Microsoft one:

- label: `Войти через ely.by`
- invokes the same `login` command with `flow: 'elyby'`.

Follow the i18n pattern of the file (use `i18n` keys, not raw strings) — check how existing strings are stored (`packages/ui` locales) and add key(s).

### Tests

- `apps/app/frontend`: if `helpers/auth.js` has unit tests, add a test that `login('elyby')` passes `flow: 'elyby'` to the Tauri invoke mock. Otherwise manual verification during Task 12.

### Out-of-scope

- Distinct signin webview branding per provider.
- Remember-me / account switching UX beyond the existing default-account mechanics.

### Final checklist

- [ ] `cargo check -p theseus`, `cargo check --manifest-path apps/app/Cargo.toml` pass.
- [ ] Manual: clicking ely.by button opens account.ely.by.

---

## Task 6 — connectivity state + plugin

### Rationale

Offline behavior needs a single source of truth readable from any Rust layer and settable from the frontend: `State.offline: AtomicBool`.

### Task list

- [ ] Add `offline: AtomicBool` to `State` (init `false`), plus `set_offline`/`is_offline` accessors.
- [ ] Add `packages/app-lib/src/state/connectivity.rs` or keep accessors on `State` directly.
- [ ] New plugin `apps/app/src/api/connectivity.rs` ("connectivity"): `check`, `get_offline`, `set_offline`.
- [ ] Register plugin in `apps/app/src/main.rs`.

### Code changes

**A. `packages/app-lib/src/state/mod.rs`** — add to `State`:

```rust
pub offline: std::sync::atomic::AtomicBool,
```

Initialize with `AtomicBool::new(false)` in the `State` constructor. Add methods:

```rust
impl State {
    pub fn is_offline(&self) -> bool {
        self.offline.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_offline(&self, offline: bool) {
        self.offline.store(offline, std::sync::atomic::Ordering::Relaxed);
    }
}
```

`State::get()` is async; for use from non-async contexts add a convenience in `apps/app` instead (see plugin below) — the launcher path is async so `State::get().await?.is_offline()` works.

**B. New plugin** `apps/app/src/api/connectivity.rs` (mirror the `apps/app/src/api/cache.rs` plugin shape — `pub fn init() -> TauriPlugin<R>` + commands). Commands:

```rust
// check: perform a connectivity probe, update State.offline, return new offline bool
async fn check(app: tauri::AppHandle) -> api::Result<bool> {
    let state = theseus::State::get().await?;
    let reachable = probe_reachable(&state).await; // see below
    state.set_offline(!reachable);
    Ok(state.is_offline())
}

async fn get_offline() -> api::Result<bool> {
    let state = theseus::State::get().await?;
    Ok(state.is_offline())
}

async fn set_offline(offline: bool) -> api::Result<()> {
    let state = theseus::State::get().await?;
    state.set_offline(offline);
    Ok(())
}
```

`probe_reachable`: HEAD/GET `https://staging-api.modrinth.com/` with a short timeout (3s) via `theseus::util::fetch`'s client (`INSECURE_REQWEST_CLIENT.is_public()`) or simply call `api::minecraft_auth::check_reachable()` (cheap; already account-aware after Task 4). Prefer reusing `check_reachable` — it is already the app's network-health probe, and it correctly targets ely.by vs mojang.

Register in `main.rs` near line 270:

```rust
.plugin(api::connectivity::init())
```

and add `mod connectivity;` to `apps/app/src/api/mod.rs`.

### Tests

- Trivial; keep `state.mod` compile-clean. Offline gating tests land in Task 7.

### Out-of-scope

- OS-level network-change listeners in Rust (frontend listens to `navigator.onLine`; Task 11).

---

## Task 7 — offline gating: `CachedEntry` guard + friendly offline launch errors + pre-flight

### Rationale

"Offline" must mean: no network attempts that will fail-and-retry, cached data served, and accurate, friendly errors instead of raw connect timeouts.

### Task list

- [ ] `state/cache.rs`: inside the `CachedEntry` network-fetch path, short-circuit when offline.
- [ ] `launcher/mod.rs`: at launch, if offline and required local artifacts are absent → `ErrorKind::LauncherError` with a friendly message.
- [ ] Keep `download_version_info`'s local-file-first behaviour (already correct).

### Code changes

**A. `packages/app-lib/src/state/cache.rs`.** Locate the `CacheValue`/behaviour fetch loop (this is the `configured_cache_value...` region — read around the `CachedEntry` fetch before editing). At the point where the code is about to perform the backing network fetch (the browser component's `fetch`), insert:

```rust
if State::get().await?.is_offline() {
    return Err(ErrorKind::OfflineError(
        "App is offline; serving cached data only".into(),
    )
    .into());
}
```

This makes any cache miss while offline fail fast with a typed error instead of a connect timeout, while `StaleWhileRevalidateSkipOffline` still serves fresh or stale cache (its semantics already skip revalidation on failure).

**B. `packages/app-lib/src/error.rs`** — add variant:

```rust
OfflineError(String),
```

and wire it through the crate's error `Display` implementation. (Check the macro/`thiserror` style used in `error.rs` and mirror it.)

**C. `packages/app-lib/src/launcher/mod.rs`** — in `launch_minecraft` (line 788), before `resolve_minecraft_manifest`:

```rust
if State::get().await?.is_offline() {
    // The manifest serves install metadata; if it isn't cached locally,
    // an offline launch will fail later with a confusing error.
    if !version_info_cached_locally(state.directories.meta_dir()).await? {
        return Err(ErrorKind::LauncherError(
            "Cannot launch instance while offline: version data is not cached on this device".into(),
        ).into());
    }
}
```

Implement `version_info_cached_locally`: check for the presence of the version-info JSON in the meta dir (the same path `download_version_info` writes to/reads from at `launcher/download.rs:456`). If non-trivial to locate the existing local-path helper, reuse it directly instead of a new fn.

Also verify `resolve_minecraft_manifest` short-circuits on offline (it uses `CachedEntry` → Task 7A covers it); if it calls `get_minecraft_versions` which is a `CachedEntry`, no further work.

### Tests

- `#[test]` for the offline gate in `state/cache.rs` is not practical without a DB + network; instead add a focused unit test for `ErrorKind::OfflineError` Display formatting in `error.rs` if there's a test module there. Mark heavyweight behaviour as covered by manual verification in Task 12.

### Out-of-scope

- Offline modpack import/install (out of scope per design).
- Java/JRE auto-download while offline (fails fast via existing fetch path + friendly error).

### Final checklist

- [ ] `cargo check -p theseus` / `cargo test -p theseus` pass.

---

## Task 8 — offline cache backend (`api_cache` table + plugin)

### Rationale

Storage layer for browsing cache: namespace-keyed JSON blobs persisted in SQLite, exposed via Tauri so the api-client feature can read/write it.

### Task list

- [ ] Migration `20260906130000_offline-cache.sql`.
- [ ] `packages/app-lib/src/state/api_cache.rs`: `load`, `save`, `delete`, `clear`, `keys`.
- [ ] `apps/app/src/api/api_cache.rs`: plugin `"api-cache"` with `get`, `set`, `delete`, `clear`, `keys`.
- [ ] Register plugins in `main.rs` + module in `api/mod.rs` and `state/mod.rs`.
- [ ] Unit tests (in-memory sqlite).

### Code changes

**A. Migration** `packages/app-lib/migrations/20260906130000_offline-cache.sql`:

```sql
CREATE TABLE api_cache (
	namespace TEXT NOT NULL,
	cache_key TEXT NOT NULL,
	data JSONB NOT NULL,
	updated INTEGER NOT NULL,
	PRIMARY KEY (namespace, cache_key)
);

CREATE INDEX api_cache_namespace ON api_cache(namespace);
```

**B. `packages/app-lib/src/state/api_cache.rs`:**

```rust
use serde_json::Value;

pub async fn load(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
) -> crate::Result<Option<Value>> {
    let res = sqlx::query!(
        "SELECT json(data) AS \"data!: serde_json::Value\" FROM api_cache WHERE namespace = ? AND cache_key = ?",
        namespace,
        cache_key
    )
    .fetch_optional(exec)
    .await?;
    Ok(res.map(|row| row.data))
}

pub async fn save(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
    data: &Value,
) -> crate::Result<()> {
    let data = serde_json::to_string(data)?;
    sqlx::query!(
        "INSERT INTO api_cache (namespace, cache_key, data, updated) VALUES (?, ?, jsonb(?), unixepoch())
         ON CONFLICT (namespace, cache_key) DO UPDATE SET data = excluded.data, updated = excluded.updated",
        namespace,
        cache_key,
        data,
    )
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn delete(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
    cache_key: &str,
) -> crate::Result<()> {
    sqlx::query!("DELETE FROM api_cache WHERE namespace = ? AND cache_key = ?", namespace, cache_key)
        .execute(exec)
        .await?;
    Ok(())
}

pub async fn clear(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: Option<&str>,
) -> crate::Result<()> {
    if let Some(namespace) = namespace {
        sqlx::query!("DELETE FROM api_cache WHERE namespace = ?", namespace).execute(exec).await?;
    } else {
        sqlx::query!("DELETE FROM api_cache").execute(exec).await?;
    }
    Ok(())
}

pub async fn keys(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    namespace: &str,
) -> crate::Result<Vec<String>> {
    let res = sqlx::query!("SELECT cache_key FROM api_cache WHERE namespace = ? ORDER BY updated DESC", namespace)
        .fetch_all(exec)
        .await?;
    Ok(res.into_iter().map(|row| row.cache_key).collect())
}
```

Match the file's quoting/`jsonb` conventions seen elsewhere (`state/db.rs` uses `jsonb()`, `state/cache.rs` uses `json(...) AS "col!: serde_json::Value"`).

**C. Plugin** `apps/app/src/api/api_cache.rs` — follow `apps/app/src/api/cache.rs`'s plugin shape exactly (it's a thin wrapper over theseus state fns). Commands: `get(namespace, key)`, `set(namespace, key, data: serde_json::Value)`, `delete(namespace, key)`, `clear(namespace: Option<String>)`, `keys(namespace)`.

Register: `apps/app/src/main.rs` after `.plugin(api::worlds::init())` add `.plugin(api::api_cache::init())`; add `mod api_cache;` to `apps/app/src/api/mod.rs`.

**D. Frontend helper** `apps/app-frontend/src/helpers/apiCache.js`:

```js
export function apiCacheGet(namespace, key) {
  return invoke('plugin:api-cache|get', { namespace, key })
}
export function apiCacheSet(namespace, key, data) {
  return invoke('plugin:api-cache|set', { namespace, key, data })
}
export function apiCacheDelete(namespace, key) {
  return invoke('plugin:api-cache|delete', { namespace, key })
}
export function apiCacheClear(namespace = null) {
  return invoke('plugin:api-cache|clear', { namespace })
}
export function apiCacheKeys(namespace) {
  return invoke('plugin:api-cache|keys', { namespace })
}
```

Check the invoke import style of `apps/app-frontend/src/helpers/cache.js` and mirror it exactly.

### Tests

In `state/api_cache.rs`:

```rust
#[tokio::test]
async fn api_cache_roundtrip() {
    let options = sqlx::sqlite::SqliteConnectOptions::new().in_memory(true).create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePool::connect_with(options).await.unwrap();
    sqlx::query("CREATE TABLE api_cache (namespace TEXT NOT NULL, cache_key TEXT NOT NULL, data JSONB NOT NULL, updated INTEGER NOT NULL, PRIMARY KEY (namespace, cache_key))").execute(&pool).await.unwrap();

    assert!(load(&pool, "search", "q=test").await.unwrap().is_none());
    save(&pool, "search", "q=test", &serde_json::json!({"hits": []})).await.unwrap();
    assert!(load(&pool, "search", "q=test").await.unwrap().is_some());
    keys(&pool, "search").await.unwrap();
    delete(&pool, "search", "q=test").await.unwrap();
    assert!(load(&pool, "search", "q=test").await.unwrap().is_none());
}
```

### Out-of-scope

- TTL/eviction (frontend decides staleness; `updated` is available).
- Caching non-GET requests.

---

## Task 9 — `OfflineCacheFeature` in api-client + wiring

### Rationale

Browsing must work offline: reads that normally hit Modrinth should, when the app is offline, fall back to the persisted cache; successful responses are written to cache so the next offline session works.

### Task list

- [ ] Read `packages/api-client/src/core/abstract-client.ts` (request model) and `src/features/retry.ts` to copy the feature interface.
- [ ] Implement `packages/api-client/src/features/offline-cache.ts`.
- [ ] Insert it into `TauriModrinthClient` feature chain (`platform/tauri.ts`) and the App.vue construction.
- [ ] Vitest unit tests with a mocked invoke + fake base client.

### Code changes

**A. `packages/api-client/src/features/offline-cache.ts`.** Follow the `AbstractFeature` interface used by `RetryFeature`. Sketch (adapt to the actual `request`/`response` shapes in `abstract-client.ts` — verify):

```ts
import type { AbstractClientFetchContext } from '../core/abstract-client'
import type {
  UUID,
  ... // the types the feature chain passes through
} from '../core/...'
import { apiCacheGet, apiCacheSet } from '../../../apps/app-frontend/src/helpers/apiCache' // might not be importable from packages — see note

interface OfflineCacheOptions {
  get: (namespace: string, key: string) => Promise<unknown>
  set: (namespace: string, key: string, data: unknown) => Promise<unknown>
  isOffline: () => boolean
  namespaces?: (url: URL, method: string) => string
}
```

Note: `packages/api-client` must not import from `apps/app-frontend`. Instead, the Tauri platform layer creates the feature with injected fns that call `invoke('plugin:api-cache|...')` directly (the `invoke` import lives in the Tauri platform glue). The feature itself stores a `Map<namespace, Map<key, { data, updated }>>` mirror for cheap in-memory reads, delegating persistence to the injected `get`/`set`.

Behavior in the request wrapper:
- On **successful GET**: compute `key` from `path + canonicalized query` (sorted params, e.g. `search?limit=20&q=test`), store body under `namespace = 'projects' | 'versions' | 'search' | ...` derived from the path.
- On **error** when `isOffline()`: return `{ status: 200, body: cached }` from the mirror (or injected `get`), so the page renders cached content.

**B. `platform/tauri.ts`** — in the `TauriModrinthClient` features array (line ~34) add:

```ts
new OfflineCacheFeature({
  get: (namespace, key) => invoke('plugin:api-cache|get', { namespace, key }),
  set: (namespace, key, data) => invoke('plugin:api-cache|set', { namespace, key, data }),
  isOffline: () => localStorage.getItem('offline') === 'true', // replaced by useConnectivity in Task 11
})
```

(The offline source is refined in Task 11 once `useConnectivity` exists; the feature reads it via injected `isOffline`, so the wiring change is a single line.)

### Tests

`packages/api-client/src/features/__tests__/offline-cache.test.ts` (follow existing vitest pattern in that package, e.g. `features/retry.test.ts` if present — check `packages/api-client/src/features/**/__tests__`):

- compose a fake base feature whose GET returns success; assert `set` was called once with the right key.
- compose a failing base feature with `isOffline() → true`; assert the caller receives the cached body.

### Out-of-scope

- Caching POST/PATCH/DELETE.
- Cache eviction UI.

---

## Task 10 — offline queue backend (`offline_queue` + plugin)

### Rationale

Install/update attempts made while offline must be captured and replayed automatically when connectivity returns.

### Task list

- [ ] Migration `20260906140000_offline-queue.sql`.
- [ ] `packages/app-lib/src/state/offline_queue.rs`: `enqueue`, `list`, `remove`, `dequeue_pending`.
- [ ] `apps/app/src/api/queue.rs`: plugin `"queue"`: `enqueue`, `list`, `remove`.
- [ ] Register plugin + modules.
- [ ] Unit tests (in-memory sqlite).

### Code changes

**A. Migration** `packages/app-lib/migrations/20260906140000_offline-queue.sql`:

```sql
CREATE TABLE offline_queue (
	id TEXT NOT NULL PRIMARY KEY,
	kind TEXT NOT NULL,
	state JSONB NOT NULL,
	status TEXT NOT NULL DEFAULT 'pending',
	created INTEGER NOT NULL,
	attempted_after INTEGER NULL
);

CREATE INDEX offline_queue_status ON offline_queue(status);
```

**B. `packages/app-lib/src/state/offline_queue.rs`:** plain CRUD with `uuid::Uuid::new_v4()` ids. `kind` ∈ `{"install_project", "update_project"}` (extensible). `state` holds the frontend's payload (project id/version ids). Include `list::<T: DeserializeOwned>() → Vec<(String, String, T)>` decoding `state` generically.

**C. Plugin** `apps/app/src/api/queue.rs` — same thin-wrapper shape as Task 8C. Commands return/accept `serde_json::Value` for `state`.

Register in `main.rs` + `api/mod.rs` + `state/mod.rs`.

### Tests

Mirror the Task 8 test pattern: roundtrip enqueue/list/remove on in-memory sqlite.

### Out-of-scope

- Queue retry/backoff logic (frontend drives retry on reconnect — Task 11).
- Persistent job progress/summary UI beyond listing.

---

## Task 11 — frontend: `useConnectivity`, queue interception, queue UI

### Rationale

The frontend owns the reactive offline state and the UX: auto-detection, manual toggle, "queued for offline" toasts, the pending-actions UI, and replay on reconnect.

### Task list

- [ ] `apps/app-frontend/src/composables/useConnectivity.ts` (or `store/offline.ts` per repo convention): reactive `isOffline`, `check()`, listeners for `online`/`offline` events, calls `plugin:connectivity|check` on app start and on `window.ononline`.
- [ ] Browsing fallback: connect `OfflineCacheFeature`'s `isOffline` to the composable (Task 9 wire-up).
- [ ] Intercept install/update in `apps/app-frontend/src/providers/content-install.ts`: when `isOffline`, instead of launching the install flow, `invoke('plugin:queue|enqueue', { kind, state })` and toast "Действие добавлено в очередь офлайн".
- [ ] Queue UI: a "Pending offline actions" section (e.g. in a sidebar/location fitting the existing layout) listing queue items via `plugin:queue|list`, with "Retry" and "Remove"; auto-run on reconnect.
- [ ] Offline Mode toggle in settings: switch bound to `set_offline`.

### Code changes

Precise edits depend on files not yet read. Read in this order before editing: `App.vue` (line 301 client construction), `src/stores`/`composables` layout, `src/providers/content-install.ts`, `src/components/AccountsCard/*`, the settings UI. Then:

**A. `useConnectivity`:**

```ts
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const isOffline = ref(false)

export function useConnectivity() {
  async function check(): Promise<boolean> {
    const offline = await invoke<boolean>('plugin:connectivity|check')
    isOffline.value = offline
    return offline
  }

  function setOffline(offline: boolean): void {
    isOffline.value = offline
    void invoke('plugin:connectivity|set_offline', { offline })
  }

  return { isOffline: computed(() => isOffline.value), check, setOffline }
}

if (import.meta.client) {
  window.addEventListener('online', () => {
    isOffline.value = false
    void invoke('plugin:connectivity|set_offline', { offline: false })
  })
  window.addEventListener('offline', () => {
    isOffline.value = true
    void invoke('plugin:connectivity|set_offline', { offline: true })
  })
}
```

Call `check()` once after app mount (in `App.vue` setup).

**B. Queue interception** in the publish/install flow (content-install.ts `installProjectToInstance` or similar entry point): wrap the "about to install" moment; if `useConnectivity().isOffline` → enqueue + toast, skip network. On reconnect (`online` event) → `plugin:queue|list`, filter `status === 'pending'`, re-dispatch each item through the same install path that exists today, then `plugin:queue|remove`.

**C. Queue UI + Offline toggle:** follow existing UI conventions/components in the repo (use `@modrinth/ui`). Strings must go through the i18n system (check `packages/ui` locales + existing pattern; do NOT hardcode Russian in components).

### Tests

- Vitest for `useConnectivity` (mock `invoke`).
- Manual verification in Task 12 for the end-to-end queue flow (heavy to automate without a live backend).

### Out-of-scope

- Offline queue for actions other than install/update (design section 5).

---

## Task 12 — verification, pre-PR commands, wrap-up

### Rationale

Verify everything compiles/test-passes, then run the repo's control commands.

### Task list

- [ ] `cargo test -p theseus` (Rust: account type, ely.by parse, api_cache, offline_queue, args tests).
- [ ] `cargo check --manifest-path apps/app/Cargo.toml` (Tauri seams).
- [ ] `pnpm --filter @modrinth/api-client test` (OfflineCacheFeature vitest) — confirm script name first.
- [ ] `pnpm prepr:frontend:app` (lints, types, tests for app frontend + libs; AGENTS.md).
- [ ] Manual spot checks: ely.by sign-in (needs `.env.local` with credentials), offline launch of an installed instance, offline browse of a previously-loaded project, install-while-offline queued + auto-run on reconnect, JVM args show authlib-injector for ely.by launch.
- [ ] Create `.env.local` template documentation entry (ely.by client id/secret) — NOT a committed secret file.
- [ ] Say you're done: show git status, staged files, and the diff stat; open a PR only when the user asks.

### Final checklist

- [ ] No `ELYBY_CLIENT_SECRET` anywhere in tracked files.
- [ ] `.repowise/` still untracked.
- [ ] Design doc + plan committed; implementation commits in `feat:` style.