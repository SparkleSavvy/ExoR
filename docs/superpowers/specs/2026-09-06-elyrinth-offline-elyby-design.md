# Дизайн: оффлайн-режим + ely.by (Elyrinth)

Дата: 2026-09-06
Форк: Modrinth desktop app (`modrinth/code`, ветка `main`)

## Контекст и цели

Доработка десктоп-приложения Modrinth под два требования:

1. **Полная оффлайн-работа приложения** — три сценария:
   - запуск уже установленных инстансов без сети;
   - просмотр ранее открытых/установленных модов из локального кэша;
   - очередь действий (установка/обновление модов) с автовыполнением при появлении сети.
2. **Полный вход через ely.by** — аккаунт ely.by как тип входа наравне с Microsoft:
   - OAuth2-логин через `account.ely.by`;
   - токен как Minecraft-токен;
   - профиль (uuid + ник) для запуска;
   - скины отображаются из текстур профиля ely.by;
   - **authlib-injector** при запуске с ely.by-аккаунтом для online-режима серверов (Вариант 2 — утверждён).

Принят подход **A: гибрид Rust-ядро + швы во фронтенде**. Оффлайн-аккаунты (фейковый UUID) — **вне scope**.

## Архитектура решений

### Секция 1 — ely.by: модель аккаунтов (Rust)

- Новый enum `AccountType { Microsoft, ElyBy }` в `packages/app-lib/src/state/minecraft_auth.rs` (паттерн — как `MinecraftSkinVariant`):
  ```rust
  #[derive(sqlx::Type, Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
  #[serde(rename_all = "UPPERCASE")]
  #[sqlx(rename_all = "UPPERCASE")]
  pub enum AccountType { Microsoft, ElyBy }
  ```
- Миграция: `ALTER TABLE minecraft_users ADD COLUMN account_type TEXT NOT NULL DEFAULT 'MICROSOFT'`.
- `Credentials` получает поле `account_type: AccountType`. Ветвления по типу:
  - `refresh()` — Microsoft: `oauth_refresh → sisu → xsts → minecraft_token`; ElyBy: `POST account.ely.by/api/oauth2/v1/token` (`grant_type=refresh_token`, client_id/secret) — новый `access_token` сразу является Minecraft-токеном (JWT, scope `minecraft_server_session`).
  - `online_profile()` — Microsoft: `api.minecraftservices.com/minecraft/profile`; ElyBy: Yggdrasil-совместимый профиль `authserver.ely.by/session/minecraft/profile/{uuid}` (точный путь уточнить при реализации).
  - `check_reachable()` — по auth-серверу типа аккаунта.
  - `upsert()/get_active()/get_all()` — колонка `account_type`.
- OAuth2-флоу ely.by (`login_begin_elyby` / `login_finish_elyby`):
  - authorize: `https://account.ely.by/oauth2/v1?client_id=…&redirect_uri=…&response_type=code&scope=account_info minecraft_server_session offline_access&state=…`
  - token: `POST account.ely.by/api/oauth2/v1/token` (authorization_code; затем refresh_token).
  - профиль: `GET account.ely.by/api/account/v1/info` → `{uuid, username}` → `offline_profile`.
- OAuth-приложение (регистрация пользователем): `client_id=elyrinth2`; `client_secret` задаётся в **gitignored** `packages/app-lib/.env.local` (`ELYBY_CLIENT_ID`, `ELYBY_CLIENT_SECRET`, `ELYBY_REDIRECT_URI`). Секрет в git не попадает. redirect_uri: `https://elyrinth-modrinth/oauth` (перехват в вебвью по префиксу).

### Секция 2 — ely.by: Tauri, UI, authlib-injector

- `apps/app/src/api/auth.rs`: `login(app, flow: String)` — `flow ∈ {"microsoft","elyby"}`; поллинг URL по префиксу `ELYBY_REDIRECT_URI` + `code`.
- Запуск (`run.rs`): сессионный join по типу аккаунта — Microsoft: `sessionserver.mojang.com/session/minecraft/join`; ElyBy: `authserver.ely.by/session/join`.
- authlib-injector (только для ElyBy): скачивание jar (latest.json → `#executable-url`) один раз в `libraries/` инстанса; добавление `-javaagent:<jar>=https://authserver.ely.by` + `-Dauthlibinjector.side=client` в JVM-аргументы. Новый модуль `packages/app-lib/src/launcher/authlib_injector.rs`. В `args.rs::get_jvm_arguments` — вторая позиция javaagent.
- `${user_type}` пер-аккаунтный: Microsoft → `"msa"`, ElyBy → `"legacy"`.
- UI (`apps/app-frontend`):
  - `helpers/auth.js`: `login(flow)`.
  - `AccountsCard.vue`: выбор «Microsoft / Ely.by» при добавлении; бейдж типа аккаунта.
  - `Skins.vue`: для ely.by — только отображение скина (из текстур профиля). Редактирование — Non-Goal.
  - `WelcomeScreen`, `MinecraftAuthErrorModal`, `MinecraftRequiredModal` — i18n-строки под ely.by.

