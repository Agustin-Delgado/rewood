<script lang="ts">
	import { Collapsible } from '@human-kit/ui/collapsible';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { Snippet } from 'svelte';
	import { section } from './recipes';

	/**
	 * A titled block of a side panel. Collapsible unless `static`; `actions` sit
	 * at the right of the title and never toggle it.
	 */
	type Props = {
		title: string;
		count?: number | string;
		open?: boolean;
		static?: boolean;
		actions?: Snippet;
		children: Snippet;
		class?: string;
		bodyClass?: string;
	};

	let {
		title,
		count,
		open = $bindable(true),
		static: fixed = false,
		actions,
		children,
		class: className,
		bodyClass
	}: Props = $props();
	const r = section();
</script>

{#if fixed}
	<section class={r.root({ class: className })}>
		<div class={r.header({ class: 'cursor-default' })}>
			<h3 class={r.title()}>{title}</h3>
			{#if count !== undefined}<span class={r.count()}>{count}</span>{/if}
			{#if actions}<div class={r.actions()}>{@render actions()}</div>{/if}
		</div>
		<div class={r.body({ class: bodyClass })}>{@render children()}</div>
	</section>
{:else}
	<Collapsible.Root bind:open class={r.root({ class: className })}>
		<div class="flex items-center pr-3">
			<Collapsible.Trigger class={r.header({ class: 'flex-1 pr-0' })}>
				<ChevronRight class={r.chevron()} />
				<h3 class={r.title()}>{title}</h3>
				{#if count !== undefined}<span class={r.count()}>{count}</span>{/if}
			</Collapsible.Trigger>
			{#if actions}<div class={r.actions()}>{@render actions()}</div>{/if}
		</div>
		<Collapsible.Panel class={r.body({ class: bodyClass })}>{@render children()}</Collapsible.Panel>
	</Collapsible.Root>
{/if}
