<script lang="ts">
	import { Tabs } from '@human-kit/ui/tabs';
	import type { Snippet } from 'svelte';
	import { tabs } from './recipes';
	import { getTabsVariant } from './tabs-context';

	/** `end` holds whatever sits at the right of the strip (a status, an action). */
	type Props = { class?: string; children: Snippet; end?: Snippet; 'aria-label': string };
	let { class: className, children, end, ...rest }: Props = $props();
	const r = $derived(tabs({ variant: getTabsVariant() }));
</script>

<div class={r.bar({ class: className })}>
	<Tabs.List class={r.list()} {...rest}>
		{@render children()}
		<Tabs.Indicator class={r.indicator()} />
	</Tabs.List>
	{#if end}<div class={r.end()}>{@render end()}</div>{/if}
</div>
