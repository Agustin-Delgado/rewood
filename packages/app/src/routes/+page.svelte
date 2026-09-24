<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import JSZip from 'jszip';
	import Download from '@lucide/svelte/icons/download';
	import FileJson from '@lucide/svelte/icons/file-json';
	import FileText from '@lucide/svelte/icons/file-text';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import OctagonAlert from '@lucide/svelte/icons/octagon-alert';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { app } from '$lib/state.svelte';
	import { findVariant } from '$lib/catalog';
	import { Badge, Button, Tabs, TabsList, TabsPanel, TabsTab } from '$lib/ui';
	import Viewer from '$lib/components/Viewer.svelte';
	import Tree from '$lib/components/Tree.svelte';
	import Properties from '$lib/components/Properties.svelte';
	import Components from '$lib/components/Components.svelte';
	import Library from '$lib/components/Library.svelte';
	import Catalog from '$lib/components/Catalog.svelte';
	import Design from '$lib/components/Design.svelte';
	import Bottom from '$lib/components/Bottom.svelte';

	let rightTab: 'design' | 'props' | 'comps' | 'library' = $state('design');
	const current = $derived(app.variant ? findVariant(app.variant) : null);
	// A part clicked in the viewer is described in Parámetros.
	$effect(() => {
		const picked = app.selectedPart ?? app.selectedFastener;
		if (picked && untrack(() => rightTab) === 'design') rightTab = 'props';
	});
	let loading = $state(true);
	let fileInput: HTMLInputElement | undefined = $state();

	onMount(async () => {
		await app.init();
		loading = false;
	});

	async function downloadPackage() {
		if (!app.engine) return;
		const files = app.engine.packageFiles(app.effectiveSpec());
		const zip = new JSZip();
		for (const f of files) zip.file(f.path, f.contents);
		const blob = await zip.generateAsync({ type: 'blob' });
		const a = document.createElement('a');
		a.href = URL.createObjectURL(blob);
		a.download = `${app.spec.id}-v${app.spec.version ?? '1.0'}.zip`;
		a.click();
		URL.revokeObjectURL(a.href);
	}

	function openReport() {
		if (!app.engine) return;
		const report = app.engine.packageFiles(app.effectiveSpec()).find((f) => f.path === 'documentation/report.html');
		if (!report) return;
		const url = URL.createObjectURL(new Blob([report.contents], { type: 'text/html' }));
		window.open(url, '_blank');
	}

	function openFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		file.text().then((t) => app.openSpecFile(t));
		input.value = '';
	}
</script>

<svelte:head><title>rewood</title></svelte:head>

<div
	class="grid h-screen grid-cols-[280px_minmax(0,1fr)_360px] grid-rows-[44px_minmax(0,1fr)_240px] bg-background
		[grid-template-areas:'header_header_header'_'left_main_right'_'footer_footer_footer']"
>
	<header class="flex items-center gap-2 border-b bg-depth-0 px-3 [grid-area:header]">
		<span class="mr-2 flex items-center gap-1.5 select-none">
			<span class="grid size-5 place-items-center rounded-sm bg-primary text-2xs font-bold text-primary-foreground raised">r</span>
			<span class="text-sm font-semibold tracking-tight">rewood</span>
		</span>
		<Button class="catalog-btn" onclick={() => (app.catalogOpen = true)} disabled={loading} title="Elegir otro mueble">
			<LayoutGrid class="text-primary" />
			{#if current}
				<span class="font-normal text-muted-foreground">{current.category.name}</span>
				<ChevronRight class="-mx-1 text-subtle-foreground" />
				<span>{current.variant.name}</span>
			{:else}
				<span>{app.spec.name}</span>
			{/if}
		</Button>
		<Button variant="ghost" onclick={() => fileInput?.click()} title="Abrir una spec (.json) del disco">
			<FileJson />abrir spec…
		</Button>
		<input bind:this={fileInput} class="hidden" type="file" accept="application/json" onchange={openFile} />
		<span class="flex-1"></span>
		{#if app.current}
			<Badge outline class="font-mono">
				{app.current.id} · v{app.current.version}{app.dirty ? ' · sin guardar' : ''}
			</Badge>
		{/if}
		<Button onclick={openReport} disabled={loading}><FileText />Informe</Button>
		{#if app.plan?.manufacturingBlocked}
			<Button
				variant="primary"
				class="border-danger/40 bg-danger"
				disabled
				title="Hay hallazgos fatales: el paquete no lleva programas ni DXF hasta corregirlos"
			>
				<OctagonAlert />Fabricación bloqueada
			</Button>
		{:else}
			<Button variant="primary" onclick={downloadPackage} disabled={loading}><Download />Descargar paquete (.zip)</Button>
		{/if}
		{#if app.engine}
			<span class="ml-1 font-mono text-2xs text-subtle-foreground">motor {app.engine.engineVersion()}</span>
		{/if}
		<a href="/sistema" class="text-2xs text-subtle-foreground hover:text-foreground hover:underline" title="Sistema de diseño">sistema</a>
	</header>
	{#if loading}
		<div class="flex items-center justify-center gap-2 text-sm text-muted-foreground [grid-area:main]">
			<LoaderCircle class="size-4 animate-spin" />Cargando el motor…
		</div>
	{:else}
		<aside class="overflow-hidden border-r bg-depth-0 [grid-area:left]"><Tree /></aside>
		<main class="overflow-hidden [grid-area:main]"><Viewer /></main>
		<aside class="flex flex-col overflow-hidden border-l bg-depth-0 [grid-area:right]">
			<Tabs value={rightTab} onChange={(v) => (rightTab = v as typeof rightTab)} class="h-full">
				<TabsList aria-label="Panel lateral">
					<TabsTab value="design">Diseño</TabsTab>
					<TabsTab value="props">Parámetros</TabsTab>
					<TabsTab value="comps">Componentes</TabsTab>
					<TabsTab value="library">Biblioteca</TabsTab>
				</TabsList>
				<TabsPanel value="design" class="overflow-hidden"><Design /></TabsPanel>
				<TabsPanel value="props" class="overflow-hidden"><Properties /></TabsPanel>
				<TabsPanel value="comps" class="overflow-hidden"><Components /></TabsPanel>
				<TabsPanel value="library" class="overflow-hidden"><Library /></TabsPanel>
			</Tabs>
		</aside>
		<footer class="overflow-hidden border-t bg-depth-0 [grid-area:footer]"><Bottom /></footer>
		<Catalog />
	{/if}
</div>
