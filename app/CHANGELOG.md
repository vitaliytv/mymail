# Changelog

## [0.31.1] - 2026-09-25

### Fixed

- Збірки macOS DMG і Android APK запускаються на наявному Mac mini runner `macos-arm64` (як у foc) замість неіснуючої мітки `release-macos`, через яку реліз v0.31.0 не зібрався.

## [0.31.0] - 2026-09-25

### Changed

- DMG-реліз MyMail готується локально з Infisical signing secrets і публікується у Forgejo замість GitHub Actions.
- Локальний macOS реліз тепер створює лише ARM64 assets з оптимізованим Cargo release profile.
- Локальні macOS release assets тепер запускаються через `cargo xtask release-assets`.
- `cargo xtask release-assets` тепер зберігає версію релізу в source commit і тегу, а publisher не переписує Tauri config.
- Реліз переведено на модель foc: release PR із bump версії, тег після мержу, draft-реліз, збірка macOS DMG і Android APK у CI та публікація після завантаження всіх assets. Єдине джерело версії — `app/package.json`; плагіни отримують справжню версію застосунку.

### Fixed

- Локальна universal DMG-збірка самостійно встановлює обидва macOS Rust targets.
- Локальна DMG-збірка не запускає Finder-оформлення, яке не працює в non-interactive процесі.
- Локальний DMG-реліз додає підписаний macOS updater archive і Forgejo latest.json.
- Локальний DMG pipeline перевіряє цілісність образу без нестабільного монтування.
- Android release на Forgejo ARM64 використовує вже встановлений Android SDK замість застарілого пакета `tools`.

## [0.30.1] - 2026-08-20

### Changed

- Закріплено public OCI fixtures для повторюваної online/offline перевірки dependency graph.

## [0.30.0] - 2026-08-19

### Added

- Додано generic preflight для typed plugin Components
- Додано account-bound consent і crash-recoverable activation confirm для плагінів
- Додано exact typed invocation для встановлених Draft Helper і Booking Finder Components
- Додано preflight consent flow і exact typed actions у менеджері плагінів
- Додано exact OCI dependency graph, edge grants, offline confirm і quarantine GC для плагінів
- Додано локальну E2E matrix і посібник для авторів плагінів mlmail

## [0.29.0] - 2026-08-19

### Added

- Додано product-owned typed registry контрактів плагінів mlmail

## [0.28.0] - 2026-08-17

### Added

- Додано durable context для Gmail provider і Draft Helper з offline replay та emission gate

## [0.27.0] - 2026-08-17

### Removed

- Plugin Manager перейшов на n-plugin WebAssembly Components: legacy `mlmail-plugin-host`, ZIP/core-Wasm loader та JSON/string ABI видалено.

## [0.26.0] - 2026-08-17

### Added

- Додано exact account-scoped consent `mail:search` для Booking Finder

## [0.25.0] - 2026-08-16

### Fixed

- Дозволено E2E тесту Booking Finder приймати встановлений packaged Component

## [0.24.2] - 2026-08-14

### Changed

- Оновлено точний snapshot plugin platform `n-plugin`.

## [0.24.1] - 2026-08-14

### Changed

- Перенесено Rust workspace і lockfile в корінь monorepo.

## [0.24.0] - 2026-08-13

### Added

- Додано Draft Helper WebAssembly Component з typed Gmail draft creation.

## [0.23.0] - 2026-08-13

### Added

- Додано Booking Finder WebAssembly Component з typed Gmail search.

## [0.22.0] - 2026-08-13

### Added

- Registered the generated Gmail search WIT binding with the generic n-plugin Component runtime using exact WKG lock metadata.

## [0.21.0] - 2026-08-12

### Added

- Generated async Component Model binding for the product-local Gmail search stream.

## [0.20.0] - 2026-08-12

### Added

- Product-local Gmail search WIT contract and native paged users.messages.list host adapter.

## [0.19.1] - 2026-08-12

### Fixed

- Показ реальної причини помилки локальної LLM замість застарілої заглушки omlx.

## [0.19.0] - 2026-08-12

### Added

- Явні налаштування OpenAI-compatible локальної LLM з перевіркою endpoint і без legacy omlx fallback.

## [0.18.0] - 2026-08-06

### Changed

- A2UI з @7n/tauri-components замість vendored копії; bump пакета до ^0.17.0

## [0.17.0] - 2026-08-04

### Added

- Plugin Manager: native .n-plugin picker; detail A2UI panel у читачі

## [0.16.0] - 2026-08-04

### Changed

- MVP gaps: .n-plugin, signed sample install, draft via installed wasm

