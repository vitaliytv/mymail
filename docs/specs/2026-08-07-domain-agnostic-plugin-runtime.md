# Domain-agnostic plugin runtime через Dynamic WIT registry

**Дата:** 2026-08-07
**Оновлено:** 2026-08-09
**Статус:** архітектурні рішення погоджено; наступний крок — M0 Component Model spike
**Зв'язані документи:** `docs/specs/2026-08-02-wasm-a2ui-plugin-platform.md`,
`docs/adr/20260808-211238-n-plugin-monorepo.md`

## 1. Мета

Побудувати спільну plugin platform для Vue/Tauri-застосунків, де:

- новий застосунок або product domain не потребує зміни `nitra/tauri-components`;
- product repository володіє власними WIT contracts, Rust handlers і capability policy;
- плагіни є WebAssembly Components із compile-time typed imports та exports;
- встановлений плагін може залежати від інших встановлених Wasm plugins;
- `wkg` є єдиним authoritative engine для WIT/package version resolution,
  fetch, content verification і portable lock, а `n-plugin` активує exact
  resolved graph без власного version selector;
- додавання плагіна, який використовує вже зареєстровані interfaces і triggers, не
  потребує перекомпіляції застосунку;
- application update перевіряє майбутню сумісність плагінів до встановлення.

Це breaking migration. Legacy core-Wasm `Module`, ручний pointer/length ABI,
JSON/string transport і dual-loader не підтримуються.

## 2. Межі M0

M0 навмисно оптимізований для перевірки Component Model architecture, а не для
запуску довільного неперевіреного коду.

### 2.1. Входить до M0

- Bytecode Alliance WebAssembly runtime 48 LTS після його офіційного stable release;
- WebAssembly Component Model і Canonical ABI;
- generated typed host registration;
- Rust toolchain для авторів плагінів навколо `wkg`;
- required plugin dependencies різних publishers;
- native experimental async Component Model features;
- immutable activation graphs, SQLite metadata і filesystem CAS;
- capability grants, consent, layered deny policy та structured audit;
- Vue A2UI integration;
- application/plugin compatibility preflight.

### 2.2. Не входить до M0

- package signatures, publisher-key verification або package signing frameworks;
- authenticity чи ownership proof для `publisher_id`;
- memory, fuel, CPU, deadline, concurrency, task, stream, state або result quotas;
- optional dependencies;
- lifecycle WIT;
- state migrations і rollback snapshots;
- compatibility з legacy plugin ABI;
- guest languages, крім Rust authoring toolchain.

`publisher_id` у M0 є декларативною identity, а digest перевіряє content address
і пошкодження. Configured package sources вважаються trusted. Така конфігурація
не є безпечною для довільних сторонніх plugins і повинна мати явну позначку
`experimental/trusted-only`.

Tauri application updater може зберігати власну стандартну перевірку update
artifact. Відкладення plugin package signing не скасовує перевірку самого
application installer.

## 3. Архітектурні межі

```text
Wasm Component plugin
  │ typed WIT imports/exports
  ▼
PluginRuntime у nitra/n-plugin
  │ generic lifecycle, graph, grants, audit, A2UI
  ▼
PluginHostInterfaceRegistry у product application
  │ generated typed registration
  ▼
Product-owned Rust handlers
  │ host-owned credentials і services
  ▼
Gmail, task storage або інший product backend
```

### 3.1. Відповідальність `nitra/n-plugin`

Platform core знає лише generic concepts:

- package, release, dependency edge та activation graph;
- Component compilation, composition, instantiation і invocation;
- grants, deny authorities, audit і plugin state;
- CAS, SQLite registry, downloads, updates і garbage collection;
- Vue Plugin Manager, A2UI renderer і dependency slots;
- public registration contract для interfaces та triggers.

Core не містить `mail:*`, `task:*`, Gmail types, product secrets або product
handlers. Новий domain не додає `match` branch чи feature flag у platform core.

### 3.2. Product-owned host interfaces

Кожен застосунок має власний `PluginHostInterfaceRegistry`. Лише trusted
application root може реєструвати interfaces; plugins і dependencies не можуть
мутувати registry.

