---
type: Vue Component
title: LlmSettingsDialog.vue
resource: app/src/components/LlmSettingsDialog.vue
docgen:
  crc: 1a45fb20
  model: manual
---

## Огляд

Діалог налаштовує OpenAI-compatible локальний LLM endpoint для всіх функцій mlmail.

## Інтерфейс компонента

- Приймає `modelValue` та емітить `update:modelValue` для керування відкриттям.
- Показує URL сервера, тимчасовий API key і вибір моделі.

## Поведінка

- Під час відкриття підтягує стартовий URL і намагається завантажити моделі.
- «Перевірити» робить запит списку моделей і показує результат або помилку підключення.
- «Зберегти» записує тільки валідний URL і модель; API key лишається в пам’яті до закриття застосунку.