## [0.15.0] - 2026-08-04

### Added

- M6: Plugin Manager — consent, disable, uninstall purge

## [0.14.0] - 2026-08-03

### Added

- M5: draft.create через Wasm handle_action + audit (30d) у PluginSidebar

## [0.13.1] - 2026-08-03

### Fixed

- Case-insensitive пошук closing </head> для injected style/link-interceptor у Login.vue (раніше exact-match пропускав HTML з </HEAD> в іншому регістрі)

## [0.13.0] - 2026-08-03

### Added

- M4: A2UI sidebar — validated sample surface + PluginSidebar у читачі листа

## [0.12.0] - 2026-08-03

### Added

- Крейт mlmail-plugin-host: Gmail metadata host і grant-gated sample Wasm read_meta (M3)

## [0.11.5] - 2026-07-27

### Changed

- Панель "Правило для схожих листів" тепер постійна частина сторінки листа (під полем "Запитати про цей лист"), замість спливного вікна за кнопкою "Правило" — кнопку прибрано з нижньої панелі дій

## [0.11.4] - 2026-07-24

### Fixed

- Усунено desync bun.lock/package.json, через який build-desktop падав на `bun install --frozen-lockfile` і DMG для Apple Silicon не потрапляв у реліз v0.11.3

## [0.11.3] - 2026-07-23

### Changed

- Панель "Правило для схожих листів" тепер постійна частина сторінки листа (під полем "Запитати про цей лист"), замість спливного вікна за кнопкою "Правило" — кнопку прибрано з нижньої панелі дій

## [0.11.2] - 2026-07-23

### Fixed

- cargo fmt у gmail/message.rs (CI lint-rust debt)

## [0.11.1] - 2026-07-22

### Fixed

- Відновлено пряме omlx-чат API (createOpenAiChat/useOmlx), видалене з @7n/tauri-components у 0.11.0 — саммарі/переклад/ask/аналіз дзвінків не працювали через відсутній `app/src/omlx.js`. Локальна Rust-команда `omlx_config` читає `~/.omlx/settings.json` замість видаленої `plugin:agent|omlx_config`.

## [0.11.0] - 2026-07-20

### Changed

- Оновлення `@7n/tauri-components` до `^0.11.1`

## [0.11.0] - 2026-07-19

### Changed

- `@7n/tauri-components` ^0.8.0 → ^0.11.1: `useAgent` → `useAcpAgent` (CODEX/cursor/pi presets); one-shot omlx helpers вендорені в `app/src/omlx.js` після видалення `createOpenAiChat`/`useOmlx` з пакета

## [0.10.0] - 2026-07-12

### Added

- Показувати список вкладених файлів у листі, приховуючи inline-зображення з підписів
- Клік на завдання у панелі "Відкриті завдання" відкриває відповідний лист
- Клік на вкладення завантажує файл і відкриває його програмою за замовчуванням ОС

### Fixed

- Список відкритих завдань більше не показує листи, переміщені в кошик

## [0.9.0] - 2026-07-12

### Fixed

- Fix cross-platform clippy break: gate macOS-only badge-count code and its Manager import so Linux CI compiles clean

## [0.8.0] - 2026-07-12

### Fixed

- Fix remaining CI red: Lint Rust working-directory, cspell words, .vscode/settings.json gitignore conflict, dedup gmail POST helper and Vue template-editor fields

## [0.7.0] - 2026-07-12

### Fixed

- Fix pre-existing eslint/oxlint errors across the codebase (JSDoc, static regex, no-alert, etc.)

## [0.6.0] - 2026-07-12

### Fixed

- Filters dialog: show all Gmail filter conditions, hide raw id, add search and per-filter action icons

## [0.5.2] - 2026-07-11

### Changed

- У діалозі «Правило для схожих листів»: поки локальна LLM підбирає тему, замість кружечка одразу показується хрестик, що скасовує підбір і очищає поле теми. Кнопка «Видалити всі такі» тепер формує Gmail-запит лише за відправником, без теми — щоб масове видалення не залежало від підказаної LLM фрази.

## [0.5.1] - 2026-07-11

### Fixed

- Автооновлення насправді ніколи не показувало діалог: перевірка знаходила нову версію й навіть завантажувала її, але виклик `$q.dialog(...)` падав з `TypeError: e.dialog is not a function` — Quasar-плагін `Dialog` не був підключений у `main.js` (лише `Notify`). Помилка тихо ловилась у catch механізму оновлення й ніде не показувалась користувачу.

## [0.5.0] - 2026-07-06

### Changed

- Додано діалог для перегляду та видалення фільтрів Gmail

