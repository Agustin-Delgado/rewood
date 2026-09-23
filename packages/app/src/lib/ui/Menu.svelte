<script lang="ts" module>
	import type { Component } from 'svelte';

	export type MenuEntry =
		| {
				label: string;
				onSelect: () => void;
				hint?: string;
				icon?: Component;
				danger?: boolean;
				disabled?: boolean;
		  }
		| { separator: true }
		| { heading: string };
</script>

<script lang="ts">
	import { Menu } from '@human-kit/ui/menu';
	import type { Snippet } from 'svelte';
	import { button, floating, type ButtonVariants } from './recipes';

	/** A button that opens a list of actions. */
	type Props = {
		items: MenuEntry[];
		trigger: Snippet;
		variant?: ButtonVariants['variant'];
		size?: ButtonVariants['size'];
		disabled?: boolean;
		title?: string;
		class?: string;
		'aria-label'?: string;
		placement?: 'bottom-start' | 'bottom-end' | 'top-start' | 'top-end';
	};

	let {
		items,
		trigger,
		variant = 'ghost',
		size = 'sm',
		disabled,
		title,
		class: className,
		'aria-label': ariaLabel,
		placement = 'bottom-start'
	}: Props = $props();
	const f = floating();
</script>

<Menu.Root>
	<Menu.Trigger {disabled} {title} aria-label={ariaLabel} class={button({ variant, size, class: className })}>
		{@render trigger()}
	</Menu.Trigger>
	<Menu.Content {placement} class={f.content({ class: 'min-w-44' })}>
		{#each items as item, i (i)}
			{#if 'separator' in item}
				<Menu.Separator class={f.separator()} />
			{:else if 'heading' in item}
				<div class={f.label()} role="presentation">{item.heading}</div>
			{:else}
				<Menu.Item
					onAction={item.onSelect}
					disabled={item.disabled}
					textValue={item.label}
					class={f.item({ class: item.danger ? 'text-danger' : '' })}
				>
					{#if item.icon}<item.icon />{/if}
					<span class="min-w-0 truncate">{item.label}</span>
					{#if item.hint}<span class={f.itemHint()}>{item.hint}</span>{/if}
				</Menu.Item>
			{/if}
		{/each}
	</Menu.Content>
</Menu.Root>
