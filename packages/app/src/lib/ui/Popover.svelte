<script lang="ts">
	import { Popover } from '@human-kit/ui/popover';
	import type { Snippet } from 'svelte';
	import { button, floating, type ButtonVariants } from './recipes';

	/** A button that opens a floating panel with arbitrary content. */
	type Props = {
		open?: boolean;
		trigger: Snippet;
		children: Snippet;
		variant?: ButtonVariants['variant'];
		size?: ButtonVariants['size'];
		title?: string;
		class?: string;
		contentClass?: string;
		'aria-label'?: string;
		placement?: 'bottom-start' | 'bottom-end' | 'top-start' | 'top-end' | 'bottom' | 'top';
	};

	let {
		open = $bindable(false),
		trigger,
		children,
		variant = 'secondary',
		size = 'sm',
		title,
		class: className,
		contentClass,
		'aria-label': ariaLabel,
		placement = 'bottom-start'
	}: Props = $props();
	const f = floating();
</script>

<Popover.Root bind:open>
	<Popover.Trigger {title} aria-label={ariaLabel} class={button({ variant, size, class: className })}>
		{@render trigger()}
	</Popover.Trigger>
	<Popover.Content {placement} offset={6} class={f.content({ class: `p-3 ${contentClass ?? ''}` })}>
		{@render children()}
	</Popover.Content>
</Popover.Root>
