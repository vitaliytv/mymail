# Changelog

Усі помітні зміни цього пакета документуються тут.

Формат — [Keep a Changelog](https://keepachangelog.com/uk/1.1.0/), нумерація — [SemVer](https://semver.org/lang/uk/).

## Unreleased

### Changed

- Android release APK тепер виконується на виділеному Forgejo Mac mini runner (`release-macos`); підписаний macOS DMG збирається там само.

## [0.1.1] - 2026-05-21

### Added

- `.github/workflows/lint-security.yml` — окремий CI-скан секретів через TruffleHog.
- `.trufflehog-exclude` — regex-винятки для скану секретів.
- `.vscode/settings.json` — формат-налаштування редактора для основних мов і `github-actions-workflow`.
- Workspace-пакет `scripts/` із власними залежностями.

### Changed

- `lint-security` переведено з `gitleaks` на `trufflehog filesystem`.
- Залежності скриптів (`gray-matter`, `tinyglobby`) перенесено з кореневих devDependencies у workspace `scripts/`.
- Оновлено правила, скіли та hook-и ADR.

### Fixed

- `lint-ga.yml` — додано крок Install conftest.
- `lint-text.yml` — додано кроки Install shellcheck і Install dotenv-linter.
