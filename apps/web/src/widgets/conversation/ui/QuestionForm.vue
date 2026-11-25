<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, shallowRef } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import type { AcpAnswers, AcpQuestion, AcpQuestionForm } from '../../../entities/session';
import { digitIndex } from '../model/digitKey';

const props = defineProps<{ form: AcpQuestionForm; typed: boolean }>();
const emit = defineEmits<{ submit: [answers: AcpAnswers] }>();

const picked = shallowRef<Readonly<Record<string, readonly string[]>>>({});
const text = reactive<Record<string, string>>({});

const instant = computed(() => props.form.questions.length === 1 && !props.form.questions[0]!.multi_select);
const answers = computed<AcpAnswers>(() =>
  Object.fromEntries(
    props.form.questions.map((question) => {
      const typedAnswer = props.typed ? (text[question.id] ?? '').trim() : '';
      const chosen = picked.value[question.id] ?? [];
      return [question.id, typedAnswer ? (question.multi_select ? [...chosen, typedAnswer] : [typedAnswer]) : [...chosen]];
    }),
  ),
);
const complete = computed(() => props.form.questions.every((question) => answers.value[question.id]?.length));

function choose(question: AcpQuestion, value: string) {
  if (instant.value) {
    emit('submit', { [question.id]: [value] });
    return;
  }
  const current = picked.value[question.id] ?? [];
  const next = question.multi_select ? (current.includes(value) ? current.filter((entry) => entry !== value) : [...current, value]) : [value];
  picked.value = { ...picked.value, [question.id]: next };
}

function enter(question: AcpQuestion) {
  if (instant.value) {
    const typedAnswer = (text[question.id] ?? '').trim();
    if (typedAnswer) emit('submit', { [question.id]: [typedAnswer] });
  } else if (complete.value) emit('submit', answers.value);
}

function onKey(event: KeyboardEvent) {
  const index = digitIndex(event);
  const choice = instant.value && index !== null ? props.form.questions[0]!.choices[index] : undefined;
  if (!choice) return;
  event.preventDefault();
  choose(props.form.questions[0]!, choice.value);
}

onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <div class="form" data-testid="question-form">
    <fieldset v-for="question in form.questions" :key="question.id" class="question">
      <legend v-if="!instant" class="text">{{ question.text }}</legend>
      <div class="options">
        <UiButton v-for="(choice, index) in question.choices" :key="choice.value" type="button" class="option" :class="{ primary: instant ? index === 0 : picked[question.id]?.includes(choice.value) }" :aria-pressed="instant ? undefined : !!picked[question.id]?.includes(choice.value)" :title="choice.description ?? undefined" data-testid="question-option" @click="choose(question, choice.value)">
          {{ choice.name }}
          <kbd v-if="instant && index < 9" class="key">{{ index + 1 }}</kbd>
        </UiButton>
      </div>
      <UiInput v-if="typed && question.free_text_allowed" class="free" v-model="text[question.id]" :placeholder="t('chat.question.placeholder')" :aria-label="t('chat.question.placeholder')" data-testid="question-text" @keydown.enter.prevent="enter(question)" />
    </fieldset>
    <UiButton v-if="!instant" type="button" class="option primary submit" :disabled="!complete" data-testid="question-submit" @click="emit('submit', answers)">{{ t('chat.question.submit') }}</UiButton>
  </div>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.question {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.text {
  float: left;
  width: 100%;
  margin-bottom: var(--vr-space-2);
  padding: 0;
  font-size: var(--vr-font-size-control);
  font-weight: 500;
}

.options {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  clear: both;
}

.option {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 30px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-control);
  font-weight: 500;
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.option.primary {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.option:disabled {
  cursor: default;
  opacity: 0.5;
}

.submit {
  align-self: flex-start;
}

.key {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-key);
  opacity: 0.6;
}

.free {
  box-sizing: border-box;
  height: 32px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-sunken);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-md);
  outline: none;
}

.free:focus {
  border-color: var(--vr-color-focus);
}
</style>
