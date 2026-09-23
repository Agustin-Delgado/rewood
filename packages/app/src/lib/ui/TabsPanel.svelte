<script lang="ts">
	import { Tabs } from '@human-kit/ui/tabs';
	import type { Snippet } from 'svelte';
	import { tabs } from './recipes';

	type Props = { value: string; class?: string; children: Snippet };
	let { class: className, children, ...rest }: Props = $props();
</script>

<Tabs.Panel class={tabs().panel({ class: className })} {...rest}>
	{@render children()}
</Tabs.Panel>
