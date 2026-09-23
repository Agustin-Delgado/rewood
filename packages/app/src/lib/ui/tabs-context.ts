import { getContext, setContext } from 'svelte';

type Variant = 'line' | 'pill';
const key = Symbol('rewood-tabs');

export const setTabsVariant = (v: () => Variant) => setContext(key, v);
export const getTabsVariant = (): Variant => (getContext(key) as (() => Variant) | undefined)?.() ?? 'line';