Product crate генерує typed runtime bindings і registration code з WIT.
Застосунок підключає їх через API на кшталт:

```rust
PluginRuntimeBuilder::<AppTriggerSet>::new()
    .register_host_interfaces(product_plugin_interfaces())
    .register_triggers(AppTriggerSet::inventory())
    .build(app_handle)
```

Конкретна Rust форма API уточнюється M0 spike, але контракт має лишатися
generated і typed. Type-erased JSON dispatcher не допускається.

Installing a plugin не потребує rebuild, якщо всі його required interfaces і
triggers уже входять до application inventory. Новий host interface або trigger
type потребує rebuild product application, але не зміни plugin platform.

### 3.3. Product-local triggers

Platform core визначає generic `ApplicationTriggerSet`. Product build генерує
typed Rust enum та inventory для власних triggers. Plugin manifest посилається
на versioned WIT identity trigger-а, а не на довільний рядок із JSON payload.

### 3.4. Repository boundary

Plugin system розробляється в окремому coordinated platform monorepo
`nitra/n-plugin`. Він містить Rust runtime crates, Tauri host
integration, Vue packages, WIT contracts, schemas, `n-plugin-cli`, demos,
fixtures, conformance tests і документацію. Усі artifacts випускаються одним
coordinated platform version та утворюють один tested compatibility set.

`nitra/tauri-components` не містить plugin-specific runtime, UI, bindings, demo
або compatibility facade. Plugin platform може залежати від його public generic
Vue/Tauri primitives і design tokens, але зворотна залежність заборонена. Product
application залежить від обох repositories напряму. M0 spike одразу реалізується
в новому repository, без проміжної реалізації в `tauri-components`.

### 3.5. SDK boundaries і naming

Усі plugin-specific packages послідовно використовують prefix `n-plugin`.
Coordinated release містить:

```text
n-plugin-interfaces  canonical ABI types без Tauri/runtime dependencies
n-plugin-guest       generated bindings і macros для Preview 2 guest plugins
n-plugin-host        generated native host bindings
n-plugin-runtime     Component linker, registry, graphs і Tauri integration
n-plugin-cli         authoring/build/publish CLI; executable n-plugin
```

Canonical WIT зберігається тільки в `wit/`. CI забороняє копіювати shared WIT у
crates; host і guest bindings генеруються з одного source tree. Усі crates, Vue
packages, WIT profile і CLI випускаються однією platform version.

## 4. Runtime і ABI

### 4.1. Runtime baseline

- Production baseline — latest stable Bytecode Alliance runtime 48 LTS після
  офіційного release.
- До release дозволений лише development spike на prerelease build.
- Увесь M0 pin-ить точні runtime, `wit-bindgen`, `cargo-component`, `wkg` та
  adapter toolchain versions.
- Toolchain/ABI snapshot не змінюється протягом runtime 48 lifecycle без нового
  platform compatibility profile.

### 4.2. Component Model only

Runtime використовує:

- component `Component` API;
- component `Linker`;
- generated WIT bindings;
- Canonical ABI records, variants, resources, `future` і `stream`;
- cached compiled artifacts та pre-instantiation data.

Core-Wasm `Module`, guest-memory pointer operations, JSON buffers та integer ABI
error conventions не входять до runtime path.

### 4.3. Experimental async profile

Погоджено `C1 — unrestricted experimental async`:

- plugins можуть використовувати native `future`, `stream` і concurrency;
- M0 не вводить restricted async subset;
- весь graph компонується під одним pinned async ABI snapshot;
- incompatible snapshot є compatibility failure, а не runtime fallback.

### 4.4. Instance lifecycle

- Instance/Store створюється lazy для activation generation.
- Один root graph має long-lived instance/Store протягом generation.
- Окремого lifecycle WIT (`init`, `shutdown`, `migrate`) немає.
- Application shutdown скасовує Store tasks і закриває generation.

Trap не повторює поточний call автоматично. Runtime створює нову instance із
exponential backoff, jitter, token bucket і automatic half-open canary на одному
реальному наступному виклику. Кількість lifetime restarts не обмежується, але
backoff state зберігається в SQLite.

