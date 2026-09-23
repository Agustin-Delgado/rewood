<script lang="ts" module>
	import type { Snippet } from 'svelte';

	export type SegmentedOption<T extends string = string> = {
		value: T;
		label?: string;
		title?: string;
		disabled?: boolean;
		icon?: Snippet;
	};
</script>

<script lang="ts" generics="V extends string">
	import { ToggleGroup } from '@human-kit/ui/toggle-group';
	import { segmented } from './recipes';

	type Props = {
		options: SegmentedOption<V>[];
		value: V | null | undefined;
		onChange?: (value: V) => void;
		size?: 'xs' | 'sm' | 'md';
		/** Stretch to the width of the container, the choices sharing it. */
		fill?: boolean;
		disabled?: boolean;
		class?: string;
		'aria-label': string;
	};

	let { options, value = $bindable(), onChange, size, fill, disabled, class: className, ...rest }: Props = $props();

	const r = $derived(segmented({ size, fill }));

	function change(next: (string | number)[]) {
		const picked = next[0] as V | undefined;
		if (picked === undefined || picked === value) return;
		value = picked;
		onChange?.(picked);
	}
</script>

<ToggleGroup.Root
	selectionMode="single"
	disallowEmptySelection
	value={value == null ? [] : [value]}
	onChange={change}
	{disabled}
	class={r.root({ class: className })}
	{...rest}
>
	{#each options as o (o.value)}
		<ToggleGroup.Item value={o.value} title={o.title} disabled={o.disabled} class={r.item()}>
			{@render o.icon?.()}
			{#if o.label}<span class="truncate">{o.label}</span>{/if}
		</ToggleGroup.Item>
	{/each}
</ToggleGroup.Root>
