<script lang="ts">
	import { Canvas } from '@threlte/core';
	import BoxIcon from '@lucide/svelte/icons/box';
	import DoorOpen from '@lucide/svelte/icons/door-open';
	import Scene from './Scene.svelte';
	import { app, VIEWS } from '$lib/state.svelte';
	import { Segmented, Slider, Toggle } from '$lib/ui';

	const viewOptions = $derived(VIEWS.map((v) => ({ value: v.id, label: v.name, title: v.title })));
</script>

<div class="relative h-full w-full bg-viewport">
	<Canvas>
		<Scene />
	</Canvas>
	<div
		class="absolute top-2.5 left-2.5 flex max-w-[calc(100%-1.25rem)] flex-wrap items-center gap-2 rounded-lg border bg-depth-0/95 p-1 shadow-sm backdrop-blur-sm"
	>
		<div class="flex items-center gap-0.5">
			<Toggle size="xs" bind:selected={app.showHoles} title="Mostrar perforaciones">perforaciones</Toggle>
			<Toggle size="xs" bind:selected={app.showHardware} title="Mostrar herrajes">herrajes</Toggle>
			<Toggle
				size="xs"
				class="open"
				selected={app.openAll}
				onChange={(open) => app.setOpenAll(open)}
				title="Abrir o cerrar puertas y cajones (doble clic en uno lo abre solo a él)"
			>
				<DoorOpen />{app.openAll ? 'cerrar' : 'abrir'}
			</Toggle>
		</div>
		<span class="h-4 w-px bg-border"></span>
		<label class="flex items-center gap-2 pl-1 text-xs text-muted-foreground" title="Vista explotada">
			explotar
			<Slider aria-label="Vista explotada" class="w-24" min={0} max={2} step={0.05} bind:value={app.explode} />
		</label>
		<span class="h-4 w-px bg-border"></span>
		<Segmented aria-label="Vista" size="xs" options={viewOptions} value={app.view} onChange={(v) => app.setView(v)} />
	</div>
	{#if app.selected || app.fastener}
		<div
			class="overlay absolute bottom-2.5 left-2.5 flex max-w-[calc(100%-1.25rem)] items-center gap-2 rounded-lg border bg-depth-0/95 px-2.5 py-1.5 text-xs shadow-sm backdrop-blur-sm"
		>
			<BoxIcon class="size-3.5 shrink-0 text-primary" />
			{#if app.selected}
				<span class="sel truncate font-medium"><span class="font-mono text-2xs text-muted-foreground">{app.selected.id}</span> · {app.selected.name}</span>
			{:else if app.fastener}
				{@const f = app.fastener}
				<span class="sel truncate font-medium">{app.hardwareName(f.fastener.hardware)} · <span class="font-mono text-2xs text-muted-foreground">{f.joint.id}</span></span>
			{/if}
			<span class="text-2xs text-subtle-foreground">Esc para soltar</span>
		</div>
	{/if}
</div>
