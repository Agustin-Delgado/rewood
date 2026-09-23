<script lang="ts">
	import type { Snippet } from 'svelte';
	import { badge, type BadgeVariants } from './recipes';

	type Props = BadgeVariants & { class?: string; title?: string; children: Snippet };
	let { tone, outline, class: className, title, children }: Props = $props();
</script>

<span class={badge({ tone, outline, class: className })} {title}>{@render children()}</span>