Цей operational retry control не є execution resource limit. M0 свідомо не має
захисту від нескінченного або надмірного guest execution.

## 5. Authoring toolchain і package format

### 5.1. `n-plugin-cli`

Authoring toolchain має назву `n-plugin-cli`, executable — `n-plugin`. Він є
тонким orchestration layer навколо embedded Rust libraries
`wasm-pkg-core`/`wasm-pkg-client`, а не окремим package resolver. Зовнішній
executable `wkg` не є runtime dependency.

M0 підтримує Rust authoring, але canonical component і manifest formats не
містять Rust-specific assumptions. Основні flows:

```text
n-plugin new
n-plugin build
n-plugin lock
n-plugin test
n-plugin inspect
n-plugin publish
n-plugin fetch
```

`n-plugin test` запускає фактичний Wasm Component у local Component test host,
підключає generated WIT mocks, компонує dependencies, перевіряє async streams,
traps і Vue A2UI snapshots.

Distribution commands зберігають upstream semantics: `n-plugin publish`,
`n-plugin fetch` і `n-plugin lock` викликають відповідні embedded
`wasm-pkg-tools` APIs для publish/get/resolution. CLI не запускає зовнішній
`wkg` subprocess. Власний registry protocol, SemVer selector або portable
dependency lock format не створюється.

### 5.2. Authoring files

- `wkg.toml` — стандартна конфігурація `wkg`;
- `wkg.lock` — стандартний portable exact WIT/package resolution без
  plugin-specific extension fields;
- `.n-plugin.toml` — plugin-specific authoring metadata;
- compiled `.n-plugin` — raw valid WebAssembly Component.

`.n-plugin` не є ZIP/TAR container і не містить довільних assets. Versioned
runtime manifest вбудовується у custom section Component-а. Plugin entrypoints
у `.n-plugin.toml` явно зіставляються з typed WIT export references; naming
conventions, export-all і JSON entrypoints заборонені.

### 5.3. Identity

`publisher_id` є стандартним `wkg` namespace. Canonical package identity
серіалізується як:

```text
namespace:package
```

Canonical release identity:

```text
namespace:package@version + digest
```

Наприклад, `publisher_id = nitra` і `package = booking-finder` утворюють
`nitra:booking-finder`. Registry URL, OCI reference або local path є лише
locator. Вони не входять до identity або trust decision. Publisher transfer не
підтримується. Для нового namespace створюється інша package identity.

## 6. Resolution, installation і updates

### 6.1. Sources

Ordered resolver перевіряє:

1. local content-addressed store;
2. explicit local file/import;
3. configured `wkg` sources.

Namespace mapping використовує стандартні `wkg` configuration fields
`namespace_registries` і `package_registry_overrides`. Registry metadata
отримується через `/.well-known/wasm-pkg/registry.json` і може спрямувати
namespace до GHCR або іншого OCI storage. Product policy може дозволяти чи
забороняти mappings, але не змінює package identity.

Dependency іншого publisher може завантажуватися окремо від root package.
Registry/OCI coordinates не фіксуються як identity. Стандартний `wkg.lock`
зберігає package, requirement, exact version, content digest і configured
registry name; SQLite може додатково зберігати OCI reference та manifest digest
як provenance evidence для diagnostics.

Основний install input — `namespace:package@version-or-range`. Explicit import
може прийняти `oci://` або local file locator. Installer завантажує artifact,
читає embedded manifest, перевіряє `namespace:package@version`, резолвить mutable
OCI tag у concrete artifact, обчислює/verifies content digest і лише потім
створює exact package lock. Artifact без embedded identity або з identity, що не
відповідає request, відхиляється. Dependencies із невідомими namespace mappings
блокують installation до налаштування mapping.

Canonical release digest дорівнює `wkg` content digest: SHA-256 raw WebAssembly
Component bytes, які в стандартному OCI artifact є digest першого Component
layer. OCI manifest digest і OCI reference зберігаються лише як locator та
provenance evidence і не входять до release identity.

### 6.2. Version resolution

