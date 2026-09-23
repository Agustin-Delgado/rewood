<script lang="ts">
	import { Switch } from '@human-kit/ui/switch';
	import type { Snippet } from 'svelte';
	import { switchRecipe } from './recipes';

	type Props = {
		checked?: boolean;
		onChange?: (checked: boolean) => void;
		disabled?: boolean;
		title?: string;
		class?: string;
		'aria-label'?: string;
		children?: Snippet;
	};

	let {
		checked = $bindable(false),
		onChange,
		disabled,
		title,
		class: className,
		children,
		'aria-label': ariaLabel
	}: Props = $props();
	const r = switchRecipe();

	function change(next: boolean) {
		checked = next;
		onChange?.(next);
	}
</script>

{#snippet control()}
	<Switch.Root {checked} {disabled} onCheckedChange={change} aria-label={ariaLabel} class={r.root()}>
		<Switch.Thumb class={r.thumb()} />
	</Switch.Root>
{/snippet}

{#if children}
	<label class={r.label({ class: className })} {title}>{@render control()}{@render children()}</label>
{:else}
	<span class="inline-flex {className ?? ''}" {title}>{@render control()}</span>
{/if}
