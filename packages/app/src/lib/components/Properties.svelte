<script lang="ts">
	/**
	 * Parameters of the spec, editable live, and the selected part's
	 * details. Numbers become number inputs; expressions stay text.
	 */
	import { app, JOINT_KIND_ES } from '$lib/state.svelte';

	const params = $derived(Object.entries(app.spec.parameters ?? {}));
	const derivedValues = $derived(Object.entries(app.plan?.derived ?? {}));

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
	<h3>Parámetros</h3>
	{#each params as [name, value] (name)}
		<label class="row">
			<span class="name">{name}</span>
			{#if typeof value === 'number'}
				<input type="number" step="1" {value} onchange={(e) => onInput(name, e.currentTarget.value, true)} />
			{:else if typeof value === 'boolean'}
				<input type="checkbox" checked={value} onchange={(e) => app.setParameter(name, e.currentTarget.checked)} />
			{:else}
				<input type="text" {value} class="expr" onchange={(e) => onInput(name, e.currentTarget.value, false)} />
			{/if}
		</label>
	{/each}

	{#if derivedValues.length}
		<h3>Derivados</h3>
		{#each derivedValues as [name, value] (name)}
			<div class="row muted"><span class="name">{name}</span><span>{Math.round(value * 1000) / 1000}</span></div>
		{/each}
	{/if}

	{#if app.fastener}
		{@const { joint, fastener } = app.fastener}
		{@const edge = app.partById(joint.edgePart)}
		{@const face = app.partById(joint.facePart)}
		<h3>Herraje</h3>
		<div class="row"><span class="name">qué es</span><span>{app.hardwareName(fastener.hardware)}</span></div>
		<div class="row"><span class="name">unión</span><span>{joint.id} · {JOINT_KIND_ES[joint.kind]} · {fastener.index + 1} de {joint.fasteners.length}</span></div>
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
		<h3>Pieza {p.id}</h3>
		<div class="row"><span class="name">nombre</span><span>{p.name}</span></div>
		<div class="row"><span class="name">material</span><span>{p.material}</span></div>
		<div class="row"><span class="name">terminada</span><span>{p.dims.length} × {p.dims.width} × {p.dims.thickness}</span></div>
		<div class="row"><span class="name">corte</span><span>{p.cut.length} × {p.cut.width}</span></div>
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
		<h3>Operaciones <span class="muted">{p.operations.length}</span></h3>
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
							{:else}
								{op.material} {op.length} mm
							{/if}
						</td>
						<td class="muted">{op.source ? `${op.source.joint} ${op.source.hardware}` : ''}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

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
		font-family: ui-monospace, monospace;
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