## [0.4.5] - 2026-07-06

### Fixed

- Виправлено визначення кодування HTML/plain-text листів: помилково задекларований легасі charset (напр. windows-1251) більше не спричиняє мojibake, якщо сирі байти насправді валідний UTF-8

## [0.4.4] - 2026-07-05

### Fixed

- Android-білд падав ("Permission updater:default not found") — updater-плагін не реєструється на Android/iOS, тож дозвіл винесено в окрему capability з `platforms: [macOS, windows, linux]`.

## [0.4.3] - 2026-07-05

### Changed

- Локальний use-updater.js замінено на спільний useUpdater() з @7n/tauri-components/vue (0.8.0) — та сама логіка, тепер в одній копії для mlmail/myshare/myllm/task.

## [0.4.2] - 2026-07-05

### Added

- Автоматичний повторний запит до Gmail кожні 15 секунд при мережевій помилці (kind: Network), з нотифікацією користувача про кожну спробу — замість негайного відображення помилки "Не вдалося з'єднатися з Google".

### Fixed

- Автооновлення не працювало: у capabilities/default.json бракувало дозволу `updater:default`, тож `check()` завжди падав з permission-denied ще до мережевого запиту (тихо ловилось у catch, консоль недоступна в релізній збірці). Додано дозвіл.

## [0.4.1] - 2026-07-05

### Changed

- release: app@0.4.0

## [0.4.0] - 2026-07-05

### Added

- Аналіз дзвінків: новий компонент AuditAnalysisDialog.vue з use-call-analysis.js для аналізу записів розмов, backend-сервіс call_analysis.rs у Tauri
- Розширена каталогізація інструментів у tool/catalog.js

### Changed

- Оновлено NewsletterView.vue й TasksPanel.vue для інтеграції з новим аналізом

## [0.3.0] - 2026-07-05

### Added

- Перезапуск у нову версію одразу після встановлення оновлення (relaunch), періодична перевірка оновлень щогодини, логування помилок апдейтера

### Fixed

- Апдейтер не запускається в dev-режимі: версія dev-збірки завжди 0.1.0, тож перевірка помилково пропонувала «оновитись» до опублікованого релізу

## [0.2.0] - 2026-07-03

### Added

- Правило для схожих листів: на картці листа кнопка «Правило» відкриває панель із полями Від (email)/Тема (стабільний префікс підказує локальний LLM, редаговано) і двома діями — «Видалити всі такі» (gmail_trash_query: пагінований збір усіх збігів + batchModify TRASH батчами ≤1000) та «Створити фільтр» (gmail_create_filter: Gmail-фільтр з action addLabelIds TRASH/removeLabelIds INBOX). Новий OAuth-скоуп gmail.settings.basic (потрібен повторний логін). Tool-каталог: trash_query (destructive), create_filter (write).
- Двоколонковий рідер листа: ліворуч оригінал (Від/Тема/Дата + тіло), праворуч резюме українською від локального LLM (omlx) — composables/use-summary.js + pure services/summary.js (buildSummaryPrompt). Резюме автоматично оновлюється при зміні листа (watch на currentMessage) із захистом від застарілих відповідей; на недоступній моделі показує банер-підказку. Колонки адаптивні (col-12 col-md-6).
- Панель задач (tasks panel): use-task-scan.js сканує вхідні на предмет задач, use-ask.js для швидких запитів до LLM, нові компоненти в src/components/
- Версія застосунку відображається в заголовку вікна (mlmail vX.Y.Z)
- Шаблони для новинних розсилок (newsletter): JSON-шаблони пакуються в бінарник як bundle-ресурс, use-newsletter-render.js рендерить лист за шаблоном, TemplatesManager.vue керує списком

### Fixed

- Авто-оновлення: увімкнено bundle.createUpdaterArtifacts — релізи тепер публікуватимуть latest.json і .sig
- Резюме/переклад листа: fetch з abort-таймаутом 60с (omlx міг зависати без відповіді) і зменшений розмір батчу перекладу (80→15) для стабільної якості й швидкості

Усі помітні зміни цього пакета документуються тут.

