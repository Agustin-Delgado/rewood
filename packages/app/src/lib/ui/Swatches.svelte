<script lang="ts" module>
	export type SwatchOption = {
		value: string;
		label: string;
		/** `#rrggbb`: the sample's colour. */
		hex: string;
		/** Shown under the name in the tooltip (a maker's code). */
		hint?: string;
		/** Draw a grain over the sample (a wood print). */
		grain?: boolean;
	};
</script>

<script lang="ts">
	import { ToggleGroup } from '@human-kit/ui/toggle-group';
	import Tooltip from './Tooltip.svelte';
	import { swatches } from './recipes';

	/**
	 * Pick one colour from samples. Each sample names itself in a tooltip
	 * and to a screen reader; the colour is data (a library's), not a token.
	 */
	type Props = {
		options: SwatchOption[];
		value: string | null | undefined;
		onChange?: (value: string) => void;
		disabled?: boolean;
		class?: string;
		'aria-label': string;
	};
	let { options, value, onChange, disabled, class: className, ...rest }: Props = $props();

	const r = swatches();

	function change(next: (string | number)[]) {
		const picked = next[0] as string | undefined;
		if (picked === undefined || picked === value) return;
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
		<Tooltip text={o.hint ? `${o.label} · ${o.hint}` : o.label} delay={150}>
			<ToggleGroup.Item value={o.value} aria-label={o.label} class={r.item()} style="background-color: {o.hex}">
				{#if o.grain}<span class={r.grain()}></span>{/if}
			</ToggleGroup.Item>
		</Tooltip>
	{/each}
</ToggleGroup.Root>
