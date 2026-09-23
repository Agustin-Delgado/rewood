<script lang="ts">
	import { Tabs } from '@human-kit/ui/tabs';
	import type { Snippet } from 'svelte';
	import { tabs } from './recipes';
	import { getTabsVariant } from './tabs-context';

	type Props = { value: string; disabled?: boolean; title?: string; class?: string; children: Snippet };
	let { class: className, children, ...rest }: Props = $props();
	const r = $derived(tabs({ variant: getTabsVariant() }));
</script>

<Tabs.Tab class={r.tab({ class: className })} {...rest}>
	{@render children()}
</Tabs.Tab>
