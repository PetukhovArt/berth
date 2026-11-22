<script setup lang="ts">
import { computed } from 'vue';
import { useI18n, type Locale } from '../lib/i18n/i18n';
import { VChipGroup } from './kit';

const { t, locale, setLocale } = useI18n();

const LOCALES: Locale[] = ['en', 'ru'];
const options = computed(() => LOCALES.map((value) => ({ value, label: value.toUpperCase(), title: t(`common.language.${value}`) })));
const selected = computed({
  get: () => locale.value,
  set: (value: string) => {
    const next = LOCALES.find((candidate) => candidate === value);
    if (next) setLocale(next);
  },
});
</script>

<template>
  <VChipGroup v-model="selected" :options="options" :label="t('common.language.label')" test-id="language" data-testid="language-switch" />
</template>
