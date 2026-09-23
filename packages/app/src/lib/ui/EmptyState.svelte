<script lang="ts">
	import type { Snippet } from 'svelte';

	/** What a panel says when it has nothing to show, and what to do about it. */
	type Props = { title: string; description?: string; icon?: Snippet; children?: Snippet; class?: string };
	let { title, description, icon, children, class: className }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center gap-1.5 px-6 py-8 text-center {className ?? ''}">
	{#if icon}<div class="mb-1 text-subtle-foreground [&_svg]:size-6">{@render icon()}</div>{/if}
	<p class="text-sm font-medium text-foreground">{title}</p>
	{#if description}<p class="max-w-72 text-xs text-muted-foreground">{description}</p>{/if}
	{#if children}<div class="mt-2 flex gap-2">{@render children()}</div>{/if}
</div>