- Author manifest використовує WIT/package SemVer ranges.
- Embedded `wasm-pkg-core` виконує SemVer selection, fetch, cache lookup і
  content-digest verification за upstream `wkg` semantics.
- M0 activation завжди використовує exact locked versions і digests.
- Side-by-side versions дозволені в різних activation graphs.
- M0 не реалізує власний global SAT/backtracking solver: exact locks не
  потребують його, а майбутня range resolution слідує upstream `wkg` semantics.
- Dependency collector та activation compiler відхиляють plugin cycle, missing
  required node й incompatible WIT types; `wkg` окремо перевіряє WIT dependency
  resolution.
- Optional dependencies не підтримуються: кожна declared dependency required.

### 6.3. `wkg`-owned package resolution

Package resolution і runtime activation є послідовними шарами з одним version
resolver, а не двома конкурентними package managers:

```text
embedded .n-plugin manifests
  -> n-plugin Dependency Collector
  -> wasm-pkg-core DependencyResolver
  -> exact Resolved Package Graph
  -> n-plugin Activation Compiler
  -> SQLite activation generation
```

`Dependency Collector` читає immutable embedded manifests root package і
required dependencies, зберігає caller-to-callee constraints та передає package
requirements до `wasm-pkg-core`. Collector може ітеративно відкривати manifests
щойно завантажених dependencies, але не вибирає version, не порівнює SemVer і не
підміняє upstream resolver policy.

`wasm-pkg-core` є authoritative для namespace routing, requirement-to-version
selection, fetch/cache, offline lock lookup, yanked-release handling і content
digest verification. Resolver invocation scoped до candidate root activation
graph; інший root graph може отримати іншу exact version того самого package.

`Activation Compiler` відображає exact resolutions назад на declared
caller-to-callee edges, перевіряє Component imports/exports, WIT resource
identity, host inventory, edge grants і application compatibility. Він не може
змінити version або digest, повернуті resolver-ом.

Стандартний `wkg.lock` зберігає portable tuples:

```text
package + registry evidence + requirement + version + content digest
```

Lock не розширюється полями edges, grants, consent, enabled state або activation
generations. Runtime матеріалізує ті самі exact resolutions разом із
plugin-specific edges у SQLite; це runtime projection, а не альтернативний
dependency lock format.

### 6.4. Atomic installation

Downloads є resumable і спочатку потрапляють у staging/CAS. Resolver будує
повний graph, type-check-ить його та лише потім створює нову registry generation.
Partial download не є installed plugin. Failure не змінює active generation.

Compatible plugin/dependency updates можуть завантажуватися автоматично. Вони
активуються лише під час наступного application restart. Нові capabilities або
зміна grants завжди потребують consent. Composition failure залишає попередню
generation активною.

## 7. Plugin dependencies і calls

### 7.1. Guarded immutable composition

Plugin-to-plugin calls не проходять через dynamic JSON broker. Installer створює
immutable composed activation graph:

```text
root plugin
  │ typed import
  ▼
edge guard component
  │ caller/callee identity + edge grants
  ▼
dependency component
```

Guard generated для конкретного dependency edge та generation. Він перевіряє
edge identity, enabled state і edge-scoped grants. Root і required dependencies
працюють як один composed graph/Store. Registry mutations створюють нову
copy-on-write generation; стара generation draining до завершення активних calls.

### 7.2. Graph ownership

Dependency є видимим graph-owned node, а не автоматично top-level plugin. Plugin
Manager показує direct `Depends on` і `Used by`. Dependency можна окремо
promote-нути до root installation, після чого вона отримує власний root graph.

Будь-яка невдала required dependency робить root graph неактивованим. Partial
або degraded activation не існує, бо optional dependencies відсутні.

### 7.3. Grants

Grants є explicit і edge-scoped. Caller не успадковує grants dependency, а
dependency не отримує grants caller-а. Dependency без host capability requests
не створює додатковий consent. Dependency з host capabilities отримує власні
edge grants у тому самому graph consent flow.

Layered deny authorities застосовуються незалежно від allow grant:

```text
platform hard deny
→ product policy deny
→ local administrator deny
→ user deny
→ edge-scoped allow grant
```

