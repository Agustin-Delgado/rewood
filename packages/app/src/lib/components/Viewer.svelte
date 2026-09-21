<script lang="ts">
	import { Canvas } from '@threlte/core';
	import Scene from './Scene.svelte';
	import { app, VIEWS } from '$lib/state.svelte';
</script>

<div class="viewer">
	<Canvas>
		<Scene />
	</Canvas>
	<div class="overlay">
		<label><input type="checkbox" bind:checked={app.showHoles} /> perforaciones</label>
		<label><input type="checkbox" bind:checked={app.showHardware} /> herrajes</label>
		<label title="vista explotada">explotar <input type="range" min="0" max="2" step="0.05" bind:value={app.explode} /></label>
		<span class="views">
			{#each VIEWS as v (v.id)}
				<button class:on={app.view === v.id} title={v.title} onclick={() => app.setView(v.id)}>{v.name}</button>
			{/each}
		</span>
		{#if app.selected}
			<span class="sel">{app.selected.id} · {app.selected.name}</span>
		{:else if app.fastener}
			{@const f = app.fastener}
			<span class="sel">{app.hardwareName(f.fastener.hardware)} · {f.joint.id}</span>
		{/if}
	</div>
</div>

<style>
	.viewer {
		position: relative;
		width: 100%;
		height: 100%;
		background: #f3f4f6;
	}
	.overlay {
		position: absolute;
		left: 10px;
		top: 8px;
		display: flex;
		gap: 12px;
		align-items: center;
		font-size: 12px;
		background: rgba(255, 255, 255, 0.85);
		padding: 4px 8px;
		border-radius: 4px;
	}
	.sel {
		font-weight: 600;
	}
	.views {
		display: inline-flex;
		gap: 2px;
		border-left: 1px solid #d1d5db;
		padding-left: 10px;
	}
	.views button {
		font: inherit;
		font-size: 11px;
		padding: 1px 6px;
		border: 1px solid #d1d5db;
		border-radius: 3px;
		background: #fff;
		cursor: pointer;
	}
	.views button.on {
		background: #1f2937;
		color: #fff;
		border-color: #1f2937;
	}
</style>