Формат — [Keep a Changelog](https://keepachangelog.com/uk/1.1.0/), нумерація — [SemVer](https://semver.org/lang/uk/).

## [0.1.6] - 2026-06-15

### Added

- Tool Surface (`n-tool-surface`) у `app/src/tool/`: спільний каталог інструментів (`catalog.js`) зі схемами, `dispatch.js` з уніфікованим конвертом `{ ok, output } / { ok:false, error:{ code, message, kind } }`, `manifest.js` (OpenAI function-calling для майбутнього LLM-агента), `transports.js` (Tauri-invoke) та `index.js`. Тести `tool.test.js`.

### Changed

- `auth-store.js` переведено з прямих `invoke` на `dispatch` для read/action-команд (`is_authenticated`, `current_email`, `inbox_count`, `random_message`, `random_newsletter`, `unsubscribe`); lifecycle/secret (`login`, `logout`, `getAccessToken`) лишаються прямими invoke. Конверт зберігає backend-`kind`, тож ReauthRequired-логіка не змінилась.
- Тестовий стек переведено з `bun test` + happy-dom preload на **Vitest** + happy-dom (канон `n-test`/`n-vue`). `vitest.config.mjs` через `mergeConfig` повторно використовує `vite.config.js` (Vue/VueMacros/AutoImport/Quasar), тож SFC-компіляція й авто-імпорти працюють нативно — preload-хаки прибрано. Тест-файли переписано з `bun:test` (`mock`/`mock.module`) на `vitest` (`vi`/`vi.hoisted`/`vi.mock`). `stryker.config.mjs` — на `vitest-runner` з `perTest`-аналізом покриття.

### Removed

- Тестовий preload `test/happy-dom.preload.js` і залежності `@happy-dom/global-registrator`, `@types/bun`, `@vue/compiler-sfc` (більше не потрібні з Vitest).

### Fixed

- Логін з ненастроєними Google OAuth credentials більше не падає з плутаним `OAuth`-помилкою: `run_login` (macOS/Android) перевіряє client_id/secret через `require_configured` ще до старту флоу й повертає новий `ConfigMissing(<env-var>)` → UI показує «Google OAuth не налаштовано: заповніть credentials у .env / .env.secret.». `is_real_client_id` тепер відхиляє і порожній/пробільний рядок (раніше порожній id вважався «справжнім»).

## [0.1.5] - 2026-05-26

### Changed

- Оновлено форматування JS/Vue джерел, тестового preload та Vite-конфігу для узгодження з поточними tooling-перевірками.

## [0.1.4] - 2026-05-26

### Changed

- Посилено JS-тести auth store та нормалізацію повідомлень помилок для кращого mutation coverage.

### Fixed

- Оновлено конфіги lint/coverage, щоб ігнорувати локальні артефакти Stryker і worktree-копії під час перевірок.

## [0.1.3] - 2026-05-22

### Added

- Мутаційне тестування: скрипти `test:mutation` (StrykerJS, JS/Vue) та `test:rust:mutation` (`cargo mutants`, Rust).
- `stryker.config.mjs` — конфігурація StrykerJS: command-runner на `bun test`, `inPlace: true`, мутує `src/**/*.{js,vue}` мінус тести та `main.js`, звіт у `reports/stryker/mutation.json`.

### Fixed

- Порожнє тіло листа для частини повідомлень: Gmail повертає `body.data` як base64url із `=`-паддингом, який код раніше відхиляв. Декодування зроблено толерантним до паддингу, а байти тепер конвертуються в текст за `charset` із заголовка MIME-частини (через `encoding_rs`, fallback — UTF-8), тож листи в ISO-8859-1, windows-1251 тощо більше не показують порожнє тіло.

## [0.1.2] - 2026-05-21

### Added

- Скрипт `test:coverage` (`bun test --coverage`) для звіту покриття JS.
- Скрипт `test:rust:coverage` (`cargo llvm-cov`) для звіту покриття Rust.
- Тести для `App.vue`.

### Changed

- `test-utils/quasar.js`: додано хелпер `mountQuasar` для монтування компонентів із власним layout.

## [0.1.1] - 2026-05-21

### Added

- Тестовий preload `test/happy-dom.preload.js`: реєструє happy-dom як DOM-середовище, компілює `.vue` SFC через Bun-плагін на `@vue/compiler-sfc`, віддає авто-імпорти Vue / Vue Router як глобальні змінні і підміняє `quasar` на browser-збірку.
- Залежності `@happy-dom/global-registrator`, `@vue/compiler-sfc`, `@types/bun`.

### Changed

- Компонентні тести переведено з Vitest на Bun Test Runner + happy-dom.
- `Login.vitest.js` перейменовано на `Login.test.js` і переписано під `bun:test` (`mock` / `mock.module`).
- `mountWithQuasar` реєструє всі Quasar-компоненти глобально (немає `@quasar/vite-plugin` під `bun test`).
- Скрипт `test` запускає `bun test` для всього `src`; додано `test:watch`.

### Removed

- Залежності `vitest` і `jsdom`, блок `test` із `vite.config.js`, скрипт `test:ui`.
