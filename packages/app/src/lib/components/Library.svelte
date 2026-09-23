<script lang="ts">
	/**
	 * The workshop's library: what the engine takes as standard (sizes sold
	 * in Argentina) and what this workshop changed. Every change is a
	 * `libraries` override, applied to every piece and carried inside the
	 * spec that is compiled, packaged or saved.
	 */
	import type { EdgeMaterial, HardwareDef, LibraryOverrides, Material } from '@rewood/engine/browser';
	import { app } from '../state.svelte';
	import { duplicate, hasOverride, isEmpty, resetEntry, setValue } from '../workshop';

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
	function num(list: ListKey, item: Record<string, unknown>, path: (string | number)[], raw: string) {
		const v = Number(raw.replace(',', '.'));
		if (raw.trim() === '' || !Number.isFinite(v)) return;
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
	function setCentres(h: HardwareDef, raw: string) {
		const d = Number(raw.replace(',', '.'));
		if (!Number.isFinite(d) || d <= 0 || (h.holes ?? []).length !== 2) return;
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
</script>

{#snippet numberRow(list: ListKey, item: Record<string, unknown>, f: Field)}
	{@const value = get(item, f.path)}
	{@const std = standard(list, item.id as string, f.path)}
	{#if value !== undefined || std !== undefined || f.always}
		<label class="row" class:changed={std !== undefined && value !== std}>
			<span>{f.label}</span>
			<input inputmode="decimal" value={fmt(value)} onchange={(e) => num(list, item, f.path, e.currentTarget.value)} />
			<span class="unit">{f.unit ?? ''}</span>
			{#if std !== undefined && value !== std}<span class="std" title="medida estándar">estándar {fmt(std)}</span>{/if}
		</label>
	{/if}
{/snippet}

{#snippet head(list: ListKey, item: { id: string; name: string })}
	<button class="item" class:open={open === item.id} onclick={() => (open = open === item.id ? null : item.id)}>
		<span class="name">{item.name}</span>
		{#if isNew(list, item.id)}<span class="tag new">nuevo</span>{:else if hasOverride(app.workshop, list, item.id)}<span class="tag">cambiado</span>{/if}
	</button>
{/snippet}

{#snippet actions(list: ListKey, item: { id: string; name: string })}
	<div class="actions">
		<span class="id mono">{item.id}</span>
		<span class="spacer"></span>
		<button onclick={() => copy(list, item)} title="Un ítem nuevo con estos datos, para cargar otra medida">Duplicar</button>
		{#if hasOverride(app.workshop, list, item.id)}
			<button onclick={() => reset(list, item.id)}>{isNew(list, item.id) ? 'Quitar' : 'Volver al estándar'}</button>
		{/if}
	</div>
{/snippet}

<div class="library">
	<p class="intro">
		Por defecto, medidas estándar de Argentina. Lo que cambies acá vale para todos los muebles, lo recuerda este navegador y
		viaja dentro de la spec al descargar el paquete o guardar en el servidor.
	</p>
	<div class="bar">
		<div class="tabs">
			<button class:active={section === 'boards'} onclick={() => (section = 'boards')}>Placas</button>
			<button class:active={section === 'edges'} onclick={() => (section = 'edges')}>Cantos</button>
			<button class:active={section === 'hardware'} onclick={() => (section = 'hardware')}>Herrajes</button>
		</div>
		<input class="filter" placeholder="buscar" bind:value={filter} />
	</div>

	{#if section === 'boards'}
		{#each boards as m (m.id)}
			{@const item = m as Material & Record<string, unknown>}
			<div class="card">
				{@render head('materials', m)}
				{#if open === m.id}
					<div class="fields">
						<label class="row"><span>nombre</span><input value={m.name} onchange={(e) => text('materials', item, ['name'], e.currentTarget.value)} /></label>
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
						{@render actions('materials', m)}
					</div>
				{/if}
			</div>
		{/each}
	{:else if section === 'edges'}
		{#each edges as c (c.id)}
			{@const item = c as EdgeMaterial & Record<string, unknown>}
			<div class="card">
				{@render head('edgeMaterials', c)}
				{#if open === c.id}
					<div class="fields">
						<label class="row"><span>nombre</span><input value={c.name} onchange={(e) => text('edgeMaterials', item, ['name'], e.currentTarget.value)} /></label>
						{@render numberRow('edgeMaterials', item, { path: ['thickness'], label: 'espesor', unit: 'mm' })}
						{@render numberRow('edgeMaterials', item, { path: ['pricePerMetre'], label: 'precio por metro', unit: '$', always: true })}
						{@render actions('edgeMaterials', c)}
					</div>
				{/if}
			</div>
		{/each}
	{:else}
		{#each groups as [kind, list] (kind)}
			{@const k = KINDS[kind] ?? OTHER}
			<h4>{k.label}</h4>
			{#each list as h (h.id)}
				{@const item = h as unknown as Record<string, unknown>}
				<div class="card">
					{@render head('hardware', h)}
					{#if open === h.id}
						<div class="fields">
							<label class="row"><span>nombre</span><input value={h.name} onchange={(e) => text('hardware', item, ['name'], e.currentTarget.value)} /></label>
							{#each k.fields as f (f.path.join('.'))}
								{@render numberRow('hardware', item, f)}
							{/each}
							{#if kind === 'handle' && centres(h) !== null}
								<label class="row">
									<span>entre centros</span>
									<input inputmode="decimal" value={fmt(centres(h))} onchange={(e) => setCentres(h, e.currentTarget.value)} />
									<span class="unit">mm</span>
								</label>
							{/if}
							{#if (h.bomItems ?? []).length}
								<div class="bom">
									<span class="bomhead">Se compra</span>
									{#each h.bomItems ?? [] as b, i (i)}
										<div class="bomrow">
											<input class="bname" value={b.name} onchange={(e) => write('hardware', item, ['bomItems', i, 'name'], e.currentTarget.value.trim() || b.name)} />
											<input class="bnum" title="cantidad por unidad" inputmode="decimal" value={fmt(b.quantity)} onchange={(e) => num('hardware', item, ['bomItems', i, 'quantity'], e.currentTarget.value)} />
											<input class="bnum" title="precio unitario" placeholder="$" inputmode="decimal" value={fmt(b.unitPrice)} onchange={(e) => num('hardware', item, ['bomItems', i, 'unitPrice'], e.currentTarget.value)} />
										</div>
									{/each}
								</div>
							{/if}
							{@render actions('hardware', h)}
						</div>
					{/if}
				</div>
			{/each}
		{/each}
	{/if}

	<div class="footer">
		<span class="count">{changes === 0 ? 'Sin cambios: todo estándar.' : `${changes} cambio${changes === 1 ? '' : 's'} del taller.`}</span>
		<span class="spacer"></span>
		<button onclick={exportFile} disabled={isEmpty(app.workshop)}>Exportar</button>
		<label class="button">Importar<input type="file" accept="application/json,.json" onchange={(e) => importFile(e.currentTarget)} hidden /></label>
		<button onclick={resetAll} disabled={isEmpty(app.workshop)}>Todo estándar</button>
	</div>
	{#if message}<p class="message">{message}</p>{/if}
</div>

<style>
	.library {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 13px;
	}
	.intro {
		margin: 0 0 4px;
		color: #555;
		line-height: 1.4;
	}
	.bar {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	.tabs {
		display: flex;
		gap: 2px;
	}
	.tabs button {
		border: 1px solid #ccc;
		background: #fff;
		padding: 3px 8px;
		cursor: pointer;
		border-radius: 3px;
	}
	.tabs button.active {
		background: #ff8c42;
		border-color: #ff8c42;
		color: #fff;
	}
	.filter {
		flex: 1;
		min-width: 0;
		padding: 3px 6px;
	}
	h4 {
		margin: 10px 0 2px;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: #777;
	}
	.card {
		border: 1px solid #e3e3e3;
		border-radius: 4px;
	}
	.item {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 6px;
		background: none;
		border: 0;
		padding: 5px 8px;
		text-align: left;
		cursor: pointer;
		font: inherit;
	}
	.item.open {
		background: #fff4ec;
	}
	.name {
		flex: 1;
	}
	.tag {
		font-size: 11px;
		color: #b85c14;
		border: 1px solid #f0c29b;
		border-radius: 8px;
		padding: 0 6px;
	}
	.tag.new {
		color: #1f7a3a;
		border-color: #9fd3ae;
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 6px 8px 8px;
		border-top: 1px solid #eee;
	}
	.row {
		display: grid;
		grid-template-columns: 1fr 90px 30px;
		align-items: center;
		gap: 6px;
	}
	.row input {
		min-width: 0;
		padding: 2px 4px;
	}
	.row:has(> span + input:not([inputmode])) {
		grid-template-columns: 70px 1fr;
	}
	.row.changed input {
		border-color: #ff8c42;
	}
	.std {
		grid-column: 2 / 4;
		font-size: 11px;
		color: #888;
		margin-top: -2px;
	}
	.unit {
		color: #888;
		font-size: 11px;
	}
	.bom {
		display: flex;
		flex-direction: column;
		gap: 3px;
		margin-top: 4px;
	}
	.bomhead {
		font-size: 11px;
		color: #777;
	}
	.bomrow {
		display: grid;
		grid-template-columns: 1fr 44px 70px;
		gap: 4px;
	}
	.bomrow input {
		min-width: 0;
		padding: 2px 4px;
	}
	.actions,
	.footer {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 4px;
	}
	.spacer {
		flex: 1;
	}
	.id {
		color: #999;
		font-size: 11px;
	}
	.mono {
		font-family: ui-monospace, monospace;
	}
	.footer {
		border-top: 1px solid #eee;
		padding-top: 8px;
		margin-top: 8px;
	}
	.count {
		color: #555;
	}
	button,
	.button {
		font: inherit;
		font-size: 12px;
		border: 1px solid #ccc;
		background: #fff;
		border-radius: 3px;
		padding: 3px 8px;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.message {
		margin: 4px 0 0;
		color: #555;
	}
</style>
