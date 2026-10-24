<script setup lang="ts" generic="T extends string">
// A select with a styled list, following the WAI-ARIA listbox pattern: the
// button opens it, arrow keys move, Enter or a click picks, Escape closes.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import PlIcon from './PlIcon.vue'

const props = withDefaults(
  defineProps<{
    modelValue: T | ''
    options: { value: T; label: string; hint?: string }[]
    /** Accessible name, and the text shown when nothing is selected. */
    label: string
    placeholder?: string
  }>(),
  { placeholder: '' },
)
const emit = defineEmits<{ (e: 'update:modelValue', value: T): void }>()

const open = ref(false)
const active = ref(0)
const root = ref<HTMLElement>()
const list = ref<HTMLElement>()
const button = ref<HTMLButtonElement>()
const id = `pl-select-${Math.random().toString(36).slice(2, 8)}`

const selectedIndex = computed(() => props.options.findIndex((o) => o.value === props.modelValue))
const selected = computed(() => props.options[selectedIndex.value])

function show() {
  active.value = Math.max(0, selectedIndex.value)
  open.value = true
  nextTick(() => {
    list.value?.focus()
    scrollActiveIntoView()
  })
}

function hide(focusButton = true) {
  open.value = false
  if (focusButton) button.value?.focus()
}

function pick(index: number) {
  const option = props.options[index]
  if (option) emit('update:modelValue', option.value)
  hide()
}

function scrollActiveIntoView() {
  list.value?.querySelector<HTMLElement>(`[data-index="${active.value}"]`)?.scrollIntoView({ block: 'nearest' })
}

function onListKey(event: KeyboardEvent) {
  const last = props.options.length - 1
  const moves: Record<string, () => number> = {
    ArrowDown: () => Math.min(last, active.value + 1),
    ArrowUp: () => Math.max(0, active.value - 1),
    Home: () => 0,
    End: () => last,
    PageDown: () => Math.min(last, active.value + 5),
    PageUp: () => Math.max(0, active.value - 5),
  }
  if (moves[event.key]) {
    event.preventDefault()
    active.value = moves[event.key]()
    scrollActiveIntoView()
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    pick(active.value)
  } else if (event.key === 'Escape') {
    event.preventDefault()
    hide()
  } else if (event.key === 'Tab') {
    hide(false)
  }
}

function onButtonKey(event: KeyboardEvent) {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    show()
  }
}

function onOutside(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) hide(false)
}

watch(open, (isOpen) => {
  if (isOpen) document.addEventListener('pointerdown', onOutside)
  else document.removeEventListener('pointerdown', onOutside)
})
onBeforeUnmount(() => document.removeEventListener('pointerdown', onOutside))
</script>

<template>
  <div ref="root" class="pl-select" :class="{ open }">
    <button
      ref="button"
      type="button"
      class="trigger"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="`${id}-list`"
      :aria-label="`${label}: ${selected?.label ?? placeholder}`"
      @click="open ? hide() : show()"
      @keydown="onButtonKey"
    >
      <span class="value" :class="{ placeholder: !selected }">{{ selected?.label ?? placeholder }}</span>
      <PlIcon name="chevron-down" class="chevron" />
    </button>
    <Transition name="pl-pop">
      <ul
        v-show="open"
        :id="`${id}-list`"
        ref="list"
        class="list"
        role="listbox"
        tabindex="-1"
        :aria-label="label"
        :aria-activedescendant="`${id}-${active}`"
        @keydown="onListKey"
      >
        <li
          v-for="(option, i) in options"
          :id="`${id}-${i}`"
          :key="option.value"
          :data-index="i"
          role="option"
          class="option"
          :class="{ active: i === active, selected: i === selectedIndex }"
          :aria-selected="i === selectedIndex"
          @pointermove="active = i"
          @click="pick(i)"
        >
          <span v-if="option.hint" class="hint">{{ option.hint }}</span>
          <span class="label">{{ option.label }}</span>
          <PlIcon v-if="i === selectedIndex" name="check" class="check" />
        </li>
      </ul>
    </Transition>
  </div>
</template>

<style scoped>
.pl-select {
  position: relative;
  min-width: 0;
}

.trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 36px;
  padding: 0 10px 0 12px;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius);
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  font-size: 14px;
  font-weight: 500;
  text-align: left;
  transition: border-color 0.15s, background-color 0.15s;
}

.trigger:hover {
  border-color: var(--pl-border-strong);
}

.open .trigger {
  border-color: var(--vp-c-brand-2);
}

.value {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.placeholder {
  color: var(--vp-c-text-3);
}

.chevron {
  color: var(--vp-c-text-3);
  transition: transform 0.15s;
}

.open .chevron {
  transform: rotate(180deg);
}

.list {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 30;
  min-width: 100%;
  max-height: min(360px, 60vh);
  margin: 0;
  padding: 4px;
  overflow-y: auto;
  list-style: none;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  background: var(--vp-c-bg-elv);
  box-shadow: var(--pl-shadow-pop);
  outline: none;
}

.option {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 34px;
  padding: 6px 10px;
  border-radius: var(--pl-radius-sm);
  font-size: 14px;
  white-space: nowrap;
  color: var(--vp-c-text-1);
  cursor: pointer;
}

.option.active {
  background: var(--vp-c-default-soft);
}

.option.selected {
  color: var(--vp-c-brand-1);
  font-weight: 500;
}

.hint {
  min-width: 18px;
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: var(--vp-c-text-3);
  font-variant-numeric: tabular-nums;
}

.label {
  flex: 1;
}

.check {
  color: var(--vp-c-brand-1);
}

.pl-pop-enter-active,
.pl-pop-leave-active {
  transition: opacity 0.12s, transform 0.12s;
}

.pl-pop-enter-from,
.pl-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
