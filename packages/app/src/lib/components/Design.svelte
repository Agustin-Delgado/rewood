<script lang="ts">
	/**
	 * The template's options as controls: numbers with a slider, switches,
	 * choices as buttons. Bounds, what applies right now and what is out of
	 * range all come from the plan (the engine resolved them); a change
	 * writes the parameter back and recompiles.
	 */
	import type { Diagnostic, PlanOption } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';
	import { findVariant, overall } from '$lib/catalog';
	import { Badge, Button, EmptyState, NumberField, Section, Segmented, Slider, Swatches, Switch, recipes } from '$lib/ui';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import Wrench from '@lucide/svelte/icons/wrench';

	const options = $derived((app.plan?.options ?? []).filter((o) => o.active));
	const groups = $derived.by(() => {
		const map = new Map<string, PlanOption[]>();
		for (const o of options) {
			const g = o.group ?? 'Opciones';
			if (!map.has(g)) map.set(g, []);
			map.get(g)!.push(o);
		}
		return [...map.entries()];
	});
	const current = $derived(app.variant ? findVariant(app.variant) : null);
	const size = $derived(app.plan ? overall(app.plan) : null);
	const items = $derived(app.plan?.diagnostics.items ?? []);
	const errors = $derived(items.filter((d) => d.severity === 'ERROR' || d.severity === 'FATAL'));
	/** What stands between the piece and the workshop (an out-of-range value shows on its option). */
	const blocking = $derived(errors.filter((d) => d.code !== 'SPEC-503'));
	const warnings = $derived(items.filter((d) => d.severity === 'WARNING'));
	const hardwareCount = $derived(app.plan?.joints.reduce((n, j) => n + j.fasteners.length, 0) ?? 0);

	/** The finding an option's value triggered, if any. */
	const outOfRange = (o: PlanOption) => items.find((d) => d.code === 'SPEC-503' && d.entity === o.param);

	// A slider fires on every pixel: the value on screen follows at once,
	// the recompile at most once a frame.
	let drafts: Record<string, number> = $state({});
	let pending: { param: string; value: number } | null = null;
	let frame = 0;
	function slide(param: string, value: number) {
		drafts[param] = value;
		pending = { param, value };
		if (!frame)
			frame = requestAnimationFrame(() => {
				frame = 0;
				if (pending) app.setParameter(pending.param, pending.value);
				pending = null;
			});
	}
	function set(o: PlanOption, value: number | boolean) {
		delete drafts[o.param];
		app.setParameter(o.param, value);
	}
	function num(o: PlanOption): number {
		return drafts[o.param] ?? (o.value as number);
	}
	function stepOf(o: PlanOption) {
		return o.step ?? 1;
	}
	function nudge(o: PlanOption, d: number) {
		let v = num(o) + d * stepOf(o);
		if (o.min !== undefined) v = Math.max(o.min, v);
		if (o.max !== undefined) v = Math.min(o.max, v);
		set(o, Math.round(v * 1000) / 1000);
	}
	/** A typed value; an emptied field keeps the current one. */
	function typed(o: PlanOption, v: number | null) {
		if (v === null || !Number.isFinite(v)) return;
		if (v !== num(o)) set(o, v);
	}
	/** The discrete values of a short range, drawn as a row of choices. */
	function ticks(o: PlanOption): number[] {
		return Array.from({ length: Math.round((o.max! - o.min!) / stepOf(o)) + 1 }, (_, i) => o.min! + i * stepOf(o));
	}
	const tick = (o: PlanOption) => ticks(o).find((v) => Math.abs(num(o) - v) < 1e-6);
	// Colour: the decors the body's board is sold in. Fronts follow the
	// body unless the person gives them their own.
	const decors = $derived(
		[...app.decorsFor(app.spec.material)].sort((a, b) => Number(!!a.grain) - Number(!!b.grain) || lightness(b.hex) - lightness(a.hex))
	);
	/** Plain colours first, then wood; each from light to dark. */
	function lightness(hex: string) {
		const n = parseInt(hex.slice(1), 16);
		return 0.2126 * (n >> 16) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255);
	}
	const decorOptions = $derived(
		decors.map((d) => ({ value: d.id, label: d.name, hex: d.hex, grain: d.grain, hint: `${d.brand} ${d.code ?? ''}`.trim() }))
	);
	const bodyDecor = $derived(app.spec.decor ?? app.libraries?.materials.materials[app.spec.material]?.defaultDecor ?? null);
	const nameOf = (id: string | null | undefined) => decors.find((d) => d.id === id)?.name ?? '';

	function reset() {
		if (app.variant) app.loadVariant(app.variant);
	}
