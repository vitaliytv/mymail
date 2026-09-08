<template>
  <q-page class="column items-center q-pa-md" :class="{ 'has-bar': auth.isAuthenticated.value }">
    <template v-if="auth.isAuthenticated.value">
      <q-banner v-if="auth.inboxErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
        {{ errorMessage(auth.inboxErrorKind.value) }}
      </q-banner>

      <template v-if="auth.currentMessage.value">
        <div class="row no-wrap q-col-gutter-md reader">
          <div class="col-12 col-md-6 column">
            <q-card flat bordered class="fit column">
              <q-card-section>
                <div class="text-overline text-grey-7">Оригінал</div>
                <div><strong>Від:</strong> {{ auth.currentMessage.value.from }}</div>
                <div><strong>Тема:</strong> {{ auth.currentMessage.value.subject }}</div>
                <div><strong>Дата:</strong> {{ auth.currentMessage.value.date }}</div>
                <div v-if="auth.currentMessage.value.attachments?.length" class="q-mt-sm attachments-row">
                  <q-chip
                    v-for="att in auth.currentMessage.value.attachments"
                    :key="att.attachment_id"
                    @click="auth.openAttachment(att)"
                    icon="sym_o_attach_file"
                    dense
                    outline
                    square
                    clickable
                    :disable="auth.openingAttachmentId.value === att.attachment_id">
                    <q-spinner
                      v-if="auth.openingAttachmentId.value === att.attachment_id"
                      size="16px"
                      class="q-mr-xs" />
                    {{ att.filename }}
                    <span class="text-grey-6 q-ml-xs">({{ formatFileSize(att.size) }})</span>
                  </q-chip>
                </div>
              </q-card-section>
              <q-separator inset />
              <q-card-section v-if="auth.currentMessage.value.html_body" class="col message-html-section">
                <iframe
                  :srcdoc="htmlBodyWithInterceptor"
                  sandbox="allow-scripts"
                  class="message-iframe"
                  referrerpolicy="no-referrer" />
              </q-card-section>
              <q-card-section v-else class="message-body col">
                {{ auth.currentMessage.value.body }}
              </q-card-section>
            </q-card>
          </div>
          <div class="col-12 col-md-6 column">
            <q-card flat bordered class="fit column">
              <NewsletterView ref="newsletterViewRef" :message="auth.currentMessage.value" />
            </q-card>
          </div>
        </div>
      </template>
      <template v-else-if="auth.isMessageLoading.value">
        <q-card flat bordered style="max-width: 60ch; width: 100%">
          <q-card-section>
            <q-skeleton type="text" width="70%" />
            <q-skeleton type="text" width="60%" />
            <q-skeleton type="text" width="50%" />
          </q-card-section>
          <q-separator inset />
          <q-card-section>
            <q-skeleton type="text" />
            <q-skeleton type="text" />
            <q-skeleton type="text" width="80%" />
          </q-card-section>
        </q-card>
      </template>
      <q-banner v-else-if="auth.messageErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
        {{ errorMessage(auth.messageErrorKind.value) }}
      </q-banner>
      <q-banner v-else class="bg-grey-2" rounded dense> Скринька порожня. </q-banner>

      <q-banner v-if="auth.saveErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
        {{ errorMessage(auth.saveErrorKind.value) }}
      </q-banner>
      <q-banner v-if="auth.unsubscribeErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
        {{ errorMessage(auth.unsubscribeErrorKind.value) }}
      </q-banner>
      <q-banner v-if="auth.trashErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
        {{ errorMessage(auth.trashErrorKind.value) }}
      </q-banner>
    </template>

    <q-btn
      v-else
      @click="auth.login()"
      color="primary"
      icon-right="sym_o_login"
      size="md"
      :loading="auth.isLoading.value">
      <template v-if="!auth.isLoading.value">Увійти через Google</template>
      <template v-else>Зачекайте…</template>
    </q-btn>

    <q-banner v-if="auth.errorKind.value" class="bg-red-1 text-red-9" rounded dense>
      {{ errorMessage(auth.errorKind.value) }}
    </q-banner>

    <q-page-sticky v-if="auth.isAuthenticated.value" position="bottom" :offset="[0, 0]" expand>
      <q-toolbar class="bg-grey-2 text-primary action-bar q-px-md">
        <q-btn
          @click="auth.unsubscribeFromCurrent()"
          flat
          no-caps
          icon="sym_o_unsubscribe"
          label="Відписатися"
          :disable="!auth.currentMessage.value?.unsubscribe"
          :loading="auth.isUnsubscribing.value" />
        <q-btn
          @click="auth.saveCurrent()"
          flat
          no-caps
          icon="sym_o_bookmark_add"
          label="Зберегти"
          :disable="!auth.currentMessage.value"
          :loading="auth.isSaving.value" />
        <q-btn
          @click="newsletterViewRef?.flagAsTask()"
          flat
          no-caps
          icon="sym_o_task_alt"
          label="Задача"
          :disable="!auth.currentMessage.value" />
        <q-space />
        <q-btn
          @click="auth.trashCurrent()"
          flat
          no-caps
          icon="sym_o_delete"
          label="Видалити"
          :disable="!auth.currentMessage.value"
          :loading="auth.isTrashing.value" />
        <q-btn
          @click="auth.loadRandomMessage()"
          flat
          no-caps
          icon="sym_o_skip_next"
          label="Показати інший"
          :loading="auth.isMessageLoading.value" />
        <q-toggle
          @update:model-value="toggleOnlyNewsletters"
          :model-value="auth.onlyNewsletters.value"
          label="Тільки розсилки" />
        <q-btn
          @click="showActionLog = true"
          flat
          no-caps
          icon="sym_o_history"
          label="Журнал"
          :color="auth.actionLog.value.length ? 'primary' : undefined" />
        <q-btn @click="showTemplates = true" flat no-caps icon="sym_o_layers" label="Шаблони" />
        <q-btn @click="showPlugins = true" flat no-caps icon="sym_o_extension" label="Плагіни" />
        <q-btn @click="agentOpen = true" flat no-caps icon="sym_o_smart_toy" label="Агент" color="primary" />
        <q-btn @click="auditOpen = true" flat no-caps icon="sym_o_manage_search" title="Журнал запитів" />
        <q-btn flat no-caps round icon="sym_o_more_vert">
          <q-tooltip>Меню</q-tooltip>
          <q-menu anchor="top right" self="bottom right">
            <q-list style="min-width: 180px">
              <q-item v-close-popup @click="showFilters = true" clickable>
                <q-item-section avatar><q-icon name="sym_o_filter_alt" /></q-item-section>
                <q-item-section>Фільтри Gmail</q-item-section>
              </q-item>
              <q-item v-close-popup @click="showLlmSettings = true" clickable>
                <q-item-section avatar><q-icon name="sym_o_smart_toy" /></q-item-section>
                <q-item-section>Налаштування LLM</q-item-section>
              </q-item>
              <q-separator />
              <q-item v-close-popup @click="auth.logout()" clickable>
                <q-item-section avatar><q-icon name="sym_o_logout" /></q-item-section>
                <q-item-section>Вийти</q-item-section>
              </q-item>
            </q-list>
          </q-menu>
        </q-btn>
      </q-toolbar>
    </q-page-sticky>

    <TemplatesManager v-model="showTemplates" />
    <PluginManagerPanel v-model="showPlugins" />
    <GmailFiltersDialog v-model="showFilters" />
    <LlmSettingsDialog v-model="showLlmSettings" />

    <q-dialog v-model="showActionLog">
      <q-card style="min-width: 480px; max-width: 90vw">
        <q-card-section class="text-h6 row items-center">
          Журнал дій
          <q-space />
          <q-btn v-close-popup flat round dense icon="sym_o_close" />
        </q-card-section>
        <q-separator />
        <q-card-section style="max-height: 60vh; overflow-y: auto">
          <div v-if="!auth.actionLog.value.length" class="text-grey-6">Журнал порожній.</div>
          <q-list v-else separator>
            <q-item v-for="(entry, i) in auth.actionLog.value" :key="i" dense>
              <q-item-section>
                <q-item-label>{{ entry.text }}</q-item-label>
                <q-item-label caption>{{ new Date(entry.ts).toLocaleString('uk-UA') }}</q-item-label>
              </q-item-section>
            </q-item>
          </q-list>
        </q-card-section>
      </q-card>
    </q-dialog>

    <AgentDialog v-model="agentOpen" :agent="agent" />
    <AuditAnalysisDialog v-model="auditOpen" :agent="agent" />
  </q-page>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { AgentDialog } from '@7n/tauri-components/components'
