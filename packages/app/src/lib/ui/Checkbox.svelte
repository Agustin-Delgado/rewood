<script lang="ts">
	import { Checkbox } from '@human-kit/ui/checkbox';
	import Check from '@lucide/svelte/icons/check';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import type { Snippet } from 'svelte';
	import { checkbox } from './recipes';

	type Props = {
		checked?: boolean;
		indeterminate?: boolean;
		onChange?: (checked: boolean) => void;
		disabled?: boolean;
		title?: string;
		class?: string;
		'aria-label'?: string;
		/** The label, drawn at the right of the box and clickable with it. */
		children?: Snippet;
	};

	let {
		checked = $bindable(false),
		indeterminate = false,
		onChange,
		disabled,
		title,
		class: className,
		children,
		'aria-label': ariaLabel
	}: Props = $props();
	const r = checkbox();

	function change(next: boolean) {
		checked = next;
		onChange?.(next);
	}
</script>

{#snippet box()}
	<Checkbox.Root {checked} {indeterminate} {disabled} onCheckedChange={change} aria-label={ariaLabel} class={r.root()}>
		<Checkbox.Indicator class={r.indicator()}>
			{#if indeterminate}<MinusIcon />{:else}<Check />{/if}
		</Checkbox.Indicator>
	</Checkbox.Root>
{/snippet}

{#if children}
	<label class={r.label({ class: className })} {title}>{@render box()}{@render children()}</label>
{:else}
	<span class="inline-flex {className ?? ''}" {title}>{@render box()}</span>
{/if}
