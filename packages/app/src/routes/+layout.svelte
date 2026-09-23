<script lang="ts">
	import '../app.css';
	import { LocaleProvider } from '@human-kit/ui/locale-provider';
	import favicon from '$lib/assets/favicon.svg';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<LocaleProvider locale="es-AR">
	{@render children()}
</LocaleProvider>
