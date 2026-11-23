import { t } from '../../../shared/lib/i18n/i18n';

const BUILT_IN = [{ id: 'fallback', name: 'Next agent in the fallback chain', label: () => t('router.builtIn.fallback') }];

export const isDefaultName = (id: string, name: string) => BUILT_IN.some((rule) => rule.id === id && rule.name === name);

export function routeRuleLabel(rule: string): string {
  return BUILT_IN.find((known) => known.id === rule || known.name === rule)?.label() ?? rule;
}