Deny має пріоритет над allow. У M0 ці policies не доводять authenticity package.

## 8. State, registry, CAS та audit

### 8.1. Storage

- SQLite зберігає package metadata, activation graphs, exact resolutions,
  caller-to-callee edges, grants, desired/effective state, restart state, audit
  indexes і registry generations. Exact `package + version + digest` походять
  лише з `wasm-pkg-core`; SQLite не виконує package resolution.
- Filesystem CAS зберігає Components, downloaded packages і compiled cache.
- SQLite transaction публікує generation лише після готовності всіх CAS objects.

### 8.2. Plugin state

M0 підтримує лише graph-scoped state. Provider-shared state можна додати пізніше
як окремий explicit scope. State schema change автоматично очищає state без
migration, consent або rollback snapshot. Authors повинні вважати state
disposable; wipe створює audit event.

### 8.3. Garbage collection

CAS використовує mark-and-sweep із quarantine/grace period. Roots: installed
graphs, active/draining generations, exact locks і незавершені staging
transactions. LRU застосовується лише до відтворюваного compiled cache.

### 8.4. Audit

Audit є structured SQLite log із correlation chain для nested calls і
hash-chain checkpoints. Він зберігає identities, operation, capability, scope,
status, timing і generation, але не payload bodies, OAuth tokens, Gmail query,
message content або secrets. Hash chain дає tamper evidence локального журналу,
але без зовнішнього підпису не є незалежним proof authenticity.

## 9. Vue UI та neutral demo

Усі підтримувані Tauri products використовують Vue. Plugin platform постачає:

- стандартний Vue Plugin Manager;
- A2UI renderer;
- host-governed dependency slots;
- consent і dependency graph views;
- neutral demo components та integration guide для LLM/розробника.

Plugin UI не розміщує dependency UI довільно. Host відображає лише direct
dependencies у визначених slots. Transitive graph доступний у Plugin Manager.

Neutral `Platform Info` demo не залежить від product domains і повертає лише:

- application ID, name і version;
- OS, architecture і locale;
- plugin runtime та WebAssembly engine profile;
- registry generation;
- public host interfaces і triggers.

Demo не повертає accounts, install paths, tokens, filesystem content, internal
services або sensitive product data.

## 10. Gmail search product contract

Gmail integration належить `mlmail`, а не `tauri-components`. Product реєструє
typed WIT interface `nitra:gmail/search` і capability `mail:search`.

### 10.1. Permission

Grant є account-scoped. Gmail query не входить до scope і не має allowlist чи
domain restriction. Плагін може використовувати повний query syntax Gmail.

`mlmail` виконує Gmail API через один app-owned OAuth token. Guest ніколи не
бачить token. OAuth token повинен мати scope, який дозволяє `messages.list`,
наприклад `gmail.readonly`, `gmail.modify` або повний Gmail scope; metadata-only
OAuth scope не дозволяє Gmail search query.

### 10.2. Result

WIT response семантично повторює `users.messages.list`, а не створює власну
mail abstraction:

```text
search-page {
  messages: list<message-ref>,
  next-page-token: option<string>,
  result-size-estimate: u64,
}

message-ref {
  id: string,
  thread-id: string,
}
```

Операція повертає native async `stream<search-page>`. Pages мають ті самі
значення, що й Gmail response: `messages`, `nextPageToken` і
`resultSizeEstimate`. Body, headers, attachments і raw OAuth/HTTP data не
додаються. Query unrestricted; M0 не вводить result або stream quotas.

## 11. Application update compatibility

### 11.1. Release metadata

Кожен application release публікує в updater metadata authoritative
`plugin-environment/v1`:

```json
{
  "schema": "nitra.plugin-environment/v1",
  "application": { "id": "com.example.app", "version": "2.0.0" },
  "runtime": {
    "runtimeLts": 48,
    "componentModelProfile": "nitra-component-v1",
    "asyncAbiSnapshot": "exact-toolchain-snapshot"
  },
  "pluginManifestVersions": [1],
  "hostInterfaces": [],
  "triggers": [],
  "vueA2uiSchema": 1,
  "requiredFeatures": [],
  "fingerprint": "sha256:..."
}
```

