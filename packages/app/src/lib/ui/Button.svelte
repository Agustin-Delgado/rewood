<script lang="ts">
	import { Button } from '@human-kit/ui/button';
	import type { ComponentProps, Snippet } from 'svelte';
	import { button, type ButtonVariants } from './recipes';

	/** A button; with `href` it is a link that looks like one. */
	type Props = Omit<ComponentProps<typeof Button.Root>, 'class' | 'children'> &
		ButtonVariants & { class?: string; href?: string; target?: string; children?: Snippet };

	let { variant, size, class: className, href, target, children, ...rest }: Props = $props();
</script>

{#if href}
	<a {href} {target} rel={target === '_blank' ? 'noopener' : undefined} class={button({ variant, size, class: className })} title={rest.title}>{@render children?.()}</a>
{:else}
	<Button.Root class={button({ variant, size, class: className })} {...rest}>
		{@render children?.()}
	</Button.Root>
{/if}
