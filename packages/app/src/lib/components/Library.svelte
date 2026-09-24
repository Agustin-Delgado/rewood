<script lang="ts">
	/**
	 * The workshop's library: what the engine takes as standard (sizes sold
	 * in Argentina) and what this workshop changed. Every change is a
	 * `libraries` override, applied to every piece and carried inside the
	 * spec that is compiled, packaged or saved.
	 */
	import type { EdgeMaterial, HardwareDef, LibraryOverrides, Material } from '@rewood/engine/browser';
	import type { Snippet } from 'svelte';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Copy from '@lucide/svelte/icons/copy';
	import Download from '@lucide/svelte/icons/download';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Search from '@lucide/svelte/icons/search';
	import Upload from '@lucide/svelte/icons/upload';
	import { app } from '../state.svelte';
	import { duplicate, hasOverride, isEmpty, resetEntry, setValue } from '../workshop';
	import { Badge, Button, EmptyState, Input, NumberField, Section, Segmented } from '$lib/ui';

	type ListKey = 'materials' | 'edgeMaterials' | 'hardware';
	/** `always`: shown even when neither the item nor the standard has it (a price to fill in). */
	type Field = { path: (string | number)[]; label: string; unit?: string; always?: boolean };

	const KINDS: Record<string, { label: string; fields: Field[] }> = {
		hinge: {
			label: 'Bisagras',
			fields: [
				{ path: ['hinge', 'opening'], label: 'apertura', unit: '°' },
				{ path: ['maxLoadKg'], label: 'carga por bisagra', unit: 'kg' },
				{ path: ['hinge', 'maxDoor', 0], label: 'puerta hasta (ancho)', unit: 'mm' },
				{ path: ['hinge', 'maxDoor', 1], label: 'puerta hasta (alto)', unit: 'mm' }
			]
		},
		slide: {
			label: 'Correderas',
			fields: [
				{ path: ['slide', 'length'], label: 'largo', unit: 'mm' },
				{ path: ['slide', 'sideClearance'], label: 'luz por lado', unit: 'mm' },
				{ path: ['slide', 'axisFromBoxBottom'], label: 'eje desde el fondo de la caja', unit: 'mm' },
				{ path: ['maxLoadKg'], label: 'carga por par', unit: 'kg' }
			]
		},
		handle: { label: 'Manijas y tiradores', fields: [] },
		leg: {
			label: 'Patas y ruedas',
			fields: [
				{ path: ['leg', 'height'], label: 'alto', unit: 'mm' },
				{ path: ['leg', 'baseDiameter'], label: 'base', unit: 'mm' },
				{ path: ['maxLoadKg'], label: 'carga por pata', unit: 'kg' }
			]
		},
		sliding_track: {
			label: 'Kits corredizos',
			fields: [
				{ path: ['sliding', 'maxWidth'], label: 'abertura hasta', unit: 'mm' },
				{ path: ['sliding', 'widthDeduction'], label: 'puerta: mitad de la abertura menos', unit: 'mm' },
				{ path: ['sliding', 'bottomClearance'], label: 'luz abajo', unit: 'mm' },
				{ path: ['sliding', 'topClearance'], label: 'luz arriba', unit: 'mm' },
				{ path: ['sliding', 'lanePitch'], label: 'paso entre carriles', unit: 'mm' },
				{ path: ['sliding', 'depth'], label: 'profundidad del riel', unit: 'mm' },
				{ path: ['sliding', 'frontInset'], label: 'riel desde el frente', unit: 'mm' }
			]
		},
		sliding_roller: { label: 'Ruedas de corrediza', fields: [{ path: ['maxLoadKg'], label: 'carga por rueda', unit: 'kg' }] },
		sink: {
			label: 'Bachas',
			fields: [
				{ path: ['sink', 'below'], label: 'cuelga bajo la tapa', unit: 'mm' },
				{ path: ['cutouts', 0, 'width'], label: 'recorte: ancho', unit: 'mm' },
				{ path: ['cutouts', 0, 'height'], label: 'recorte: fondo', unit: 'mm' },
				{ path: ['cutouts', 0, 'radius'], label: 'recorte: radio', unit: 'mm' }
			]
		},
		passage: {
			label: 'Pases de caños',
			fields: [
				{ path: ['cutouts', 0, 'width'], label: 'ancho', unit: 'mm' },
				{ path: ['cutouts', 0, 'height'], label: 'alto', unit: 'mm' },
				{ path: ['cutouts', 0, 'radius'], label: 'radio', unit: 'mm' }
			]
		},
		spacer: { label: 'Suplementos de corredera', fields: [{ path: ['spacer', 'thickness'], label: 'espesor', unit: 'mm' }] },
		file_rails: {
			label: 'Rieles para carpetas',
			fields: [
				{ path: ['files', 'minInner'], label: 'caja por dentro desde', unit: 'mm' },
				{ path: ['files', 'maxInner'], label: 'caja por dentro hasta', unit: 'mm' },
				{ path: ['files', 'minHeight'], label: 'alto de caja', unit: 'mm' }
			]
		}
	};
	const OTHER = { label: 'Otros herrajes', fields: [] as Field[] };

	let section: 'boards' | 'edges' | 'hardware' = $state('boards');
	let filter = $state('');
	let open: string | null = $state(null);
	let message: string | null = $state(null);
	let fileInput: HTMLInputElement | null = $state(null);

	const libs = $derived(app.libraries);
	const defaults = $derived(app.defaults);
	const changes = $derived(
		(app.workshop.materials?.length ?? 0) + (app.workshop.edgeMaterials?.length ?? 0) + (app.workshop.hardware?.length ?? 0)
	);

	const match = (x: { id: string; name: string }) => {
		const f = filter.trim().toLowerCase();
		return !f || x.name.toLowerCase().includes(f) || x.id.toLowerCase().includes(f);
	};
	const boards = $derived(libs ? Object.values(libs.materials.materials).filter(match) : []);
	const edges = $derived(libs ? Object.values(libs.materials.edgeMaterials).filter(match) : []);
	const groups = $derived.by(() => {
		const out = new Map<string, HardwareDef[]>();
		for (const h of Object.values(libs?.hardware.items ?? {})) {
			if (!match(h) || h.kind === 'pin_row') continue;
			const k = h.kind in KINDS ? h.kind : 'other';
			out.set(k, [...(out.get(k) ?? []), h]);
		}
		const order = [...Object.keys(KINDS), 'other'];
		return [...out.entries()].sort((a, b) => order.indexOf(a[0]) - order.indexOf(b[0]));
	});

	function get(o: unknown, path: (string | number)[]): unknown {
		let v = o as Record<string | number, unknown> | undefined;
		for (const k of path) v = v?.[k] as Record<string | number, unknown> | undefined;
		return v;
	}

	/** Write a value: an array on the way (cutouts, maxDoor, bomItems) is written whole, the engine replaces arrays. */
	function write(list: ListKey, item: Record<string, unknown>, path: (string | number)[], value: unknown) {
		const i = path.findIndex((k) => typeof k === 'number');
		let next: LibraryOverrides;
		if (i < 0) next = setValue(app.workshop, list, item.id as string, path as string[], value);
		else {
			const head = path.slice(0, i) as string[];
			const arr = structuredClone((get(item, head) as unknown[] | undefined) ?? []);
			let node = arr as unknown as Record<string | number, unknown>;
			for (const k of path.slice(i, -1)) {
				node[k] ??= {};
				node = node[k] as Record<string | number, unknown>;
			}
			node[path[path.length - 1]] = value;
			next = setValue(app.workshop, list, item.id as string, head, arr);
		}
		app.setWorkshop(next);
	}
	/** A number committed by a field; an emptied field keeps the value it had. */
	function num(list: ListKey, item: Record<string, unknown>, path: (string | number)[], v: number | null) {
		if (v === null || !Number.isFinite(v)) return;
		write(list, item, path, v);
	}
	function text(list: ListKey, item: Record<string, unknown>, path: string[], raw: string) {
		if (raw.trim()) write(list, item, path, raw.trim());
	}
	function standard(list: ListKey, id: string, path: (string | number)[]): unknown {
		const base =
			list === 'materials' ? defaults?.materials.materials[id] : list === 'edgeMaterials' ? defaults?.materials.edgeMaterials[id] : defaults?.hardware.items[id];
		return base ? get(base, path) : undefined;
	}
	function isNew(list: ListKey, id: string) {
		return standard(list, id, ['id']) === undefined;
	}

	/** A bar handle's centre distance: its two holes, either side of the middle. */
	function centres(h: HardwareDef): number | null {
		const along = (h.holes ?? []).map((x) => x.offsetAlong ?? 0);
		return along.length === 2 ? Math.abs(along[1] - along[0]) : null;
	}
	function setCentres(h: HardwareDef, d: number | null) {
		if (d === null || !Number.isFinite(d) || d <= 0 || (h.holes ?? []).length !== 2) return;
		const holes = structuredClone(h.holes);
		holes[0].offsetAlong = -d / 2;
		holes[1].offsetAlong = d / 2;
		write('hardware', h as unknown as Record<string, unknown>, ['holes'], holes);
	}

	function copy(list: ListKey, item: { id: string; name: string }) {
		const taken = (id: string) =>
			list === 'hardware' ? !!libs?.hardware.items[id] : list === 'materials' ? !!libs?.materials.materials[id] : !!libs?.materials.edgeMaterials[id];
		let n = 2;
		while (taken(`${item.id}_${n}`)) n += 1;
		const id = `${item.id}_${n}`;
		app.setWorkshop(duplicate(app.workshop, list, item as unknown as { id: string }, id, `${item.name} (copia)`));
		open = id;
	}
	function reset(list: ListKey, id: string) {
		app.setWorkshop(resetEntry(app.workshop, list, id));
	}

	function exportFile() {
		const blob = new Blob([JSON.stringify({ rewoodWorkshop: 1, libraries: app.workshop }, null, 2)], { type: 'application/json' });
		const a = document.createElement('a');
		a.href = URL.createObjectURL(blob);
		a.download = 'biblioteca-taller.json';
		a.click();
		URL.revokeObjectURL(a.href);
	}
	async function importFile(input: HTMLInputElement) {
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		try {
			const v = JSON.parse(await file.text());
			const libraries = (v && typeof v === 'object' && 'libraries' in v ? v.libraries : v) as LibraryOverrides;
			if (!libraries || typeof libraries !== 'object' || Array.isArray(libraries)) throw new Error('no es una biblioteca');
			app.setWorkshop(libraries);
			message = 'Biblioteca importada.';
		} catch (e) {
			message = `No se pudo importar: ${(e as Error).message}`;
		}
	}
	function resetAll() {
		if (confirm('¿Volver toda la biblioteca a las medidas estándar? Se pierden los cambios del taller.')) app.setWorkshop({});
	}
	const fmt = (v: unknown) => (v === undefined || v === null ? '' : String(v));
	const asNum = (v: unknown) => (typeof v === 'number' ? v : null);

	const ROW = 'grid grid-cols-[minmax(0,1fr)_7.5rem] items-center gap-x-2 gap-y-0.5';
	const LBL = 'truncate text-xs text-muted-foreground';