Host interface і trigger entries містять WIT package/interface identity,
version та canonical type hash. Metadata генерується з compiled product
inventory у CI; ручне редагування заборонене.

Embedded authoritative manifest і historical projections не підтримуються.
`plugin-environment/v1` є permanent additive schema:

- наявні поля та їхня семантика не змінюються;
- нові поля можуть бути лише optional;
- старі applications ігнорують невідомі optional fields;
- новий major schema не створюється, доки потрібен direct update зі старих
  application versions.

Після першого запуску нова application version порівнює фактичний generated
registry fingerprint із release metadata. Drift створює audit event і не
відкочує application; affected plugin graphs не активуються.

### 11.2. Preflight

Update check працює в такому порядку:

1. Отримати latest available application metadata.
2. Вибрати plugins із `desired_state: enabled`. Manually disabled plugins
   повністю ігноруються.
3. Через `wkg` виконати staged compatibility repair для target environment.
4. Перевірити exact WIT identities/type hashes.
5. Виконати dry composition і Component compilation check без запуску plugin logic.
6. Побудувати compatibility impact і consent preview.

Preflight класифікує graphs як compatible unchanged, compatible after plugin
update, needs new grant або incompatible/will be disabled. Якщо resolver source
недоступний чи repair неповний, application update блокується до завершення
перевірки. Partial/unknown result не дозволяє override.

Repeated repair є event-driven. Він запускається лише коли змінився application
manifest fingerprint, configured `wkg` source index/digest, active desired graph
або користувач натиснув `Check again`. Однаковий input не запускає повторну
composition.

### 11.3. User choice і consent

Основний dialog пропонує:

```text
Update application
  compatible plugins remain enabled
  compatible plugin releases will update
  known incompatible plugins will be disabled

[Update and restart] [Stay on current version]
```

Якщо repair знайшов сумісний plugin release з новими capabilities, dialog додає
per-plugin choice: `Allow and keep enabled` або `Do not allow — disable plugin`.
New grants staged і фіксуються атомарно з target graph. Відмова не блокує
application update, а вимикає лише відповідний root graph.

Application installer завантажується й перевіряється лише після `Update and
restart`. При `Stay` installer не завантажується, staging roots одразу
звільняються, а unreachable CAS objects прибирає звичайний quarantine/grace GC.

### 11.4. Deferral і reminders

Зберігаються:

```text
target_application_version
compatibility_impact_hash
decision_timestamp
notification_policy
```

Для тієї самої version та impact повторного modal немає. Settings показує
passive update status. Одне material-change reminder дозволене, коли:

- з'явився новіший application release;
- зменшилася кількість incompatible graphs;
- усі desired graphs стали compatible;
- користувач сам попросив нагадати пізніше.

При поверненні до update завжди вибирається latest available version і
виконується новий preflight. Application updates ніколи не примусові: немає
critical override, minimum supported version або expiration старої версії.

### 11.5. Activation після update

Application update не відкочується через plugin activation failure. Успішні
graphs активуються, а невдалі отримують system disable reason:

```text
desired_state: enabled | disabled
effective_state: active | system_disabled | activation_failed
disabled_by: user | application_incompatibility | activation_failure | missing_grant | policy
```

Якщо runtime вимкнув plugin автоматично, він автоматично підготує та активує
сумісний graph під час наступного application restart, коли причина усунена і
нові grants не потрібні. Manually disabled plugin ніколи не вмикається
автоматично.

## 12. Failure semantics

Stable public error categories:

- `package-missing`;
- `dependency-missing`;
- `dependency-cycle`;
- `wit-incompatible`;
- `runtime-profile-incompatible`;
- `grant-denied`;
- `policy-denied`;
- `plugin-disabled`;
- `activation-failed`;
- `source-unavailable`;
- `internal`.

Guest і UI не отримують linker internals, filesystem paths, OAuth details або
secrets. No automatic call replay applies to traps, application restart або
generation switch.

## 13. Міграція

1. Зафіксувати runtime 48 LTS toolchain snapshot після stable release.
2. Створити coordinated monorepo `nitra/n-plugin` з єдиним release
   profile для Rust, Vue, WIT і CLI artifacts.
