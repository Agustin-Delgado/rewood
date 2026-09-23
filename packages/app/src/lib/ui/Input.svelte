<script lang="ts">
	import { Input } from '@human-kit/ui/input';
	import type { ComponentProps } from 'svelte';
	import { input } from './recipes';

	type Props = Omit<ComponentProps<typeof Input>, 'class' | 'size'> & {
		class?: string;
		size?: 'xs' | 'sm' | 'md';
		mono?: boolean;
	};

	let { value = $bindable(), element = $bindable(null), size, mono, class: className, ...rest }: Props = $props();
</script>

<Input bind:value bind:element class={input({ size, mono, class: className })} {...rest} />
