<script lang="ts">
	import { onMount } from 'svelte';
	import JSZip from 'jszip';
	import { app, EXAMPLES } from '$lib/state.svelte';
	import Viewer from '$lib/components/Viewer.svelte';
	import Tree from '$lib/components/Tree.svelte';
	import Properties from '$lib/components/Properties.svelte';
	import Components from '$lib/components/Components.svelte';
	import Assistant from '$lib/components/Assistant.svelte';
	import Bottom from '$lib/components/Bottom.svelte';

	let example = $state(EXAMPLES[0].id);
	let rightTab: 'props' | 'comps' | 'ai' = $state('props');
	let loading = $state(true);

	onMount(async () => {
		await app.init();
		loading = false;
	});

	async function downloadPackage() {
		if (!app.engine) return;
		const files = app.engine.packageFiles(app.spec);
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
		const report = app.engine.packageFiles(app.spec).find((f) => f.path === 'documentation/report.html');
		if (!report) return;
		const url = URL.createObjectURL(new Blob([report.contents], { type: 'text/html' }));
		window.open(url, '_blank');
	}

	function openFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		file.text().then((t) => app.applySpecText(t));
		input.value = '';
	}
</script>

<svelte:head><title>rewood</title></svelte:head>

<div class="shell">
	<header>
		<span class="brand">rewood</span>
		<label>
			ejemplo
			<select bind:value={example} onchange={() => app.loadExample(example)}>
				{#each EXAMPLES as ex (ex.id)}
					<option value={ex.id}>{ex.name}</option>
				{/each}
			</select>
		</label>
		<label class="file">
			abrir spec…
			<input type="file" accept="application/json" onchange={openFile} />
		</label>
		<span class="spacer"></span>
		{#if app.current}
			<span class="current">
				{app.current.id} · v{app.current.version}{app.dirty ? ' · sin guardar' : ''}
			</span>
		{/if}
		<button onclick={openReport} disabled={loading}>Informe</button>
		{#if app.plan?.manufacturingBlocked}
			<button class="primary" disabled title="Hay hallazgos fatales: el paquete no lleva programas ni DXF hasta corregirlos">Fabricación bloqueada</button>
		{:else}
			<button class="primary" onclick={downloadPackage} disabled={loading}>Descargar paquete (.zip)</button>
		{/if}
		{#if app.engine}
			<span class="version">motor {app.engine.engineVersion()}</span>
		{/if}
	</header>
	{#if loading}
		<div class="loading">Cargando el motor…</div>
	{:else}
		<aside class="left"><Tree /></aside>
		<main><Viewer /></main>
		<aside class="right">
			<div class="rtabs">
				<button class:active={rightTab === 'props'} onclick={() => (rightTab = 'props')}>Parámetros</button>
				<button class:active={rightTab === 'comps'} onclick={() => (rightTab = 'comps')}>Componentes</button>
				<button class:active={rightTab === 'ai'} onclick={() => (rightTab = 'ai')}>Asistente</button>
			</div>
			<div class="rbody">
				{#if rightTab === 'props'}<Properties />{:else if rightTab === 'comps'}<Components />{:else}<Assistant />{/if}
			</div>
		</aside>
		<footer><Bottom /></footer>
	{/if}
</div>

<style>
	:global(body) {
		margin: 0;
		font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
		color: #111;
	}
	.shell {
		display: grid;
		grid-template-columns: 300px 1fr 360px;
		grid-template-rows: 40px 1fr 240px;
		grid-template-areas:
			'header header header'
			'left main right'
			'footer footer footer';
		height: 100vh;
	}
	header {
		grid-area: header;
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 12px;
		border-bottom: 1px solid #ddd;
		background: #fff;
		font-size: 13px;
	}
	.brand {
		font-weight: 700;
		letter-spacing: 0.02em;
	}
	.spacer {
		flex: 1;
	}
	.version {
		color: #888;
		font-size: 11px;
	}
	.current {
		color: #555;
		font-family: ui-monospace, monospace;
		font-size: 11px;
	}
	.file input {
		display: none;
	}
	.file {
		cursor: pointer;
		color: #246;
	}
	button {
		font: inherit;
		padding: 4px 10px;
		border: 1px solid #bbb;
		border-radius: 4px;
		background: #fff;
		cursor: pointer;
	}
	button.primary {
		background: #ff8c42;
		border-color: #ff8c42;
		color: #fff;
	}
	.left {
		grid-area: left;
		border-right: 1px solid #ddd;
		overflow: hidden;
	}
	main {
		grid-area: main;
		overflow: hidden;
	}
	.right {
		grid-area: right;
		border-left: 1px solid #ddd;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}
	.rtabs {
		display: flex;
		border-bottom: 1px solid #ddd;
		background: #fafafa;
	}
	.rtabs button {
		border: none;
		border-radius: 0;
		background: none;
		padding: 6px 12px;
		font-size: 12px;
		border-bottom: 2px solid transparent;
	}
	.rtabs button.active {
		border-bottom-color: #ff8c42;
		font-weight: 600;
	}
	.rbody {
		flex: 1;
		overflow: hidden;
	}
	footer {
		grid-area: footer;
		border-top: 1px solid #ddd;
		overflow: hidden;
	}
	.loading {
		grid-area: main;
		display: grid;
		place-items: center;
		color: #666;
	}
</style>
