<script lang="ts">
	import { Dialog } from '@human-kit/ui/dialog';
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import { button, dialog } from './recipes';

	/**
	 * A modal with a title bar, a scrolling body and an optional footer. The body
	 * brings its own padding: a grid of cards and a form want different edges.
	 */
	type Props = {
		open: boolean;
		title: string;
		description?: string;
		size?: 'sm' | 'md' | 'lg' | 'xl';
		/** Controls at the right of the title bar, before the close button. */
		actions?: Snippet;
		footer?: Snippet;
		children: Snippet;
		class?: string;
		bodyClass?: string;
	};

	let {
		open = $bindable(false),
		title,
		description,
		size,
		actions,
		footer,
		children,
		class: className,
		bodyClass
	}: Props = $props();
	const r = $derived(dialog({ size }));
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class={r.overlay()} />
		<Dialog.Content class={r.content({ class: className })}>
			<header class={r.header()}>
				<div class="min-w-0 flex-1">
					<Dialog.Title class={r.title()}>{title}</Dialog.Title>
					{#if description}<Dialog.Description class={r.description()}>{description}</Dialog.Description>{/if}
				</div>
				{#if actions}<div class="flex shrink-0 items-center gap-2">{@render actions()}</div>{/if}
				<Dialog.Close class={button({ variant: 'ghost', size: 'icon-sm', class: '-mr-1.5' })} aria-label="Cerrar">
					<X />
				</Dialog.Close>
			</header>
			<div class={r.body({ class: bodyClass })}>{@render children()}</div>
			{#if footer}<footer class={r.footer()}>{@render footer()}</footer>{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
