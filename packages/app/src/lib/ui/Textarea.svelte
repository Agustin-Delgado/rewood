<script lang="ts">
	import { TextArea } from '@human-kit/ui/textarea';
	import type { ComponentProps } from 'svelte';
	import { textarea } from './recipes';

	type Props = Omit<ComponentProps<typeof TextArea>, 'class'> & { class?: string; mono?: boolean };
	let { value = $bindable(), mono, class: className, ...rest }: Props = $props();
</script>

<TextArea bind:value class={textarea({ mono, class: className })} {...rest} />