3. Побудувати в ньому M0 neutral typed Component з async stream і generated host
   binding.
4. Додати generic `PluginHostInterfaceRegistry` та `ApplicationTriggerSet` у
   plugin platform.
5. Реалізувати `n-plugin-cli`, embedded `WkgResolutionBackend`, standard
   `wkg.lock`, raw Component manifest section і local Component test host.
6. Реалізувати SQLite registry, filesystem CAS, immutable activation generations
   та guarded dependency composition.
7. Додати grants, layered deny, audit і Vue UI.
8. Додати neutral `Platform Info` demo та LLM integration guide.
9. Винести Gmail WIT/handlers у `mlmail`; реалізувати `nitra:gmail/search`.
10. Перепакувати Draft Helper як Component і додати Booking Finder demo.
11. Додати application compatibility metadata/preflight flow.
12. Видалити legacy core-Wasm loader, JSON/string ABI і mail-specific platform API
    з `tauri-components` та product repositories.
13. Після завершення реалізації створити в `nitra/n-plugin/docs/`
    детальну документацію роботи plugin system: architecture й runtime lifecycle,
    host application integration, plugin authoring через `n-plugin-cli`, WIT
    contracts, dependencies, grants і consent, installation та updates,
    state/CAS/audit, testing, diagnostics, troubleshooting і наскрізні приклади.

Migration branch не постачає legacy fallback у production build.

## 14. M0 acceptance criteria

- Новий Vue/Tauri application реєструє product WIT без зміни plugin platform або
  `tauri-components`.
- `tauri-components` не містить plugin-specific code і не залежить від plugin
  platform; dependency graph перевіряється CI.
- Installing plugin із відомими interfaces/triggers не rebuild-ить application.
- Runtime відхиляє core-Wasm module і приймає valid WebAssembly Component.
- Typed host call і native async stream працюють через generated bindings.
- Required dependency іншого publisher завантажується окремо, exact lock-иться
  та викликається через generated edge guard.
- `wasm-pkg-core` є єдиним version selector; `n-plugin` не містить власного
  SemVer selection або SAT/backtracking solver.
- `wkg.lock` лишається upstream-compatible без plugin-specific fields, а SQLite
  activation generation відтворює ті самі exact package versions і digests.
- Optional dependency відхиляється manifest validation.
- Caller і dependency не успадковують grants одне одного.
- New grant показується per-plugin; dependency без host capability не створює
  consent.
- Copy-on-write generation activation є atomic; failure зберігає стару generation.
- Trap не replay-ить call і запускає self-healing restart policy.
- State schema change очищає graph state та створює audit event.
- `Platform Info` повертає лише public plugin environment.
- Gmail search приймає unrestricted query і повертає native stream сторінок із
  семантикою `users.messages.list`.
- Application preflight знаходить compatible plugin releases, показує impact і
  не встановлює application update без completed repair та user consent.
- `Stay` не повторює modal без material change; manual update завжди доступний.
- Automatically disabled plugin автоматично повертається після появи compatible
  graph; manually disabled plugin лишається disabled.
- Production M0 явно позначений `experimental/trusted-only` і не заявляє захист
  від malicious plugins.
- Реалізація не вважається завершеною без актуальної детальної документації
  plugin system у `docs/`, перевіреної на відповідність фактичному runtime та CLI.

## 15. Допоміжні implementation spikes

Архітектурних open decisions для M0 не залишилося. Перед coding потрібно
підтвердити експериментами:

1. Exact generated Rust shape для heterogeneous typed host registrations.
2. Runtime 48 API та pinned unrestricted async snapshot після stable release.
3. Embedded `wasm-pkg-core` integration: exact/multi-version locks, offline
   cache, yanked releases, raw Component content digest і custom-section
   encoding.
4. Guard component generation та composition для multi-publisher graph.
5. Tauri updater custom metadata plumbing для `plugin-environment/v1`.
6. Cold/warm compile й application preflight timings як diagnostics, без
   enforcement budgets.

Ці spikes можуть змінити internal API, але не ухвалені product/runtime semantics.
