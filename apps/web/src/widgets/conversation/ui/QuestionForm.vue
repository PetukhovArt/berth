<script setup lang="ts">
import { computed, reactive, shallowRef, useTemplateRef } from 'vue';
import { VButton, VInputText, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import type { AcpAnswers, AcpQuestion, AcpQuestionForm } from '../../../entities/session';
import { useDigitKeys } from '../model/digitKey';

const props = defineProps<{ form: AcpQuestionForm; typed: boolean; panelId?: string }>();
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

const root = useTemplateRef<HTMLElement>('root');

useDigitKeys(root, () => props.panelId, (index) => {
  const choice = instant.value ? props.form.questions[0]!.choices[index] : undefined;
  if (!choice) return false;
  choose(props.form.questions[0]!, choice.value);
  return true;
});
</script>

<template>
  <div ref="root" class="form" data-testid="question-form">
    <fieldset v-for="question in form.questions" :key="question.id" class="question">
      <legend v-if="!instant" class="text">{{ question.text }}</legend>
      <div class="options">
        <VButton v-for="(choice, index) in question.choices" :key="choice.value" v-tooltip="choice.description ?? undefined" class="option" size="small" :severity="(instant ? index === 0 : picked[question.id]?.includes(choice.value)) ? undefined : 'secondary'" :aria-pressed="instant ? undefined : !!picked[question.id]?.includes(choice.value)" data-testid="question-option" @click="choose(question, choice.value)">
          {{ choice.name }}
          <kbd v-if="instant && index < 9" class="key">{{ index + 1 }}</kbd>
        </VButton>
      </div>
      <VInputText v-if="typed && question.free_text_allowed" v-model="text[question.id]" class="free" size="small" :placeholder="t('chat.question.placeholder')" :aria-label="t('chat.question.placeholder')" data-testid="question-text" @keydown.enter.prevent="enter(question)" />
    </fieldset>
    <VButton v-if="!instant" class="submit" :label="t('chat.question.submit')" size="small" :disabled="!complete" data-testid="question-submit" @click="emit('submit', answers)" />
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
  gap: var(--vr-space-2);
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
  width: 100%;
}
</style>
