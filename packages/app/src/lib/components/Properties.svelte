<script lang="ts">
	/**
	 * Parameters of the spec, editable live, and the selected part's
	 * details. Numbers become number inputs; expressions stay text.
	 */
	import { app, JOINT_KIND_ES } from '$lib/state.svelte';
	import { componentLabels, mm, partName, size } from '$lib/labels';

	const params = $derived(Object.entries(app.spec.parameters ?? {}));
	/** What the template calls each parameter ("Ancho", mm), when it offers it. */
	const optionOf = $derived(new Map((app.plan?.options ?? []).map((o) => [o.param, o])));
	const named = $derived(params.filter(([n]) => optionOf.has(n)));
	const other = $derived(params.filter(([n]) => !optionOf.has(n)));
	const labels = $derived(componentLabels(app.spec, app.plan));
	/** Values the engine worked out, by component, for writing expressions. */
	const derivedGroups = $derived.by(() => {
		const map = new Map<string, [string, number][]>();
		for (const [name, value] of Object.entries(app.plan?.derived ?? {})) {
			const dot = name.indexOf('.');
			const c = dot < 0 ? '' : name.slice(0, dot);
			map.set(c, [...(map.get(c) ?? []), [name, value]]);
		}
		return [...map.entries()];
	});

	function onInput(name: string, raw: string, wasNumber: boolean) {
		const trimmed = raw.trim();
		if (trimmed === '') return;
		const asNumber = Number(trimmed);
		if (wasNumber && Number.isFinite(asNumber)) app.setParameter(name, asNumber);
		else if (!wasNumber && /^-?\d+(\.\d+)?$/.test(trimmed)) app.setParameter(name, asNumber);
		else app.setParameter(name, trimmed);
	}
</script>

