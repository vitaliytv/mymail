# Tauri + Vue 3

додаток у нас наразі працює в двох режимах: Android та для Mac OS.

## Локальний DMG-реліз

Android APK і підписаний macOS ARM64 DMG збираються на виділеному Forgejo runner на Mac mini (`release-macos`). Звичайний CI і створення Forgejo release залишаються на ARM64 runner; GitHub Actions для релізу не використовуються. Mac mini отримує secrets лише через Infisical, без збереження signing secrets у репозиторії. Intel Mac не підтримуються у нових desktop releases.

Спочатку створіть локальний Infisical project config для vitaliytv-kfse (це одноразова локальна дія, конфіг не комітиться):

```sh
infisical --domain https://secret.7n.ai init
```

Після того як тег vX.Y.Z вказує на поточний чистий HEAD, зберіть assets:

```sh
infisical --domain https://secret.7n.ai run --env=main --path=/apple --path=/updater -- \
  cargo xtask release-assets X.Y.Z
```

Команду запускають на Mac mini runner. Вона оновлює `app/src-tauri/tauri.conf.json` до `X.Y.Z`, створює локальний version commit і annotated tag `vX.Y.Z`, перевіряє Developer ID signature усередині DMG і створює `dist/MyMail-vX.Y.Z/`: DMG, updater archive, signature, SHA256SUMS і latest.json. Вона не пушить, не створює release і не завантажує файли: після успішної перевірки надруковані команди спершу публікують commit/tag у Forgejo, а потім завантажують assets до release.

Далі, ми додаємо авторизацію на Google і там, і там.

Далі, ми додаємо інтеграцію з Gmail для того, щоб зчитувати листи.

Далі, ми кажемо, що коли лист зчитано, нам потрібно зробити перший крок - це отримати якесь саммері по цьому листу. І другий крок - це озвучити це саммері.

Tak. Aše potem dališ rezultat. A što s tem listom robiti? I v nas budut otveti. Pervij rezultat otveti - vydaliti. Drugij - vydaliti i nastroiti filtr, štob takije listy bolše ne zjavlalis v počte. Tretij variant - eto zafiksirati jego v pamjati. Jak osobiste. A četvjortij - zafiksirati jego v pamjati jak roboče. Majetsja na uvazi, budut dve papočki: work i home. I tam budut md-fajliki s kožnym listom. Ot. I ostanovka - eto podgotoviti otvet na list vdolazu s propozicijeju, što ja proponuju otvetiti tak-to.
