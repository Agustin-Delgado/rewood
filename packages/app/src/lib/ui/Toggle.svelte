<script lang="ts">
	import { Toggle } from '@human-kit/ui/toggle';
	import type { ComponentProps, Snippet } from 'svelte';
	import { toggle, type ButtonVariants } from './recipes';

	type Props = Omit<ComponentProps<typeof Toggle.Root>, 'class' | 'children'> &
		ButtonVariants & { class?: string; children?: Snippet };

	let { variant, size, class: className, children, selected = $bindable(false), ...rest }: Props = $props();
</script>

<Toggle.Root bind:selected class={toggle({ variant, size, class: className })} {...rest}>
	{@render children?.()}
</Toggle.Root>