### Секция 3 — Оффлайн-запуск

- Новый `packages/app-lib/src/state/connectivity.rs`: `State::set_offline(bool)` (фронт шлёт `plugin:app|set_offline` из `navigator.onLine` + пробы); понятные `LauncherError` вместо таймаутов.
- Launch-path при `offline == true`:
  - `resolve_minecraft_manifest` / `get_loader_version_from_profile`: только из кэша, без попытки сети. `MustRevalidate`-fallback → понятная ошибка.
  - `download_version_info`: локальный JSON есть → ок; нет → ошибка «оффлайн-запуск требует полностью установленный инстанс».
  - `download_log_config`: переиспользовать локальный файл или пропустить.
  - `session join`: skip + warn, не ронять запуск.
  - Недостающие файлы оффлайн не докачиваем.
- Тумблер «Оффлайн-режим» в `Settings` (`offline_mode`) + авто-детект; UI-тумблер в шапке.
- `CachedEntry` (state/cache.rs): в общем fetch-пути — проверка `offline`, сеть не пробуем, сразу stale/ошибка.
- Non-Goal: оффлайн-установка с нуля; запуск требует полностью установленный инстанс.

### Секция 4 — Оффлайн-бранчинг (кэш просмотра)

- Через существующий `CachedEntry`/`plugin:cache`: уже покрывает поиск/проект/версию/команду/пользователя; добавляем оффлайн-гейт (см. секцию 3) — без таймаутов.
- Новый `packages/api-client/src/features/offline-cache.ts` (`AbstractFeature`, как `retry.ts`):
  - `shouldApply`: `method === GET`, api ∈ {labrinth, launchermeta}, не `excludePaths` (auth/аккаунт/friends), результат JSON.
  - успех → `JSON.stringify(result)` в кэш по url (`plugin:api-cache|set`); сетевая ошибка → `plugin:api-cache|get`, вернуть stale или пробросить.
- Хранилище: новая таблица `api_cache (url TEXT PRIMARY KEY, body TEXT NOT NULL, stored_at INTEGER NOT NULL)` в SQLite этихус; новый плагин `plugin:api-cache` (`apps/app/src/api/cache_api.rs`: `get`, `set`) по образцу `cache.rs`.
- Подключение в `App.vue:301` (features клиента).
- Пурдж: `api_cache` включается в «Управление ресурсами → кэш приложения».

### Секция 5 — Очередь оффлайн-действий

- Таблица `offline_queue (id TEXT PK, action_type TEXT, instance_id TEXT, payload TEXT, status TEXT, error TEXT, created_at INTEGER, updated_at INTEGER)`.
- Модули: `state/offline_queue.rs` + `api/queue.rs` (theseus), плагин `plugin:queue` (`apps/app/src/api/queue.rs`).
- Функции: `enqueue`, `list`, `cancel`, `remove_done`, `flush`. Маппинг: `install_project → project_install`, `update_project → project_update`. Flush поочерёдно, эмитит события, FAILED/CANCELLED/installing/quarantine пропускает.
- Фронтенд:
  - `useConnectivity()` — `navigator.onLine` + проба `api.modrinth.com` + `plugin:auth|check_reachable`; на переходы → `plugin:app|set_offline` и `plugin:queue|flush`.
  - Перехват во `providers/content-install.ts`, `InstallToPlayModal`, `UpdateToPlayModal`: оффлайн → `enqueue` + уведомление.
  - UI: бейдж «N в очереди», панель очереди (список/статус/отмена/очистка), пункты видны в списке модов инстанса.
- Non-Goals v1: только install/update проектов; без переупорядочивания очереди.

## Тестирование и Non-Goals

- Rust unit: `AccountType` serde/sqlx roundtrip; `offline_queue` на in-memory SQLite; `connectivity`; парсинг ответов ely.by. Сетевые флоу — ручные/Playwright с тест-аккаунтом ely.by.
- Frontend vitest: `OfflineCacheFeature` (запись/чтение/exclude/не-GET/не-JSON), `useConnectivity`, маппинг очереди.
- Playwright-сценарии: OAuth-логин ely.by; оффлайн-запуск; просмотр из кэша; очередь + flush.
- Pre-PR: `pnpm prepr:frontend:app` + `cargo test`/`clippy` для `app-lib`/`app`. Labrinth и веб-фронтенд не трогаем.

Non-Goals (v1): оффлайн-аккаунты; редактирование скинов через ely.by-skin API; очередь шире instal/update; authlib-injector для Microsoft; изменения backend/веба; отдельный 2FA-экран (TOTP на сайте ely.by).

## Открытые вопросы

- Точный путь Yggdrasil-профиля ely.by для `online_profile()` — уточнить по докам при реализации.
- Версия/источник authlib-injector jar — зафиксировать при реализации (latest.json).