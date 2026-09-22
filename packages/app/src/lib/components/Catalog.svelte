<script lang="ts">
	/**
	 * The catalogue: categories on the left, their variants as cards. Each
	 * card's drawing is the front elevation of the plan the engine compiles
	 * for that variant, so what the card shows is what loads.
	 */
	import { app } from '$lib/state.svelte';
	import { CATALOG, elevation, findVariant, overall, variantSpec, type Elevation } from '$lib/catalog';

	const current = $derived(app.variant ? findVariant(app.variant) : null);
	let category = $state(CATALOG[0].id);
	$effect(() => {
		if (app.catalogOpen && current) category = current.category.id;
	});
	const shown = $derived(CATALOG.find((c) => c.id === category) ?? CATALOG[0]);

	type Thumb = { drawing: Elevation | null; size: [number, number, number] | null; warnings: number; errors: number };
	let thumbs: Record<string, Thumb> = $state({});
	// Compiled the first time a category is looked at; the engine is
	// deterministic, so a cached drawing never goes stale.
	$effect(() => {
		if (!app.catalogOpen || !app.engine) return;
		for (const v of shown.variants) {
			if (thumbs[v.id]) continue;
			const plan = app.engine.compile(variantSpec(v));
			const items = plan.diagnostics.items;
			thumbs[v.id] = {
				drawing: elevation(plan),
				size: overall(plan),
				warnings: items.filter((d) => d.severity === 'WARNING').length,
				errors: items.filter((d) => d.severity === 'ERROR' || d.severity === 'FATAL').length
			};
		}
	});

	function close() {
		app.catalogOpen = false;
	}
</script>

<svelte:window onkeydown={(e) => app.catalogOpen && e.key === 'Escape' && close()} />

{#if app.catalogOpen}
	<div class="backdrop" role="presentation" onclick={close}></div>
	<div class="catalog" role="dialog" aria-modal="true" aria-label="Catálogo">
		<header>
			<h2>Catálogo</h2>
			<p>Elegí un mueble y después ajustalo en <b>Diseño</b>: medidas, cajones, lados, módulos.</p>
			<button class="close" onclick={close} aria-label="cerrar">✕</button>
		</header>
		<nav>
			{#each CATALOG as c (c.id)}
				<button class:on={c.id === category} onclick={() => (category = c.id)}>
					<span class="cname">{c.name}</span>
					<span class="count">{c.variants.length}</span>
					<span class="cdesc">{c.description}</span>
				</button>
			{/each}
		</nav>
		<section>
			<div class="grid">
				{#each shown.variants as v (v.id)}
					{@const th = thumbs[v.id]}
					<button class="card" class:current={app.variant === v.id} onclick={() => app.loadVariant(v.id)}>
						<div class="thumb">
							{#if th?.drawing}
								{@const d = th.drawing}
								{@const pad = Math.max(d.width, d.height) * 0.06}
								<svg viewBox="{-pad} {-pad} {d.width + 2 * pad} {d.height + 2 * pad}" preserveAspectRatio="xMidYMid meet">
									{#each d.rects as r, i (i)}
										<rect x={r.x} y={r.y} width={r.w} height={r.h} class:front={r.front} vector-effect="non-scaling-stroke" />
									{/each}
								</svg>
							{:else}
								<span class="loading">…</span>
							{/if}
						</div>
						<div class="meta">
							<span class="vname">{v.name}</span>
							{#if app.variant === v.id}<span class="tag">en pantalla</span>{/if}
						</div>
						<p class="vdesc">{v.description}</p>
						{#if th?.size}
							<div class="size">
								{th.size[0]} × {th.size[1]} × {th.size[2]} mm
								{#if th.errors}<span class="bad">{th.errors} errores</span>{:else if th.warnings}<span class="warn">{th.warnings} avisos</span>{/if}
							</div>
						{/if}
					</button>
				{/each}
			</div>
		</section>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(17, 24, 39, 0.35);
		z-index: 20;
	}
	.catalog {
		position: fixed;
		inset: 48px 48px;
		z-index: 21;
		background: #fff;
		border-radius: 10px;
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.25);
		display: grid;
		grid-template-columns: 240px 1fr;
		grid-template-rows: auto 1fr;
		overflow: hidden;
		font-size: 13px;
	}
	header {
		grid-column: 1 / -1;
		display: flex;
		align-items: baseline;
		gap: 16px;
		padding: 14px 20px;
		border-bottom: 1px solid #e5e7eb;
	}
	h2 {
		margin: 0;
		font-size: 18px;
	}
	header p {
		margin: 0;
		color: #6b7280;
		flex: 1;
	}
	.close {
		border: none;
		background: none;
		font-size: 16px;
		cursor: pointer;
		color: #6b7280;
	}
	nav {
		border-right: 1px solid #e5e7eb;
		overflow: auto;
		padding: 8px;
		background: #fafafa;
	}
	nav button {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 2px 8px;
		width: 100%;
		text-align: left;
		border: none;
		background: none;
		padding: 8px 10px;
		border-radius: 6px;
		cursor: pointer;
		font: inherit;
	}
	nav button:hover {
		background: #f3f4f6;
	}
	nav button.on {
		background: #fff1e6;
		box-shadow: inset 3px 0 0 #ff8c42;
	}
	.cname {
		font-weight: 600;
	}
	.count {
		color: #9ca3af;
		font-size: 12px;
	}
	.cdesc {
		grid-column: 1 / -1;
		color: #6b7280;
		font-size: 12px;
	}
	section {
		overflow: auto;
		padding: 16px 20px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 14px;
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		text-align: left;
		font: inherit;
		border: 1px solid #e5e7eb;
		border-radius: 8px;
		background: #fff;
		padding: 10px;
		cursor: pointer;
		transition:
			border-color 0.1s,
			box-shadow 0.1s;
	}
	.card:hover {
		border-color: #ff8c42;
		box-shadow: 0 4px 14px rgba(255, 140, 66, 0.15);
	}
	.card.current {
		border-color: #ff8c42;
	}
	.thumb {
		height: 150px;
		background: #f8f7f4;
		border-radius: 6px;
		display: grid;
		place-items: center;
	}
	.thumb svg {
		width: 100%;
		height: 100%;
	}
	rect {
		fill: #efe6d6;
		stroke: #8a7a66;
		stroke-width: 1;
	}
	rect.front {
		fill: #e2cfae;
	}
	.loading {
		color: #9ca3af;
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.vname {
		font-weight: 600;
	}
	.tag {
		font-size: 11px;
		color: #b45309;
		background: #fff1e6;
		border-radius: 8px;
		padding: 0 6px;
	}
	.vdesc {
		margin: 0;
		color: #4b5563;
		font-size: 12px;
		line-height: 1.35;
	}
	.size {
		color: #6b7280;
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		display: flex;
		gap: 8px;
	}
	.warn {
		color: #92400e;
	}
	.bad {
		color: #b91c1c;
	}
</style>