<div class="props">
	{#if !app.fastener && !app.selected}
		<p class="hint">Tocá una pieza o un herraje en el 3D para ver su detalle.</p>
	{/if}
	{#if app.fastener}
		{@const { joint, fastener } = app.fastener}
		{@const edge = app.partById(joint.edgePart)}
		{@const face = app.partById(joint.facePart)}
		<h3>Herraje</h3>
		<div class="row"><span class="name">qué es</span><span>{app.hardwareName(fastener.hardware)}</span></div>
		<div class="row"><span class="name">unión</span><span>{joint.id} · {JOINT_KIND_ES[joint.kind]} · {(app.selectedFastener?.index ?? 0) + 1} de {joint.fasteners.length}</span></div>
		{#if joint.edgePart === joint.facePart}
			<div class="row"><span class="name">sobre</span><button class="link" onclick={() => app.selectPart(joint.edgePart)}>{joint.edgePart} {edge?.name ?? ''}</button></div>
		{:else}
			<div class="row"><span class="name">une</span><button class="link" onclick={() => app.selectPart(joint.edgePart)}>{joint.edgePart} {edge?.name ?? ''}</button></div>
			<div class="row"><span class="name">con</span><button class="link" onclick={() => app.selectPart(joint.facePart)}>{joint.facePart} {face?.name ?? ''}</button></div>
		{/if}
		<div class="row"><span class="name">posición</span><span class="mono">({fastener.position.map((v) => Math.round(v * 10) / 10).join(', ')})</span></div>
		<h3>Perforaciones de esta unión</h3>
		<table>
			<tbody>
				{#each [edge, face].filter((p, i, a) => p && a.indexOf(p) === i) as p (p?.id)}
					{#each (p?.operations ?? []).filter((op) => op.source?.joint === joint.id) as op (op.id)}
						<tr>
							<td class="mono">{op.id}</td>
							<td>{op.face}</td>
							<td class="mono">
								{#if op.type === 'DRILL'}
									Ø{op.diameter} {op.through ? 'pasante' : `×${op.depth}`} @ ({op.u}, {op.v})
								{:else if op.type === 'GROOVE'}
									{op.width}×{op.depth}
								{:else if op.type === 'CUTOUT'}
									recorte {op.width}×{op.height} r{op.radius} @ ({op.u}, {op.v})
								{/if}
							</td>
						</tr>
					{/each}
				{/each}
			</tbody>
		</table>
	{/if}

	{#if app.selected}
		{@const p = app.selected}
		{@const hardware = app.hardwareOf(p.id)}
		<h3>{partName(p.name)} <span class="muted">{p.id}</span></h3>
		<div class="row"><span class="name">de</span><span>{labels.get(p.component) ?? p.component}</span></div>
		<div class="row"><span class="name">material</span><span>{p.material}</span></div>
		<div class="row"><span class="name">terminada</span><span>{size(p.dims)}</span></div>
		<div class="row"><span class="name">corte</span><span>{mm(p.cut.length)} × {mm(p.cut.width)}</span></div>
		<div class="row"><span class="name">veta</span><span>{p.grain}</span></div>
		<div class="row"><span class="name">cantos</span><span>{Object.entries(p.edges).map(([f, m]) => `${f}: ${m}`).join(', ') || '—'}</span></div>
		<h3>Herrajes <span class="muted">{hardware.reduce((n, r) => n + r.count, 0)}</span></h3>
		{#if hardware.length === 0}
			<div class="muted">Ninguno: no participa de ninguna unión.</div>
		{:else}
			<table>
				<tbody>
					{#each hardware as r (r.joint.id + r.hardware)}
						<tr>
							<td class="mono">{r.count} ×</td>
							<td>{r.name}</td>
							<td>
								{#if r.other}
									con <button class="link" onclick={() => app.selectPart(r.other?.id ?? null)}>{r.other.id} {r.other.name}</button>
								{:else}
									sobre la pieza
								{/if}
							</td>
							<td class="muted">{JOINT_KIND_ES[r.joint.kind]} · {r.joint.id}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		<details>
		<summary>Mecanizado <span class="muted">{p.operations.length} operaciones</span></summary>
		<table>
			<tbody>
				{#each p.operations as op (op.id)}
					<tr>
						<td class="mono">{op.id.slice(p.id.length + 1)}</td>
						<td>{op.type}</td>
						<td>{op.face}</td>
						<td class="mono">
							{#if op.type === 'DRILL'}
								Ø{op.diameter} {op.through ? 'pasante' : `×${op.depth}`} @ ({op.u}, {op.v})
							{:else if op.type === 'GROOVE'}
								{op.width}×{op.depth} ({op.from.join(',')})–({op.to.join(',')})
							{:else if op.type === 'CUTOUT'}
								recorte {op.width}×{op.height} r{op.radius} @ ({op.u}, {op.v})
							{:else}
								{op.material} {op.length} mm
							{/if}
						</td>
						<td class="muted">{op.source ? `${op.source.joint} ${op.source.hardware}` : ''}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		</details>
	{/if}
	<h3>Parámetros</h3>
	{#each named as [name, value] (name)}
		{@render param(name, value, optionOf.get(name)?.label ?? name, optionOf.get(name)?.unit)}
	{/each}
	{#if other.length}
		<details open={named.length === 0}>
			<summary>{named.length ? 'Otros parámetros' : 'Parámetros de la spec'} <span class="muted">{other.length}</span></summary>
			{#each other as [name, value] (name)}
				{@render param(name, value, name)}
			{/each}
		</details>
	{/if}

	{#if derivedGroups.length}
		<details>
			<summary>Valores calculados <span class="muted">para usar en expresiones</span></summary>
			{#each derivedGroups as [component, values] (component)}
				<div class="dgroup">{labels.get(component) ?? component} <span class="muted mono">{component}</span></div>
				{#each values as [name, value] (name)}
					<div class="row muted"><span class="name" title={name}>{name.slice(component.length + 1) || name}</span><span>{mm(value)}</span></div>
				{/each}
			{/each}
		</details>
	{/if}
</div>

{#snippet param(name: string, value: unknown, label: string, unit?: string)}
	<label class="row" title={name}>
		<span class="name">{label}{unit ? ` (${unit})` : ''}</span>
		{#if typeof value === 'number'}
			<input type="number" step="1" {value} onchange={(e) => onInput(name, e.currentTarget.value, true)} />
		{:else if typeof value === 'boolean'}
			<input type="checkbox" checked={value} onchange={(e) => app.setParameter(name, e.currentTarget.checked)} />
		{:else}
			<input type="text" value={String(value)} class="expr" onchange={(e) => onInput(name, e.currentTarget.value, false)} />
		{/if}
	</label>
{/snippet}

<style>
	.props {
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
		margin: 10px 0 4px;
	}
	.row {
		display: grid;
		grid-template-columns: 110px 1fr;
		gap: 8px;
		align-items: center;
		padding: 2px 0;
	}
	.name {
		color: #345;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	input {
		width: 100%;
		box-sizing: border-box;
		font: inherit;
		padding: 2px 4px;
		border: 1px solid #ccc;
		border-radius: 3px;
	}
	input[type='checkbox'] {
		width: auto;
		justify-self: start;
	}
	.expr {
		font-family: ui-monospace, monospace;
	}
	.muted {
		color: #888;
		font-weight: 400;
		text-transform: none;
		letter-spacing: 0;
	}
	.hint {
		color: #888;
		margin: 6px 0 2px;
	}
	details {
		margin-top: 10px;
	}
	summary {
		cursor: pointer;
		color: #666;
		font-weight: 600;
	}
	.dgroup {
		margin-top: 6px;
		font-weight: 600;
	}
	table {
		border-collapse: collapse;
		width: 100%;
		font-size: 11px;
	}
	td {
		padding: 1px 4px;
		border-bottom: 1px solid #eee;
		vertical-align: top;
	}
	.mono {
		font-family: ui-monospace, monospace;
		white-space: nowrap;
	}
	.link {
		background: none;
		border: none;
		padding: 0;
		font: inherit;
		color: #1a5fb4;
		cursor: pointer;
		text-align: left;
	}
	.link:hover {
		text-decoration: underline;
	}
</style>
