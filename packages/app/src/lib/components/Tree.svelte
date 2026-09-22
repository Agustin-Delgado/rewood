<script lang="ts">
	/**
	 * The parts, by component, in words: "Cajones · módulo 2" and not
	 * `drawers_m2`, a drawer's six pieces under "Cajón 1", sizes rounded to
	 * what a tape measure reads. Groups start folded; the part picked in
	 * the viewer unfolds its own.
	 */
	import type { Part } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';
	import { componentLabels, partName, size } from '$lib/labels';

	type Sub = { key: string; label: string; parts: Part[] };
	type Group = { component: string; label: string; count: number; subs: Sub[]; loose: Part[] };

	const labels = $derived(componentLabels(app.spec));
	const groups = $derived.by(() => {
		const map = new Map<string, Group>();
		for (const p of app.plan?.parts ?? []) {
			let g = map.get(p.component);
			if (!g) {
				g = { component: p.component, label: labels.get(p.component) ?? p.component, count: 0, subs: [], loose: [] };
				map.set(p.component, g);
			}
			g.count += 1;
			// One drawer = one line that unfolds to its pieces.
			const m = p.role.match(/^(?:bay(\d+)_)?(?:.*_)?drawer_(\d+)_/);
			if (m) {
				const key = `${p.component}:${m[1] ?? ''}:${m[2]}`;
				let sub = g.subs.find((s) => s.key === key);
				if (!sub) {
					sub = { key, label: `Cajón ${m[2]}${m[1] ? ` (hueco ${m[1]})` : ''}`, parts: [] };
					g.subs.push(sub);
				}
				sub.parts.push(p);
			} else g.loose.push(p);
		}
		return [...map.values()];
	});

	let open = $state<Set<string>>(new Set());
	function toggle(key: string) {
		const next = new Set(open);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		open = next;
	}
	// The part selected in the viewer shows where it is.
	$effect(() => {
		const id = app.selectedPart;
		if (!id) return;
		const g = groups.find((x) => x.loose.some((p) => p.id === id) || x.subs.some((s) => s.parts.some((p) => p.id === id)));
		if (!g) return;
		const next = new Set(open);
		next.add(g.component);
		for (const s of g.subs) if (s.parts.some((p) => p.id === id)) next.add(s.key);
		if (next.size !== open.size) open = next;
	});
</script>

<div class="tree">
	<h3>Piezas <span class="count">{app.plan?.parts.length ?? 0}</span></h3>
	{#each groups as g (g.component)}
		<div class="group">
			<div class="group-head">
				<button class="eye" title="mostrar / ocultar en el 3D" onclick={() => app.toggleComponent(g.component)}>
					{app.hiddenComponents.has(g.component) ? '○' : '●'}
				</button>
				<button class="fold" title={g.component} onclick={() => toggle(g.component)}>
					<span class="caret">{open.has(g.component) ? '▾' : '▸'}</span>
					<span class="name">{g.label}</span>
					<span class="count">{g.count}</span>
				</button>
			</div>
			{#if open.has(g.component)}
				{#each g.loose as part (part.id)}
					{@render row(part)}
				{/each}
				{#each g.subs as s (s.key)}
					<button class="fold sub" onclick={() => toggle(s.key)}>
						<span class="caret">{open.has(s.key) ? '▾' : '▸'}</span>
						<span>{s.label}</span>
						<span class="count">{s.parts.length} piezas</span>
					</button>
					{#if open.has(s.key)}
						{#each s.parts as part (part.id)}
							{@render row(part, true)}
						{/each}
					{/if}
				{/each}
			{/if}
		</div>
	{/each}
</div>

{#snippet row(part: Part, nested = false)}
	<button class="part" class:nested class:selected={app.selectedPart === part.id} title="{part.id} · {part.material}" onclick={() => app.selectPart(part.id)}>
		<span>{partName(part.name)}</span>
		<span class="dims">{size(part.dims)}</span>
	</button>
{/snippet}

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
		gap: 2px;
		align-items: center;
		margin-top: 4px;
	}
	.eye {
		border: none;
		background: none;
		cursor: pointer;
		font-size: 11px;
		color: #555;
		padding: 0 2px;
	}
	button {
		font: inherit;
	}
	.fold {
		display: flex;
		gap: 6px;
		align-items: baseline;
		flex: 1;
		border: none;
		background: none;
		padding: 2px 4px;
		cursor: pointer;
		text-align: left;
		border-radius: 3px;
		font-weight: 600;
	}
	.fold.sub {
		width: 100%;
		padding-left: 24px;
		font-weight: 500;
	}
	.fold:hover {
		background: #eef2f7;
	}
	.caret {
		color: #999;
		width: 10px;
	}
	.count {
		color: #888;
		font-weight: 400;
		margin-left: auto;
	}
	.part {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		width: 100%;
		text-align: left;
		border: none;
		background: none;
		padding: 2px 4px 2px 32px;
		cursor: pointer;
		border-radius: 3px;
	}
	.part.nested {
		padding-left: 46px;
	}
	.part:hover {
		background: #eef2f7;
	}
	.part.selected {
		background: #ffe0c7;
	}
	.dims {
		color: #888;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