</script>

{#snippet numberRow(list: ListKey, item: Record<string, unknown>, f: Field)}
	{@const value = get(item, f.path)}
	{@const std = standard(list, item.id as string, f.path)}
	{@const changed = std !== undefined && value !== std}
	{#if value !== undefined || std !== undefined || f.always}
		<div class={ROW}>
			<span class="{LBL} {changed ? 'font-medium text-primary-soft-foreground' : ''}">{f.label}</span>
			<NumberField size="xs" aria-label={f.label} unit={f.unit} value={asNum(value)} invalid={false}
				class={changed ? '[&_[data-number-field-group]]:border-primary/60' : ''}
				onChange={(v) => num(list, item, f.path, v)} />
			{#if changed}<span class="col-start-2 text-right text-2xs text-subtle-foreground" title="medida estándar">estándar {fmt(std)}</span>{/if}
		</div>
	{/if}
{/snippet}

{#snippet head(list: ListKey, item: { id: string; name: string })}
	<button
		class="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-depth-2 focus-visible:ring-2 focus-visible:ring-ring/60 {open === item.id ? 'bg-depth-2' : ''}"
		aria-expanded={open === item.id}
		onclick={() => (open = open === item.id ? null : item.id)}
	>
		<ChevronRight class="size-3.5 shrink-0 text-subtle-foreground transition-transform {open === item.id ? 'rotate-90' : ''}" />
		<span class="min-w-0 flex-1 truncate">{item.name}</span>
		{#if isNew(list, item.id)}<Badge tone="success">nuevo</Badge>{:else if hasOverride(app.workshop, list, item.id)}<Badge tone="primary">modificado</Badge>{/if}
	</button>
{/snippet}

{#snippet actions(list: ListKey, item: { id: string; name: string })}
	<div class="mt-1 flex items-center gap-1 border-t pt-2">
		<span class="min-w-0 flex-1 truncate font-mono text-2xs text-subtle-foreground">{item.id}</span>
		<Button size="xs" variant="ghost" onclick={() => copy(list, item)} title="Un ítem nuevo con estos datos, para cargar otra medida"><Copy />Duplicar</Button>
		{#if hasOverride(app.workshop, list, item.id)}
			<Button size="xs" variant={isNew(list, item.id) ? 'danger' : 'ghost'} onclick={() => reset(list, item.id)}>
				<RotateCcw />{isNew(list, item.id) ? 'Quitar' : 'Volver al estándar'}
			</Button>
		{/if}
	</div>
{/snippet}

{#snippet nameRow(list: ListKey, item: Record<string, unknown>, name: string)}
	<label class="grid grid-cols-[4.5rem_minmax(0,1fr)] items-center gap-2">
		<span class={LBL}>nombre</span>
		<Input size="xs" value={name} onchange={(e) => text(list, item, ['name'], e.currentTarget.value)} />
	</label>
{/snippet}

{#snippet card(list: ListKey, item: { id: string; name: string }, body: Snippet)}
	<div class="rounded-md border bg-depth-0 {open === item.id ? 'border-border-strong shadow-xs' : ''}">
		{@render head(list, item)}
		{#if open === item.id}
			<div class="flex flex-col gap-1.5 border-t px-2.5 py-2">
				{@render body()}
				{@render actions(list, item)}
			</div>
		{/if}
	</div>
{/snippet}

<div class="flex h-full flex-col">
	<div class="flex shrink-0 flex-col gap-2 border-b p-3">
		<p class="text-xs leading-relaxed text-muted-foreground">
			Por defecto, medidas estándar de Argentina. Lo que cambies acá vale para todos los muebles, lo recuerda este navegador y
			viaja dentro de la spec al descargar el paquete o guardar en el servidor.
		</p>
		<div class="flex items-center gap-2">
			<Segmented
				aria-label="Sección de la biblioteca"
				bind:value={section}
				options={[
					{ value: 'boards', label: 'Placas' },
					{ value: 'edges', label: 'Cantos' },
					{ value: 'hardware', label: 'Herrajes' }
				]}
			/>
			<div class="relative min-w-0 flex-1">
				<Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-subtle-foreground" />
				<Input size="sm" class="pl-7" placeholder="buscar" aria-label="buscar en la biblioteca" bind:value={filter} />
			</div>
		</div>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto">
		{#if section === 'boards'}
			<div class="flex flex-col gap-1.5 p-3">
				{#each boards as m (m.id)}
					{@const item = m as Material & Record<string, unknown>}
					{#snippet body()}
						{@render nameRow('materials', item, m.name)}
						{@render numberRow('materials', item, { path: ['nominalThickness'], label: 'espesor', unit: 'mm' })}
						{@render numberRow('materials', item, { path: ['actualThickness'], label: 'espesor real', unit: 'mm' })}
						{@render numberRow('materials', item, { path: ['sheetLength'], label: 'placa: largo', unit: 'mm' })}
						{@render numberRow('materials', item, { path: ['sheetWidth'], label: 'placa: ancho', unit: 'mm' })}
						{#if m.outsourced}
							{@render numberRow('materials', item, { path: ['pricePerM2'], label: 'precio por m²', unit: '$', always: true })}
						{:else}
							{@render numberRow('materials', item, { path: ['pricePerSheet'], label: 'precio por placa', unit: '$', always: true })}
						{/if}
						{@render numberRow('materials', item, { path: ['maxSpan'], label: 'luz sin pandeo', unit: 'mm', always: true })}
						{@render numberRow('materials', item, { path: ['density'], label: 'densidad', unit: 'kg/m³' })}
					{/snippet}
					{@render card('materials', m, body)}
				{:else}
					<EmptyState title="Nada coincide" description="Probá con otro nombre o id." />
				{/each}
			</div>
		{:else if section === 'edges'}
			<div class="flex flex-col gap-1.5 p-3">
				{#each edges as c (c.id)}
					{@const item = c as EdgeMaterial & Record<string, unknown>}
					{#snippet body()}
						{@render nameRow('edgeMaterials', item, c.name)}
						{@render numberRow('edgeMaterials', item, { path: ['thickness'], label: 'espesor', unit: 'mm' })}
						{@render numberRow('edgeMaterials', item, { path: ['pricePerMetre'], label: 'precio por metro', unit: '$', always: true })}
					{/snippet}
					{@render card('edgeMaterials', c, body)}
				{:else}
					<EmptyState title="Nada coincide" description="Probá con otro nombre o id." />
				{/each}
			</div>
		{:else}
			{#each groups as [kind, list] (kind)}
				{@const k = KINDS[kind] ?? OTHER}
				<Section title={k.label} count={list.length} bodyClass="flex flex-col gap-1.5">
					{#each list as h (h.id)}
						{@const item = h as unknown as Record<string, unknown>}
						{#snippet body()}
							{@render nameRow('hardware', item, h.name)}
							{#each k.fields as f (f.path.join('.'))}
								{@render numberRow('hardware', item, f)}
							{/each}
							{#if kind === 'handle' && centres(h) !== null}
								<div class={ROW}>
									<span class={LBL}>entre centros</span>
									<NumberField size="xs" aria-label="entre centros" unit="mm" value={centres(h)} onChange={(v) => setCentres(h, v)} />
								</div>
							{/if}
							{#if (h.bomItems ?? []).length}
								<div class="mt-1 flex flex-col gap-1">
									<span class="text-2xs font-semibold tracking-wide text-subtle-foreground uppercase">Se compra</span>
									{#each h.bomItems ?? [] as b, i (i)}
										<div class="grid grid-cols-[minmax(0,1fr)_3.5rem_5rem] gap-1">
											<Input size="xs" aria-label="artículo" value={b.name} onchange={(e) => write('hardware', item, ['bomItems', i, 'name'], e.currentTarget.value.trim() || b.name)} />
											<NumberField size="xs" title="cantidad por unidad" aria-label="cantidad por unidad" value={b.quantity} onChange={(v) => num('hardware', item, ['bomItems', i, 'quantity'], v)} />
											<NumberField size="xs" title="precio unitario" aria-label="precio unitario" placeholder="$" value={b.unitPrice} onChange={(v) => num('hardware', item, ['bomItems', i, 'unitPrice'], v)} />
										</div>
									{/each}
								</div>
							{/if}
						{/snippet}
						{@render card('hardware', h, body)}
					{/each}
				</Section>
			{:else}
				<EmptyState title="Nada coincide" description="Probá con otro nombre o id." />
			{/each}
		{/if}
	</div>

	<div class="flex shrink-0 flex-col gap-1 border-t bg-depth-1 px-3 py-2">
		<p class="text-xs text-muted-foreground">
			{#if changes === 0}Sin cambios: todo estándar.{:else}<Badge tone="primary" class="mr-1">{changes}</Badge>cambio{changes === 1 ? '' : 's'} del taller.{/if}
		</p>
		<div class="-mx-1.5 flex items-center gap-1">
			<Button size="xs" variant="ghost" onclick={exportFile} disabled={isEmpty(app.workshop)} title="Exportar"><Download />Exportar</Button>
			<Button size="xs" variant="ghost" onclick={() => fileInput?.click()} title="Importar"><Upload />Importar</Button>
			<input bind:this={fileInput} type="file" accept="application/json,.json" onchange={(e) => importFile(e.currentTarget)} hidden />
			<Button size="xs" variant="ghost" onclick={resetAll} disabled={isEmpty(app.workshop)}><RotateCcw />Todo estándar</Button>
		</div>
		{#if message}<p class="text-xs text-muted-foreground">{message}</p>{/if}
	</div>
</div>
