<script lang="ts">
	/**
	 * The catalogue: categories on the left, their variants as cards. Each
	 * card's drawing is the front elevation of the plan the engine compiles
	 * for that variant, so what the card shows is what loads.
	 */
	import { app } from '$lib/state.svelte';
	import { CATALOG, elevation, findVariant, overall, variantSpec, type Elevation } from '$lib/catalog';
	import { Badge, Dialog, recipes } from '$lib/ui';

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
</script>

<Dialog
	bind:open={app.catalogOpen}
	title="Catálogo"
	description="Elegí un mueble y después ajustalo en Diseño: medidas, cajones, lados, módulos."
	size="xl"
	class="h-[calc(100dvh-6rem)]"
	bodyClass="grid grid-cols-[232px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] overflow-hidden"
>
	<nav class="flex flex-col gap-0.5 overflow-y-auto border-r bg-depth-1 p-2" aria-label="Categorías">
		{#each CATALOG as c (c.id)}
			<button
				class="grid w-full grid-cols-[1fr_auto] gap-x-2 gap-y-0.5 rounded-md px-2.5 py-2 text-left transition-colors
					{c.id === category ? 'bg-primary-soft text-primary-soft-foreground' : 'hover:bg-depth-3/70'}"
				aria-current={c.id === category || undefined}
				onclick={() => (category = c.id)}
			>
				<span class="text-sm font-medium">{c.name}</span>
				<span class="num text-2xs {c.id === category ? 'text-primary-soft-foreground/70' : 'text-subtle-foreground'}">{c.variants.length}</span>
				<span class="col-span-2 text-xs leading-snug {c.id === category ? 'text-primary-soft-foreground/80' : 'text-muted-foreground'}">{c.description}</span>
			</button>
		{/each}
	</nav>
	<section class="overflow-y-auto p-4">
		<div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
			{#each shown.variants as v (v.id)}
				{@const th = thumbs[v.id]}
				{@const isCurrent = app.variant === v.id}
				<button
					class={recipes.card({ interactive: true, selected: isCurrent, class: 'card flex flex-col gap-1.5 p-2.5' })}
					onclick={() => app.loadVariant(v.id)}
				>
					<div class="grid h-36 place-items-center rounded-md bg-depth-2">
						{#if th?.drawing}
							{@const d = th.drawing}
							{@const pad = Math.max(d.width, d.height) * 0.06}
							<svg class="size-full" viewBox="{-pad} {-pad} {d.width + 2 * pad} {d.height + 2 * pad}" preserveAspectRatio="xMidYMid meet">
								{#each d.rects as r, i (i)}
									<rect
										x={r.x}
										y={r.y}
										width={r.w}
										height={r.h}
										class="stroke-muted-foreground {r.front ? 'fill-primary/25' : 'fill-primary-soft'}"
										stroke-width="1"
										vector-effect="non-scaling-stroke"
									/>
								{/each}
							</svg>
						{:else}
							<span class="text-subtle-foreground">…</span>
						{/if}
					</div>
					<div class="flex items-center gap-1.5 pt-0.5">
						<span class="text-sm font-medium">{v.name}</span>
						{#if isCurrent}<Badge tone="primary">en pantalla</Badge>{/if}
					</div>
					<p class="text-xs leading-snug text-muted-foreground">{v.description}</p>
					{#if th?.size}
						<div class="mt-auto flex items-center gap-2 pt-1">
							<span class="num font-mono text-2xs text-muted-foreground">{th.size[0]} × {th.size[1]} × {th.size[2]} mm</span>
							{#if th.errors}<Badge tone="danger">{th.errors} errores</Badge>{:else if th.warnings}<Badge tone="warning">{th.warnings} avisos</Badge>{/if}
						</div>
					{/if}
				</button>
			{/each}
		</div>
	</section>
</Dialog>
