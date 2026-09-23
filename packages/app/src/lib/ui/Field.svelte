<script lang="ts">
	import { Label } from '@human-kit/ui/label';
	import type { Snippet } from 'svelte';
	import { fieldRecipe } from './recipes';

	/**
	 * A label, its control and a line of help. `for` ties the label to the
	 * control's `id`; `inline` puts the label at the left, for dense forms.
	 */
	type Props = {
		label: string;
		for?: string;
		hint?: string;
		error?: string | null;
		layout?: 'stack' | 'inline';
		/** Extra content at the right of the label (a value, a reset link). */
		aside?: Snippet;
		children: Snippet;
		class?: string;
	};

	let { label, for: htmlFor, hint, error, layout, aside, children, class: className }: Props = $props();
	const r = $derived(fieldRecipe({ layout }));
</script>

<div class={r.root({ class: className })}>
	{#if aside && layout !== 'inline'}
		<div class="flex items-center justify-between gap-2">
			<Label for={htmlFor} class={r.label()}>{label}</Label>
			{@render aside()}
		</div>
	{:else}
		<Label for={htmlFor} class={r.label()}>{label}</Label>
	{/if}
	{@render children()}
	{#if error}
		<p class={r.error({ class: layout === 'inline' ? 'col-span-2' : '' })}>{error}</p>
	{:else if hint}
		<p class={r.hint({ class: layout === 'inline' ? 'col-span-2' : '' })}>{hint}</p>
	{/if}
</div>