</script>

<div class="h-full overflow-y-auto">
	<div class="p-3">
		<div class={recipes.card({ class: 'grid gap-2 p-3' })}>
			<div class="min-w-0">
				{#if current}
					<p class="text-2xs font-semibold tracking-[0.06em] text-subtle-foreground uppercase">{current.category.name}</p>
					<p class="truncate text-base font-semibold">{current.variant.name}</p>
				{:else}
					<p class="truncate text-base font-semibold">{app.spec.name}</p>
				{/if}
			</div>
			{#if size}
				<p class="num font-mono text-xs text-foreground">
					{size[0]} <span class="font-sans text-2xs text-subtle-foreground">ancho</span> × {size[1]}
					<span class="font-sans text-2xs text-subtle-foreground">alto</span> × {size[2]}
					<span class="font-sans text-2xs text-subtle-foreground">prof.</span> mm
				</p>
			{/if}
			<div class="flex flex-wrap gap-1">
				<Badge>{app.plan?.parts.length ?? 0} piezas</Badge>
				<Badge>{hardwareCount} herrajes</Badge>
				{#if errors.length}
					<Badge tone="danger">{errors.length} {errors.length === 1 ? 'error' : 'errores'}</Badge>
				{:else if warnings.length}
					<Badge tone="warning">{warnings.length} {warnings.length === 1 ? 'aviso' : 'avisos'}</Badge>
				{:else}
					<Badge tone="success">fabricable</Badge>
				{/if}
			</div>
			<div class="flex gap-1.5 pt-0.5">
				<Button size="xs" variant="soft" onclick={() => (app.catalogOpen = true)}><LayoutGrid />Cambiar mueble…</Button>
				{#if app.variant}
					<Button size="xs" variant="ghost" onclick={reset} title="Volver a los valores de esta variante"><RotateCcw />Restablecer</Button>
				{/if}
			</div>
		</div>
	</div>

	{#if decors.length}
		<div class="border-t">
			<Section title="Color" bodyClass="grid gap-3 px-3 pb-3">
				<div class="grid gap-1.5">
					<div class="flex items-baseline justify-between gap-2">
						<span class="text-sm font-medium">Cuerpo</span>
						<span class="text-xs text-muted-foreground">{nameOf(bodyDecor)}</span>
					</div>
					<Swatches aria-label="Color del cuerpo" options={decorOptions} value={bodyDecor} onChange={(id) => app.setDecor('decor', id)} />
				</div>
				<div class="grid gap-1.5">
					<div class="flex min-h-7 items-center justify-between gap-2">
						<span class="text-sm font-medium">Puertas y frentes de otro color</span>
						<Switch
							aria-label="Puertas y frentes de otro color"
							checked={!!app.spec.frontDecor}
							onChange={(on) => app.setDecor('frontDecor', on ? (bodyDecor === 'nogal_terracota' ? 'blanco_nature' : 'nogal_terracota') : null)}
						/>
					</div>
					{#if app.spec.frontDecor}
						<div class="flex items-baseline justify-end">
							<span class="text-xs text-muted-foreground">{nameOf(app.spec.frontDecor)}</span>
						</div>
						<Swatches
							aria-label="Color de puertas y frentes"
							options={decorOptions}
							value={app.spec.frontDecor}
							onChange={(id) => app.setDecor('frontDecor', id)}
						/>
					{/if}
				</div>
				<p class="text-xs leading-snug text-subtle-foreground">
					Melaminas Faplac de 18 mm; el canto va en el mismo color. Las de madera llevan veta y se cortan a lo largo.
				</p>
			</Section>
		</div>
	{/if}

	{#if options.length === 0}
		<EmptyState
			title="Sin opciones"
			description="Este mueble no declara opciones. Sus medidas están en Parámetros y sus partes en Componentes."
		/>
	{/if}

	<div class="border-t">
		{#each groups as [group, list] (group)}
			<Section title={group} count={list.length} bodyClass="px-3 pb-2">
				{#each list as o (o.param)}
					{@const bad = outOfRange(o)}
					<div class="border-b border-border/60 py-2.5 first:pt-0.5 last:border-b-0">
						<div class="flex min-h-7 items-center justify-between gap-3">
							<span class="min-w-0 truncate text-sm font-medium">{o.label}</span>
							{#if o.kind === 'number'}
								<div class="flex shrink-0 items-center gap-0.5">
									<Button
										variant="ghost"
										size="icon-xs"
										onclick={() => nudge(o, -1)}
										disabled={o.min !== undefined && num(o) <= o.min}
										aria-label="menos"><Minus /></Button
									>
									<NumberField
										class="w-24"
										value={num(o)}
										onChange={(v) => typed(o, v)}
										step={stepOf(o)}
										decimals={3}
										unit={o.unit}
										invalid={!!bad}
										aria-label={o.label}
									/>
									<Button
										variant="ghost"
										size="icon-xs"
										onclick={() => nudge(o, 1)}
										disabled={o.max !== undefined && num(o) >= o.max}
										aria-label="más"><Plus /></Button
									>
								</div>
							{:else if o.kind === 'toggle'}
								<Switch checked={o.value === true} aria-label={o.label} onChange={(v) => set(o, v)} />
							{/if}
						</div>
						{#if o.kind === 'number' && o.min !== undefined && o.max !== undefined && o.max > o.min}
							{#if (o.max - o.min) / stepOf(o) <= 8}
								<Segmented
									class="mt-2"
									fill
									aria-label={o.label}
									value={tick(o) === undefined ? null : String(tick(o))}
									options={ticks(o).map((v) => ({ value: String(v), label: String(v) }))}
									onChange={(v) => set(o, Number(v))}
								/>
							{:else}
								<Slider
									class="mt-2"
									aria-label={o.label}
									min={o.min}
									max={o.max}
									step={stepOf(o)}
									value={num(o)}
									onInput={(v) => slide(o.param, v)}
									onChange={(v) => set(o, v)}
								/>
								<div class="num flex justify-between text-2xs text-subtle-foreground">
									<span>{o.min}</span><span>{o.max}</span>
								</div>
							{/if}
						{:else if o.kind === 'choice'}
							<Segmented
								class="mt-2"
								fill
								aria-label={o.label}
								value={String(o.value)}
								options={(o.choices ?? []).map((c) => ({ value: String(c.value), label: c.label }))}
								onChange={(v) => set(o, Number(v))}
							/>
						{/if}
						{#if o.help}<p class="mt-1.5 text-xs leading-snug text-muted-foreground">{o.help}</p>{/if}
						{#if bad}{@render finding(bad)}{/if}
					</div>
				{/each}
			</Section>
		{/each}

		{#if blocking.length}
			<Section title="Para poder fabricarlo" count={blocking.length} static bodyClass="grid gap-1.5 px-3 pb-3">
				{#each blocking.slice(0, 4) as d (d.code + (d.entity ?? '') + d.message)}
					{@render finding(d)}
				{/each}
			</Section>
		{/if}
	</div>
</div>

{#snippet finding(d: Diagnostic)}
	<div class="mt-1.5 flex items-start gap-2 rounded-md bg-danger-soft px-2 py-1.5 text-xs text-danger">
		<TriangleAlert class="mt-px size-3.5 shrink-0" />
		<p class="min-w-0 flex-1 leading-snug">{d.message}</p>
		{#if d.fix}
			<Button size="xs" class="-my-0.5 shrink-0" onclick={() => app.applyFix(d.fix!)}><Wrench />{d.fix.label}</Button>
		{/if}
	</div>
{/snippet}
