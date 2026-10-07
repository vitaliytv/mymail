# WASM-плагінна платформа з A2UI

**Дата:** 2026-08-02  
**Оновлено:** 2026-08-03  
**Статус:** погоджено — готово до реалізації MVP  
**Зв'язані документи / references:**

- [A2UI v1.0 Candidate](https://a2ui.org/specification/v1.0-a2ui/) — обрана лінія протоколу для MVP
- [A2UI v0.9.1 Current](https://a2ui.org/specification/v0.9.1-a2ui/) — upstream production recommendation; не використовуємо в MVP
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
- [WIT](https://component-model.bytecodealliance.org/design/wit.html)
- Репозиторії: `nitra/tauri-components` (platform), `vitaliytv/mlmail` (mail host), `nitra/task` (task host)

## Glossary

| Термін | Значення |
| --- | --- |
| Host | Tauri-застосунок (`mlmail`, `task`), який завантажує плагіни |
| Platform core | Спільні crates у `tauri-components` (runtime, package, permissions, A2UI adapter) |
| Domain host | Продуктовий adapter (`mlmail-plugin-host`, `task-plugin-host`) з WIT imports і catalog |
| Capability | Іменована дія, яку плагін може виконувати лише після user grant |
| Scope | Обмеження capability на ресурс (message, account, surface…) |
| Surface | Host-owned UI slot (sidebar, detail, modal), куди рендериться A2UI |
| Catalog | Дозволений набір Vue-компонентів і їх props/events у host |
| Grant | Збережена згода user на capability+scope для plugin identity |
| Invocation | Один запуск Wasm instance (render або handle-action) |
| `.n-plugin` | Підписаний архів плагіна |

---

## 1. Проблема / Мета

`mlmail` і майбутні Tauri-застосунки, зокрема `task`, мають дозволяти користувачам встановлювати плагіни для власних сценаріїв без випуску нової версії host-застосунку. Плагіни виконують ізольовану логіку, отримують тільки явно надані доменні можливості й можуть показувати інтерактивні панелі, що залишаються стилістично та безпеково під контролем host-застосунку.

Спільна platform-логіка належить репозиторію `nitra/tauri-components`. Доменні інтеграції залишаються в `vitaliytv/mlmail` (листи) та `nitra/task` (задачі).

### 1.1. Goals (MVP)

- Local install підписаного `.n-plugin`
- Scoped capabilities з explicit user consent
- On-demand A2UI panels (sidebar / detail / modal) без guest HTML/JS/webview
- Mutating actions з audit log
- Disable / uninstall без впливу на стабільність host

### 1.2. Non-goals (MVP)

- Marketplace, remote review pipeline, publisher reputation (trust policy marketplace — відкладено, §10)
- Background email/task event triggers, scheduler, dry-run, event replay
- `mail:send`, raw HTTP з плагіна, filesystem, credential/secret bytes
- `mail:content.read` зі scope `account` (лише `message`)
- Довільний executable UI у пакеті плагіна (custom JS/HTML/native code)
- Multi-tenant / remote plugin hosting
- Browser-extension-рівень process sandbox поза Wasmtime limits
- Cross-plugin communication

---

## 2. Ухвалені рішення

| # | Питання | Рішення | MVP / Post |
| --- | --- | --- | --- |
| A | Runtime | `Wasmtime` embedded у Rust. Кожен плагін — WebAssembly Component; API — WIT. | MVP |
| Б | Межа репозиторіїв | Platform core у `tauri-components`. Кожен продукт — власний domain host adapter і catalog extensions. | MVP |
| В | UI плагінів | Плагін повертає декларативні A2UI повідомлення. Vue host рендерить зі свого catalog; без HTML/JS/webview у плагіні. | MVP |
| Г | Розповсюдження | Єдиний пакет `.n-plugin` для local install і майбутнього marketplace. | MVP local / Post market |
| Ґ | Permissions | Manifest декларує capabilities; доступ дає user у host UI. Немає прямого Tauri API, FS, мережі, secrets. | MVP |
| Д | A2UI | **v1.0 Candidate** + жорсткий schema pin у `plugin-a2ui` і поле `a2ui` у manifest. Contract tests обовʼязкові. | MVP |
| Е | Trust (local) | Ed25519 підпис обовʼязковий; TOFU ключа publisher; debug-only `--dev-unsigned` у non-release builds. | MVP |
| Є | Host I/O | WIT domain imports виконує host; час очікування host **не** споживає guest fuel; nested invocation того ж plugin заборонений. | MVP |
| Ж | Scope contract | Capability grants мають формальний Scope object (див. §4.4). | MVP |
| З | Signing toolchain | Власний `plugin-dev-cli sign`; private keys автора в OS keychain (macOS Keychain / Windows DPAPI / Linux secret service); trusted **public** keys — у app data host. | MVP |
| И | `mail:content.read` | У MVP лише scope `message` (least privilege). Scope `account` — поза MVP. | MVP |
| І | Grant persistence | Grants привʼязані до `plugin_id` + capability/scope (+ user); живуть між версіями, доки caps/scopes не escalate і publisher key не змінився. | MVP |
| Ї | Audit retention | Локально **30 днів**; без body листів; лише metadata action. User purge дозволений раніше. | MVP |
| Й | Resource defaults | Placeholders зараз: **32 MiB** memory, **50M** fuel, **2 s** wall-clock, **2** concurrent invocations/plugin. Обовʼязковий benchmark gate у **M2** перед merge в main; після spike — перегляд цифр. | MVP |
| К | Uninstall | Одразу purge package + surfaces + **settings + grants**. | MVP |

---

## 3. Threat model

### 3.1. Assets

- Вміст листів / задач і metadata
- Draft content і mutating side effects
- OAuth/credentials (ніколи не в guest memory)
- Host UI integrity і design system
- Audit trail

### 3.2. Actors

| Actor | Приклад |
| --- | --- |
| Benign plugin author | Корисний tools-плагін |
| Malicious plugin | Навмисний ексфільтратор / confuse-deputy |
| Compromised publisher key | Підписаний зловмисний update |
| Curious user | Over-broad grants |
| Supply-chain tamper | Змінений `.n-plugin` у transit |

### 3.3. Controls (MVP)

- Wasm Component + WIT: немає raw FS/net/Tauri з guest
- Deny-by-default capabilities + scoped grants
- A2UI catalog allowlist; unknown component/prop = error
- Signature + checksum перед install
- Fuel / memory / epoch timeout / output size limits
- Host валідує action + capability **до** `handle-action`
- Audit для mutating operations (30-денна локальна retention)
- Circuit breaker на repeated failures

### 3.4. Residual risks

- TOFU: user може довірити зловмисний ключ при першому install
- Candidate A2UI v1.0 semantics можуть змінитись до Stable (мітигується pin + adapter)
- Host bugs у domain imports = confused deputy (мітигується scope checks у host, не в guest)
- Довгоживучі grants між версіями: escalate/key-change вимагають re-consent; silent behavior change в межах тих самих caps — ризик продукту/ревʼю плагіна

---

## 4. Деталі реалізації

### 4.1. Пакети та ownership

`tauri-components` створює product-agnostic Rust crates:

```text
plugin-runtime/       Wasmtime Engine, component loading, WIT bindings, invocation lifecycle
plugin-manifest/      parse/validate plugin.toml, SemVer compatibility, package metadata
plugin-permissions/   capability grant, scope validation, consent model, audit decisions
plugin-package/       .n-plugin archive, checksum, signature verification, install/rollback
plugin-a2ui/          A2UI v1.0 adapter (pinned schema revision), surface state, action validation
plugin-dev-cli/       build, validate, package, sign та local install для authors
```

`mlmail` містить `mlmail-plugin-host` (WIT imports для листів + дозволені UI surfaces).  
`nitra/task` містить `task-plugin-host`.  
Жоден domain crate не переноситься до `tauri-components`.

#### Compatibility matrix (нормативний намір)

| Шар | Версіонування | Хто ламає / релізить |
| --- | --- | --- |
| Platform crates (`plugin-*`) | SemVer crates | `tauri-components` |
| Platform WIT world | SemVer у manifest `platform` | `tauri-components` |
| Domain WIT (`nitra:mail`, `nitra:task`) | окремий SemVer | відповідний product repo |
| Host app | app SemVer; фіксує версії platform crates | `mlmail` / `task` |
| Plugin package | plugin SemVer + required ranges | plugin author |

Breaking WIT → major bump + fixtures «old plugin / new host» і навпаки в CI `tauri-components`; consumer smoke в `mlmail`.

### 4.2. WIT contracts

Platform WIT contract містить:

- plugin identity, lifecycle (`activate`, `deactivate`), declared actions і event subscriptions;
- delivery доменного context лише як typed records, без database IDs або внутрішніх structs;
- `render(surface, context) -> a2ui-message-stream`;
- `handle-action(action, context) -> plugin-result`;
- settings read/write через host-managed schema;
- typed failure categories: `denied`, `invalid-input`, `timeout`, `unavailable`, `plugin-error`.

Domain WIT packages мають окремі версії: `nitra:mail` і `nitra:task`. Плагін у manifest визначає сумісний SemVer range platform і domain packages. Host відхиляє install при несумісності.

#### Invocation semantics (нормативно)

1. Один invocation = один `Store` / instance.
2. Domain WIT imports викликає **host**; вони можуть блокувати worker, але **не** витрачають guest fuel.
3. Guest fuel рахує лише виконання Wasm guest.
4. Wall-clock epoch cancellation покриває і guest, і зависання очікування host (з окремим host-timeout budget).
5. **Заборонено** nested invocation того ж plugin (напр. `handle-action` → host → `render` того ж instance). Потрібний re-render планує host **після** завершення поточного invocation.
6. Максимальна глибина host→guest callbacks у межах одного user gesture = 1 (action → optional scheduled render).

Executor: Wasm invocations на background worker (не UI thread). UI отримує вже провалідований A2UI stream.

Latency SLO (ціль, уточнити spike-ом у M2): render p95 cold < 150ms, warm < 40ms на sample plugin без важкого I/O.

### 4.3. Manifest і пакет

Формат archive: `.n-plugin`.

```text
plugin.toml
component.wasm
settings.schema.json
changelog.md
signature.ed25519
checksums.sha256
```

`plugin.toml` містить immutable `id`, name, version, publisher, `a2ui` protocol pin (`1.0` + `schema_rev`), required platform/domain API ranges, declared capabilities (+ requested scopes), registered surfaces, event subscriptions і settings schema reference.

Installation перевіряє: archive layout → checksums → signature → trust policy → compatibility → capability declaration → запис у local registry.

Installer приймає package URL або локальний файл однаково. Marketplace (Post-MVP) додає publisher identity, review metadata, download URL, ключ підпису та release history **без** нового install path; trust policy marketplace — §10.

#### Trust MVP (local)

| Режим | Поведінка |
| --- | --- |
| Release host | Потрібен валідний Ed25519 підпис; ключ має бути trusted |
| Перший publisher key | TOFU: user бачить fingerprint і підтверджує trust |
| Відомий publisher | Підпис перевіряється проти trusted public-key store (app data) |
| Зміна publisher key для того ж `id` | Re-consent + warning |
| Debug / dev builds | `--dev-unsigned` дозволений тільки поза release channel |
| Author signing | `plugin-dev-cli sign`; private key в OS keychain |

### 4.4. Capability model і Scope DSL

Capability — маленька дія з explicit scope.

Initial `mlmail` API (MVP):

| Capability | Scope kinds (MVP) | Notes |
| --- | --- | --- |
| `mail:metadata.read` | `message`, `account` | без body |
| `mail:content.read` | `message` only | body; `account` — поза MVP |
| `mail:draft.create` | `account` | mutating → audit |
| `ui:surface.register` | `surface` = manifest-declared id | без grant surface не монтується |

Поза MVP: `mail:send`, raw HTTP, filesystem, credential bytes, `mail:content.read`/`account`. Майбутній HTTP — лише як opaque host operation після allowlist + grant.

#### Scope object (нормативний JSON shape у grant store / consent / audit)

```json
{
  "capability": "mail:content.read",
  "resource_kind": "message",
  "resource_id": "msg_123",
  "optional_constraints": {}
}
```

Правила:

- Deny-by-default: відсутній grant → `denied`.
- Host enforce scope на межі WIT import (не довіряти guest).
- Consent UI показує human-readable scope (напр. «Читати вміст цього листа»).
- Escalation (нова capability, ширший `resource_kind`, новий publisher key) → обовʼязковий re-consent.
- Update з **тими самими** capabilities/scopes і тим самим publisher key → grants **зберігаються** (привʼязка до `plugin_id`, не до version).

Grant identity key: `(plugin_id, user_id, capability, scope)`.

Mutating operation → audit: `plugin_id`, `plugin_version`, `action_id`, `capability`, `scope`, `result`, `correlation_id`, timestamp. Retention: **30 днів** локально, без body; user може purge раніше.

### 4.5. Runtime isolation

`plugin-runtime` створює shared `Wasmtime::Engine`, окремий `Store`/instance на invocation.

Ліміти на кожен запуск (placeholders до M2 benchmark):

| Ліміт | Placeholder |
| --- | --- |
| memory | 32 MiB |
| guest fuel | 50_000_000 |
| wall-clock (epoch) | 2 s |
| max concurrent invocations / plugin | 2 |
| max input / A2UI output / log event size | визначити в M2 разом із benchmark |

Timeout, out-of-fuel і trap завершують **тільки** поточний invocation. Repeated failures → circuit breaker; host показує діагностику; запуск після manual retry або backoff expiry.

Dev builds: feature-flag override лімітів дозволений. Перед merge M2 у main — обовʼязковий benchmark gate на sample plugin; цифри оновлюються за результатами spike.

### 4.6. A2UI v1.0 renderer

Плагін генерує лише A2UI **v1.0** `surface` tree, data updates і actions у pinned schema revision. `plugin-a2ui` валідовує:

- protocol version = `1.0` / pin
- schema
- surface ownership
- payload size
- catalog references

до передачі в Vue renderer.

Host має базовий Vue catalog у `tauri-components` і розширює domain components у product repos. Catalog визначає props, bindings, events, design tokens. Невідомий component/prop = validation error, не fallback HTML.

MVP surfaces: sidebar panel, email/task detail panel, modal workflow.

User interaction → typed A2UI `actionResponse` → host валідовує action + capability → `handle-action` у Wasm. Плагін не читає DOM, не викликає browser API / Tauri commands.

Складні chart/editor/map — лише як vetted host catalog components із versioned schema.

Pin strategy:

1. Vendor snapshot schema files у `plugin-a2ui/schemas/1.0/`.
2. Manifest: `a2ui = { protocol = "1.0", schema_rev = "<hash-or-date>" }`.
3. Mismatch pin ↔ host adapter → install/load reject.
4. Upgrade protocol — окремий major platform change + migration notes.

### 4.7. UI та lifecycle у host

Plugin manager показує: source, publisher, version, fingerprint ключа, requested/granted permissions+scopes, compatibility, `changelog.md`, health/circuit state.

Перед install/update — diff capabilities/scopes. Failure runtime/A2UI → safe error state в panel.

Events: MVP = manual action + on-demand render. Background triggers / scheduler / dry-run / replay — Post-MVP (окремий дизайн).

#### Lifecycle

| Подія | Поведінка |
| --- | --- |
| install | verify → consent capabilities → registry → optional activate |
| update (same caps, trusted key) | replace package; **grants зберігаються**; показати changelog |
| update (escalation / new key) | re-consent обовʼязково |
| disable | stop invocations; keep package + grants + settings |
| uninstall | remove package + surfaces; **одразу purge settings + grants** |
| rollback | previous package version якщо ще в local cache і compatible |
| circuit open | no auto-run; user retry / backoff |

### 4.8. Error taxonomy → UI

| Failure | User-facing (орієнтир) | Retry |
| --- | --- | --- |
| `denied` | Немає дозволу / звузьте scope | Request permission |
| `invalid-input` | Плагін або host передав некоректні дані | No auto |
| `timeout` | Плагін не відповів вчасно | Retry |
| `unavailable` | Доменний сервіс недоступний | Retry later |
| `plugin-error` | Помилка плагіна | Retry / disable |
| validation (A2UI/catalog) | Плагін повернув недопустимий UI | Disable / update |
| trap / OOM / out-of-fuel | Плагін аварійно зупинено | Circuit / retry |

### 4.9. Тести й acceptance criteria

У `tauri-components`:

- WIT binding tests і compatibility fixtures (old/newer plugins)
- package tests: invalid manifest, tampered checksum, invalid signature, unsigned-in-release reject, downgrade/rollback
- trust tests: TOFU accept/reject, key rotation re-consent
- capability tests: deny-by-default, scope enforcement, escalation re-consent, grants survive version bump without escalation
- Wasmtime tests: fuel exhaustion, timeout, memory breach, trapped guest, concurrent isolation, no nested invocation
- A2UI v1.0 contract fixtures: valid surface, invalid catalog ref, invalid action payload, pin mismatch
- Vue renderer tests: host tokens, unrecognized content not rendered
- M2 benchmark gate: sample plugin within placeholder budgets (або оновлені після spike)

У `mlmail`:

- integration: sample plugin читає metadata і створює draft
- UI: sidebar/detail A2UI surface
- audit assertion для mutating action; retention/purge behavior
- без `mail:content.read` → body не віддається
- `mail:content.read` з `account` scope → reject у MVP

**MVP done when:** user може local-install signed sample plugin, надати scoped `mail:metadata.read` і `mail:draft.create`, побачити A2UI панель, виконати draft action, вимкнути/видалити plugin (з purge settings/grants) без впливу на host.

### 4.10. MVP milestones (порядок імплементації)

| # | Milestone | Owner repo | Exit criteria |
| --- | --- | --- | --- |
| M1 | Package parse/verify/TOFU install + `plugin-dev-cli sign` | `tauri-components` | CLI install local signed package into registry |
| M2 | Empty Wasm hello lifecycle + **benchmark gate** | `tauri-components` | activate/deactivate + fuel/timeout tests; budgets confirmed or updated |
| M3 | Mail WIT read-only metadata | `mlmail` + platform | sample plugin reads metadata under grant |
| M4 | A2UI v1.0 sidebar render | both | validated surface in mlmail UI |
| M5 | Draft action + audit (30d retention) | `mlmail` | mutating path + audit assertion |
| M6 | Manager UX (consent diff, disable, uninstall purge) | `mlmail` | acceptance checklist green |

---

## 5. Appendix A — приклади контрактів (ілюстративні)

### 5.1. `plugin.toml` (фрагмент)

```toml
id = "com.example.mail-draft-helper"
name = "Draft Helper"
version = "0.1.0"
publisher = "example"
publisher_key_id = "ext_example_2026"

[a2ui]
protocol = "1.0"
schema_rev = "REPLACE_WITH_PINNED_HASH"

[requires]
platform = "^0.1"
"nitra:mail" = "^0.1"

[[capabilities]]
name = "mail:metadata.read"
resource_kinds = ["message"]

[[capabilities]]
name = "mail:draft.create"
resource_kinds = ["account"]

[[surfaces]]
id = "sidebar.draft-helper"
kind = "sidebar"

[[surfaces]]
id = "detail.draft-helper"
kind = "email-detail"
```

### 5.2. Sequence (happy path)

```text
User → Host: install package
Host → Package: verify checksum + signature + TOFU/trust
Host → User: consent capabilities/scopes
User → Host: grant
Host → Registry: persist plugin + grants

User → Host: open message / open sidebar
Host → Wasm: render(surface, context)
Wasm → Host: A2UI v1.0 message stream
Host → plugin-a2ui: validate pin/schema/catalog
Host → Vue: render

User → Vue: click action
Host: validate action + capability + scope
Host → Wasm: handle-action(action, context)
Wasm → Host: plugin-result (e.g. draft fields)
Host → Domain: create draft (audited)
Host: schedule render refresh (new invocation)
```

### 5.3. A2UI v1.0-shaped message (псевдо; точний schema = pinned rev)

```json
{
  "createSurface": {
    "surfaceId": "sidebar.draft-helper",
    "catalogId": "nitra.core"
  }
}
```

```json
{
  "updateComponents": {
    "surfaceId": "sidebar.draft-helper",
    "components": [
      {
        "id": "root",
        "component": "Column",
        "children": ["title", "run"]
      },
      {
        "id": "title",
        "component": "Text",
        "text": "Draft Helper"
      },
      {
        "id": "run",
        "component": "Button",
        "text": "Create draft",
        "action": { "name": "create-draft" }
      }
    ]
  }
}
```

---

## 6. Відкриті технічні follow-ups

- Після M2 benchmark — за потреби оновити resource placeholders у §2 Й / §4.5.
- Окремий дизайн Post-MVP: background triggers, scheduler, dry-run, replay.
- Publisher key rotation UX copy і recovery flow.
- Vendor A2UI v1.0 schema files і зафіксувати `schema_rev` hash.
- Marketplace trust policy — §10 (відкладено).

---

## 10. Відкладені рішення

### D2. Marketplace trust policy (Post-MVP)

Відкладено свідомо. Для MVP достатньо local TOFU (§2 Е, З).

Коли повертатись — варіанти на вибір:

| Варіант | Опис |
| --- | --- |
| **D2-A** | Централізований Nitra registry ключів + revocation list |
| **D2-B** | Transparency log (append-only) + user/org pins |
| **D2-C** | Лише user-managed keys (як TOFU), marketplace тільки CDN |

---

## Changelog документа

| Дата | Зміна |
| --- | --- |
| 2026-08-02 | Початкова версія |
| 2026-08-02 | Review pass: non-goals, threat model, trust MVP, scope DSL, invocation semantics, lifecycle, milestones, appendix, §10 decisions |
| 2026-08-03 | Ухвалено D1-A, D3-C, D4-A, D5-A, D6-A, D7-A+C, D8-A; D2 відкладено; статус → готово до MVP |
