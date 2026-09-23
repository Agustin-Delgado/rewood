<script lang="ts">
	import { Tabs } from '@human-kit/ui/tabs';
	import type { Snippet } from 'svelte';
	import { tabs } from './recipes';
	import { setTabsVariant } from './tabs-context';

	type Props = {
		value?: string | null;
		onChange?: (value: string) => void;
		variant?: 'line' | 'pill';
		class?: string;
		children: Snippet;
	};

	let { value = $bindable(), onChange, variant = 'line', class: className, children }: Props = $props();
	setTabsVariant(() => variant);
</script>

<Tabs.Root
	bind:value
	onChange={(v) => v != null && onChange?.(String(v))}
	class={tabs({ variant }).root({ class: className })}
>
	{@render children()}
</Tabs.Root>
