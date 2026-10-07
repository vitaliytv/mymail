<template>
  <q-dialog v-model="show" maximized>
    <q-card>
      <q-bar>
        <span class="text-weight-medium">Фільтри Gmail</span>
        <q-space />
        <q-btn v-close-popup flat round dense icon="sym_o_close" />
      </q-bar>

      <q-card-section>
        <q-banner v-if="auth.filtersErrorKind.value" class="bg-red-1 text-red-9" rounded dense>
          {{ errorMessage(auth.filtersErrorKind.value) }}
        </q-banner>
        <q-banner v-if="auth.deleteFilterErrorKind.value" class="bg-red-1 text-red-9 q-mt-sm" rounded dense>
          {{ errorMessage(auth.deleteFilterErrorKind.value) }}
        </q-banner>

        <div v-if="auth.isLoadingFilters.value" class="text-grey-6 q-pa-md text-center">
          <q-spinner color="primary" size="24px" />
        </div>
        <div v-else-if="!auth.filters.value.length && !auth.filtersErrorKind.value" class="text-grey-6">
          Фільтрів ще немає.
        </div>
        <template v-else>
          <q-input
            v-model="search"
            debounce="200"
            dense
            outlined
            clearable
            clear-icon="sym_o_close"
            placeholder="Пошук по фільтрах…"
            class="q-mb-sm"
            prepend-icon="sym_o_search" />
          <div v-if="!filteredFilters.length" class="text-grey-6">Нічого не знайдено.</div>
          <q-list v-else bordered separator rounded>
            <q-item v-for="f in filteredFilters" :key="f.id">
              <q-item-section>
                <q-item-label>{{ criteriaLabel(f) }}</q-item-label>
              </q-item-section>
              <q-item-section side>
                <div class="row items-center q-gutter-xs">
                  <q-icon v-for="(a, i) in actionIcons(f)" :key="i" :name="a.icon" size="20px" color="grey-7">
                    <q-tooltip>{{ a.tooltip }}</q-tooltip>
                  </q-icon>
                </div>
              </q-item-section>
              <q-item-section side>
                <q-btn
                  @click="confirmDelete(f)"
                  flat
                  dense
                  round
                  icon="sym_o_delete"
                  color="negative"
                  :loading="auth.isDeletingFilterId.value === f.id"
                  :disable="!!auth.isDeletingFilterId.value" />
              </q-item-section>
            </q-item>
          </q-list>
        </template>
      </q-card-section>
    </q-card>
  </q-dialog>
</template>

<script setup>
import { useQuasar } from 'quasar'
import { errorMessage } from '../i18n/auth-errors.js'
import { useAuthStore } from '../services/auth-store.js'

const show = defineModel({ type: Boolean, default: false })
const auth = useAuthStore()
const $q = useQuasar()
const search = ref('')

watch(show, v => {
  if (v) {
    auth.listFilters()
    auth.listLabels()
  }
  if (!v) search.value = ''
})

const SIZE_COMPARISON_LABEL = { larger: 'більше', smaller: 'менше' }

const SYSTEM_ACTION_ICONS = {
  'add:TRASH': { icon: 'sym_o_delete', tooltip: 'У кошик' },
  'add:SPAM': { icon: 'sym_o_report', tooltip: 'Позначити як спам' },
  'remove:INBOX': { icon: 'sym_o_archive', tooltip: 'Архівувати (пропустити вхідні)' },
  'remove:UNREAD': { icon: 'sym_o_mark_email_read', tooltip: 'Позначити прочитаним' },
  'add:UNREAD': { icon: 'sym_o_mark_email_unread', tooltip: 'Позначити непрочитаним' },
  'add:IMPORTANT': { icon: 'sym_o_label_important', tooltip: 'Позначити важливим' },
  'remove:IMPORTANT': { icon: 'sym_o_label_important', tooltip: 'Ніколи не позначати важливим' },
  'add:STARRED': { icon: 'sym_o_star', tooltip: 'Позначити зіркою' },
  'add:CATEGORY_PERSONAL': { icon: 'sym_o_sell', tooltip: 'Категорія: Особисті' },
  'add:CATEGORY_SOCIAL': { icon: 'sym_o_sell', tooltip: 'Категорія: Соцмережі' },
  'add:CATEGORY_PROMOTIONS': { icon: 'sym_o_sell', tooltip: 'Категорія: Акції' },
  'add:CATEGORY_UPDATES': { icon: 'sym_o_sell', tooltip: 'Категорія: Оновлення' },
  'add:CATEGORY_FORUMS': { icon: 'sym_o_sell', tooltip: 'Категорія: Форуми' }
}

const labelNameById = computed(() => Object.fromEntries(auth.labels.value.map(l => [l.id, l.name])))

/**
 * @param {string} id Gmail label id
 * @returns {string} the label's display name, or the id itself if unknown
 */
function labelName(id) {
  return labelNameById.value[id] ?? id
}

/**
 * Icons (with tooltips) describing what a filter's action does, without
 * growing the row height the way a text label would.
 * @param {{ action?: object }} f the filter
 * @returns {{ icon: string, tooltip: string }[]} one entry per action effect
 */
function actionIcons(f) {
  const a = f.action ?? {}
  const icons = Array.from(
    a.addLabelIds ?? [],
    id => SYSTEM_ACTION_ICONS[`add:${id}`] ?? { icon: 'sym_o_label', tooltip: `Мітка: ${labelName(id)}` }
  )
  for (const id of a.removeLabelIds ?? []) {
    icons.push(
      SYSTEM_ACTION_ICONS[`remove:${id}`] ?? { icon: 'sym_o_label_off', tooltip: `Зняти мітку: ${labelName(id)}` }
    )
  }
  if (a.forward) icons.push({ icon: 'sym_o_forward_to_inbox', tooltip: `Переслати: ${a.forward}` })
  return icons
}

/**
 * @param {{ criteria?: object }} f the filter
 * @returns {string} human-readable summary of the filter's match conditions
 */
function criteriaLabel(f) {
  const c = f.criteria ?? {}
  const parts = []
  if (c.from) parts.push(`від: ${c.from}`)
  if (c.to) parts.push(`кому: ${c.to}`)
  if (c.subject) parts.push(`тема: ${c.subject}`)
  if (c.query) parts.push(`пошук: ${c.query}`)
  if (c.negatedQuery) parts.push(`не містить: ${c.negatedQuery}`)
  if (c.hasAttachment) parts.push('є вкладення')
  if (c.excludeChats) parts.push('без чатів')
  if (c.size !== undefined && c.size !== null && c.sizeComparison) {
    parts.push(`розмір ${SIZE_COMPARISON_LABEL[c.sizeComparison] ?? c.sizeComparison} ${c.size}`)
  }
  return parts.length ? parts.join(' · ') : '(без умов)'
}

const filteredFilters = computed(() => {
  const q = search.value.trim().toLowerCase()
  if (!q) return auth.filters.value
  return auth.filters.value.filter(f => criteriaLabel(f).toLowerCase().includes(q))
})

/**
 * @param {{ id: string }} f the filter to delete, after user confirmation
 */
function confirmDelete(f) {
  $q.dialog({
    title: 'Видалити фільтр?',
    message: `Фільтр "${criteriaLabel(f)}" буде видалено остаточно.`,
    persistent: true,
    ok: { label: 'Видалити', color: 'negative', flat: true },
    cancel: { label: 'Скасувати', flat: true }
  }).onOk(() => auth.deleteFilter(f.id))
}
</script>
