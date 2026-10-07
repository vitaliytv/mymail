<template>
  <q-dialog @update:model-value="onToggle" :model-value="modelValue">
    <q-card style="min-width: 460px; max-width: 92vw">
      <q-card-section class="row items-center">
        <div class="text-h6">Налаштування LLM</div>
        <q-space />
        <q-btn v-close-popup flat round dense icon="sym_o_close" aria-label="Закрити" />
      </q-card-section>
      <q-card-section class="column q-gutter-md">
        <q-input v-model="baseUrl" label="OpenAI-compatible URL" hint="Напр.: http://127.0.0.1:8080/v1/" outlined />
        <q-input
          v-model="apiKey"
          label="API key (лише до закриття застосунку)"
          type="password"
          autocomplete="off"
          outlined />
        <q-select v-model="model" :options="models" label="Модель" use-input new-value-mode="add-unique" outlined />
        <div class="text-caption text-grey-7">
          Ключ не записується в localStorage. Порожній ключ використовує N_LOCAL_OPENAI_API_KEY, якщо його передано під
          час запуску.
        </div>
        <div v-if="error" class="text-negative text-caption">{{ error }}</div>
        <div v-else-if="status" class="text-positive text-caption">{{ status }}</div>
      </q-card-section>
      <q-card-actions align="right">
        <q-btn v-close-popup flat no-caps label="Скасувати" />
        <q-btn @click="testConnection" flat no-caps label="Перевірити" :loading="busy" />
        <q-btn @click="saveConfig" color="primary" unelevated no-caps label="Зберегти" :disable="busy" />
      </q-card-actions>
    </q-card>
  </q-dialog>
</template>

<script setup>
/** Configure and verify the app-wide local OpenAI-compatible LLM endpoint. */
import { useLlm } from '../llm.js'

defineProps({ modelValue: { type: Boolean, default: false } })
const emit = defineEmits(['update:modelValue'])
const { baseUrl, apiKey, model, loadEnv, refreshModels, save } = useLlm({ storagePrefix: 'mlmail' })
const models = ref([])
const busy = ref(false)
const error = ref('')
const status = ref('')

/**
 * Opens the dialog state and refreshes the endpoint's available models.
 * @param {boolean} value whether the dialog is opening
 * @returns {Promise<void>} resolves after the initial model refresh
 */
async function onToggle(value) {
  emit('update:modelValue', value)
  if (!value) return
  error.value = ''
  status.value = ''
  try {
    await loadEnv()
    models.value = await refreshModels()
  } catch (error) {
    error.value = String(error?.message ?? error)
  }
}

/** Checks the entered endpoint without persisting the form values. */
async function testConnection() {
  busy.value = true
  error.value = ''
  status.value = ''
  try {
    models.value = await refreshModels()
    status.value = models.value.length
      ? `З’єднання працює. Моделей: ${models.value.length}.`
      : 'З’єднання працює, але сервер не повернув моделей.'
  } catch (error) {
    error.value = String(error?.message ?? error)
  } finally {
    busy.value = false
  }
}

/** Verifies and persists the endpoint/model while leaving the API key volatile. */
async function saveConfig() {
  busy.value = true
  error.value = ''
  try {
    models.value = await refreshModels()
    save()
    status.value = 'Налаштування збережено.'
  } catch (error) {
    error.value = String(error?.message ?? error)
  } finally {
    busy.value = false
  }
}
</script>
