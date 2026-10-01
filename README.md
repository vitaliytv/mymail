# Tauri + Vue 3

додаток у нас наразі працює в двох режимах: Android та для Mac OS.

## Реліз

Реліз працює за моделлю `foc`; workflow — `.forgejo/workflows/release.yml`. Єдине джерело версії — `app/package.json`: `tauri.conf.json` читає її через `"version": "../package.json"`.

1. Кожна зміна `app/` супроводжується change-файлом у `app/.changes/` (`npx @7n/n ch`).
2. Після merge у `main` job `prepare-release-pr` запускає `n-rules release`, який бампає `app/package.json` і дописує `app/CHANGELOG.md`, і відкриває PR `release/vX.Y.Z` з комітом `chore(release): vX.Y.Z`. Автоматизація ніколи не пушить у `main` напряму.
3. Merge release PR → `tag-release` ставить annotated tag `vX.Y.Z` і запускає `release.yml` на тезі.
4. На тезі: `validate` → draft release → паралельні збірки на ранерах, спільних з `foc`: `macos-arm64` — підписаний і нотаризований ARM64 DMG, updater archive, `latest.json`; `macos-x86_64` (`air11`, macOS 11) — Android APK (`arm64`; JDK, Android SDK, NDK r27, Rust і Big Sur-збірки Node та Bun встановлені на ранері, `bun install` виконується прямо там) → collector (`SHA256SUMS`, `MANIFEST.txt`, upload) → публікація релізу. Updater бачить нову версію лише після публікації. Rust збирається в постійному каталозі на ранерах (`~/Library/Caches/mymail/cargo-target`, як у `foc`), тож наступні релізи перекомпільовують лише змінене.

Forgejo-токени видаються через одну OIDC Authorized Integration репозиторію, обмежену `release.yml` (`RELEASE_AUDIENCE`). Як у `foc`, DMG підписується Developer ID-сертифікатом, установленим у keychain Mac mini (`APPLE_SIGNING_IDENTITY` — його назва, задана у workflow). DMG нотаризується `xcrun notarytool` через keychain-профіль `nitra-notary` (App Store Connect API key, `NOTARY_KEYCHAIN_PROFILE` у workflow), ticket пришивається до DMG (`stapler`). Ключ updater (`TAURI_SIGNING_PRIVATE_KEY`, за потреби `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) — секрет Forgejo Actions репозиторію, як у `maya/release`; Infisical постачає лише Android keystore. GitHub Actions для релізу не використовуються. Як і `foc`, збірка не клонує `internal`-репозиторії: crate-и `n-plugin-*` беруться з реєстру `crates-7n` (`.cargo/config.toml`, читання без токена), тому release job-ам не потрібна git-авторизація. Intel Mac не підтримуються у нових desktop releases.

### Локальна перевірка assets

Для діагностики ті самі assets можна зібрати локально з checkout злитого release-тегу. Спочатку створіть локальний Infisical project config (одноразово, не комітиться):

```sh
infisical --domain https://secret.7n.ai init
```

На Mac потрібні Developer ID-сертифікат у keychain і notarytool-профіль (одноразово: `xcrun notarytool store-credentials nitra-notary --key AuthKey_….p8 --key-id … --issuer …`). Потім на чистому checkout тегу `vX.Y.Z`:

```sh
APPLE_SIGNING_IDENTITY="Developer ID Application: N.itra ou (UHG6A28MTA)" NOTARY_KEYCHAIN_PROFILE=nitra-notary \
  infisical --domain https://secret.7n.ai run --env=main --path=/updater -- \
  cargo xtask release-assets X.Y.Z
```

Команда перевіряє, що `app/package.json` має версію `X.Y.Z` і тег вказує на HEAD, і створює `dist/MyMail-vX.Y.Z/`: нотаризований DMG, updater archive, signature, SHA256SUMS і latest.json. Вона нічого не комітить, не тегує і не змінює стан на сервері.

Далі, ми додаємо авторизацію на Google і там, і там.

Далі, ми додаємо інтеграцію з Gmail для того, щоб зчитувати листи.

Далі, ми кажемо, що коли лист зчитано, нам потрібно зробити перший крок - це отримати якесь саммері по цьому листу. І другий крок - це озвучити це саммері.

Tak. Aše potem dališ rezultat. A što s tem listom robiti? I v nas budut otveti. Pervij rezultat otveti - vydaliti. Drugij - vydaliti i nastroiti filtr, štob takije listy bolše ne zjavlalis v počte. Tretij variant - eto zafiksirati jego v pamjati. Jak osobiste. A četvjortij - zafiksirati jego v pamjati jak roboče. Majetsja na uvazi, budut dve papočki: work i home. I tam budut md-fajliki s kožnym listom. Ot. I ostanovka - eto podgotoviti otvet na list vdolazu s propozicijeju, što ja proponuju otvetiti tak-to.
