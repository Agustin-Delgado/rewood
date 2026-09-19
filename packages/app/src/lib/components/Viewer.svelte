<script lang="ts">
	import { Canvas } from '@threlte/core';
	import Scene from './Scene.svelte';
	import { app } from '$lib/state.svelte';
</script>

<div class="viewer">
	<Canvas>
		<Scene />
	</Canvas>
	<div class="overlay">
		<label><input type="checkbox" bind:checked={app.showHoles} /> perforaciones</label>
		<label title="vista explotada">explotar <input type="range" min="0" max="2" step="0.05" bind:value={app.explode} /></label>
		{#if app.selected}
			<span class="sel">{app.selected.id} · {app.selected.name}</span>
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
</style>