import { errorMessage } from '../i18n/auth-errors.js'
import { useAuthStore } from '../services/auth-store.js'
import { useAgent } from '../composables/use-agent.js'
import AuditAnalysisDialog from '../components/AuditAnalysisDialog.vue'
import NewsletterView from '../components/NewsletterView.vue'
import PluginManagerPanel from '../components/PluginManagerPanel.vue'
import TemplatesManager from '../components/TemplatesManager.vue'
import GmailFiltersDialog from '../components/GmailFiltersDialog.vue'
import LlmSettingsDialog from '../components/LlmSettingsDialog.vue'

const auth = useAuthStore()
const agent = useAgent()
const newsletterViewRef = ref(null)

const appVersion = ref('')
onMounted(async () => {
  appVersion.value = await getVersion()
})

watchEffect(() => {
  const email = auth.email.value
  const count = auth.inboxCount.value
  const version = appVersion.value
  const appName = version ? `MyMail v${version}` : 'MyMail'
  let title
  if (email && count !== null) {
    title = `${appName} - ${email} - ${count}`
  } else if (email) {
    title = `${appName} - ${email}`
  } else {
    title = appName
  }
  document.title = title
  invoke('app_set_title', { title })
})
const agentOpen = ref(false)
const auditOpen = ref(false)
onMounted(() => {
  auth.initialize()
  window.addEventListener('message', e => {
    if (['open-url'].includes(e.data?.type) && e.data.url) {
      invoke('app_open_url', { url: e.data.url })
    }
  })
})

