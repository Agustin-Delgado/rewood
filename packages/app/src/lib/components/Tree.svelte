<script lang="ts">
	/** Model tree: components, their parts, and a joint list. */
	import { app } from '$lib/state.svelte';

	const groups = $derived.by(() => {
		const map = new Map<string, typeof app.plan extends null ? never : NonNullable<typeof app.plan>['parts']>();
		for (const p of app.plan?.parts ?? []) {
			if (!map.has(p.component)) map.set(p.component, []);
			map.get(p.component)!.push(p);
		}
		return [...map.entries()];
	});
</script>

<div class="tree">
	<h3>Modelo</h3>
	{#each groups as [component, parts] (component)}
		<div class="group">
			<div class="group-head">
				<button class="eye" title="mostrar / ocultar" onclick={() => app.toggleComponent(component)}>
					{app.hiddenComponents.has(component) ? '○' : '●'}
				</button>
				<span class="name">{component}</span>
				<span class="count">{parts.length}</span>
			</div>
			{#each parts as part (part.id)}
				<button class="part" class:selected={app.selectedPart === part.id} onclick={() => app.selectPart(part.id)}>
					<span class="id">{part.id}</span>
					<span>{part.name}</span>
					<span class="dims">{part.dims.length}×{part.dims.width}×{part.dims.thickness}</span>
				</button>
			{/each}
		</div>
	{/each}
	{#if app.plan}
		<h3>Uniones <span class="count">{app.plan.joints.length}</span></h3>
		<div class="joints">
			{#each app.plan.joints as j (j.id)}
				<div class="joint">
					<span class="id">{j.id}</span>
					<span class="kind">{j.kind}</span>
					<span>{j.edgePart} → {j.facePart}</span>
					<span class="dims">{j.fasteners.length} fij.</span>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.tree {
		font-size: 12px;
		overflow: auto;
		height: 100%;
		padding: 8px;
	}
	h3 {
		font-size: 12px;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: #666;
		margin: 8px 0 4px;
	}
	.group-head {
		display: flex;
		gap: 6px;
		align-items: center;
		font-weight: 600;
		margin-top: 6px;
	}
	.eye {
		border: none;
		background: none;
		cursor: pointer;
		font-size: 11px;
		color: #555;
		padding: 0 2px;
	}
	.count {
		color: #888;
		font-weight: 400;
	}
	.part,
	.joint {
		display: grid;
		grid-template-columns: 42px 1fr auto;
		gap: 6px;
		width: 100%;
		text-align: left;
		border: none;
		background: none;
		padding: 2px 4px 2px 22px;
		cursor: pointer;
		font: inherit;
		border-radius: 3px;
	}
	.joint {
		grid-template-columns: 38px 60px 1fr auto;
		padding-left: 4px;
		cursor: default;
	}
	.part:hover {
		background: #eef2f7;
	}
	.part.selected {
		background: #ffe0c7;
	}
	.id {
		color: #345;
		font-family: ui-monospace, monospace;
	}
	.kind {
		color: #777;
	}
	.dims {
		color: #888;
		font-variant-numeric: tabular-nums;
	}
</style>