// Built with '<' + tag concatenation, not a literal `<script>`/`<style>` tag
// pair, so the named-template pre-transform's HTML-naive tokenizer (which
// runs on the raw file text, before real SFC parsing) can't mistake these
// injected-HTML tags for the surrounding <script setup> block's own tags.
// The concatenation is the point, so no-useless-concat is a false positive here.
const LINK_INTERCEPT_SCRIPT = `
${
  // oxlint-disable-next-line no-useless-concat
  '<' + 'script>'
}
document.addEventListener('click', function(e) {
  var a = e.target.closest('a');
  if (a && a.href && !a.href.startsWith('javascript')) {
    e.preventDefault();
    window.parent.postMessage({ type: 'open-url', url: a.href }, '*');
  }
});
${
  // oxlint-disable-next-line no-useless-concat
  '<' + '/script>'
}`

const LIGHT_BG_STYLE = `${
  // oxlint-disable-next-line no-useless-concat
  '<' + 'style>'
}
:root { color-scheme: light !important; }
html, body { background: #ffffff !important; color: #000000 !important; }
${
  // oxlint-disable-next-line no-useless-concat
  '<' + '/style>'
}`

// Built from parts so the raw source has no bare closing-head-tag substring —
// the named-template pre-transform tokenizes the whole file HTML-naively and
// mistakes that substring as a literal for a real closing tag.
// The escaped slash also keeps this match safe from that tokenizer, while the
// case-insensitive flag handles HTML emitted by different email clients.
const HEAD_CLOSE_RE = /<\/head>/i

const htmlBodyWithInterceptor = computed(() => {
  const html = auth.currentMessage.value?.html_body
  if (!html) return null
  const inject = LIGHT_BG_STYLE + LINK_INTERCEPT_SCRIPT
  return HEAD_CLOSE_RE.test(html) ? html.replace(HEAD_CLOSE_RE, m => inject + m) : inject + html
})

/**
 * Formats a byte count as a human-readable Ukrainian size label (Б/КБ/МБ/ГБ).
 * @param {number} bytes - size in bytes
 * @returns {string} formatted size, e.g. "1.2 МБ"
 */
function formatFileSize(bytes) {
  if (!bytes) return '0 Б'
  const units = ['Б', 'КБ', 'МБ', 'ГБ']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** i
  return `${i === 0 ? value : value.toFixed(1)} ${units[i]}`
}

const showActionLog = ref(false)
const showTemplates = ref(false)
const showFilters = ref(false)
const showPlugins = ref(false)
const showLlmSettings = ref(false)

/**
 * @param {boolean} value whether to request only newsletters
 */
function toggleOnlyNewsletters(value) {
  auth.setOnlyNewsletters(value)
  auth.loadRandomMessage()
}
</script>

<style scoped>
.reader {
  width: 100%;
  align-self: stretch;
  flex: 1;
  min-height: 0;
}

.message-body,
.summary-body {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-family: inherit;
}

.attachments-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.message-html-section {
  padding: 0;
  overflow: hidden;
  flex: 1;
}

.message-iframe {
  width: 100%;
  height: 100%;
  min-height: 500px;
  border: none;
  display: block;
}

.has-bar {
  padding-bottom: calc(64px + env(safe-area-inset-bottom));
}

.action-bar {
  border-top: 1px solid rgb(0 0 0 / 12%);
  padding-bottom: env(safe-area-inset-bottom);
}
</style>
