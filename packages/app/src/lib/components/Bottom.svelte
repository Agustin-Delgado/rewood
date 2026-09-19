<script lang="ts">
	/** Diagnostics, cut list, BOM and the raw spec, as tabs. */
	import { app } from '$lib/state.svelte';
	import Nesting from './Nesting.svelte';
	import Server from './Server.svelte';

	let tab: 'diag' | 'cut' | 'bom' | 'buy' | 'nest' | 'cnc' | 'spec' | 'server' = $state('diag');

	function minutes(seconds: number) {
		const t = Math.round(seconds);
		return t >= 3600
			? `${Math.floor(t / 3600)} h ${String(Math.floor((t % 3600) / 60)).padStart(2, '0')} min`
			: `${Math.floor(t / 60)} min ${String(t % 60).padStart(2, '0')} s`;
	}
	const diags = $derived(app.plan?.diagnostics.items ?? []);
	const counts = $derived({
		fatal: diags.filter((d) => d.severity === 'FATAL').length,
		error: diags.filter((d) => d.severity === 'ERROR').length,
		warning: diags.filter((d) => d.severity === 'WARNING').length
	});

	/** A finding names a part, a component or a constraint: go there. */
	function selectEntity(d: { entity?: string; location?: string }) {
		const part = [d.entity, d.location].find((e) => e && /^P\d+$/.test(e));
		if (part) {
			app.selectedPart = part;
			const owner = app.plan?.parts.find((p) => p.id === part)?.component;
			if (owner) app.focusComponent = owner;
			return;
		}
		if (d.entity && app.spec.components.some((c) => c.id === d.entity)) app.focusComponent = d.entity;
	}
</script>

<div class="bottom">
	<div class="tabs">
		<button class:active={tab === 'diag'} onclick={() => (tab = 'diag')}>
			Hallazgos
			{#if counts.fatal}<span class="badge fatal">{counts.fatal}</span>{/if}
			{#if counts.error}<span class="badge error">{counts.error}</span>{/if}
			{#if counts.warning}<span class="badge warning">{counts.warning}</span>{/if}
		</button>
		<button class:active={tab === 'cut'} onclick={() => (tab = 'cut')}>Despiece</button>
		<button class:active={tab === 'bom'} onclick={() => (tab = 'bom')}>BOM</button>
		<button class:active={tab === 'buy'} onclick={() => (tab = 'buy')}>Compras</button>
		<button class:active={tab === 'nest'} onclick={() => (tab = 'nest')}>Placas</button>
		<button class:active={tab === 'cnc'} onclick={() => (tab = 'cnc')}>CNC</button>
		<button class:active={tab === 'spec'} onclick={() => (tab = 'spec')}>Spec JSON</button>
		<button class:active={tab === 'server'} onclick={() => (tab = 'server')}>
			Servidor
			<span class="dot {app.serverStatus}"></span>
		</button>
		{#if app.plan}
			<span class="status {app.plan.status}"
				>{app.plan.status}{app.plan.manufacturingBlocked ? ' · FABRICACIÓN BLOQUEADA' : ''}</span
			>
		{/if}
	</div>
	<div class="body">
		{#if tab === 'diag'}
			{#if diags.length === 0}
				<p class="muted">Sin hallazgos.</p>
			{/if}
			{#each diags as d, i (i)}
				<button class="diag {d.severity}" onclick={() => selectEntity(d)}>
					<b>{d.severity} {d.code}</b>
					<span class="mono">{d.entity ?? ''}</span>
					<span>{d.message}</span>
					{#if d.suggestion}<i>{d.suggestion}</i>{/if}
				</button>
			{/each}
		{:else if tab === 'cut' && app.plan}
			<table>
				<thead>
					<tr>
						<th>ID</th><th>Pieza</th><th>Cant</th><th>Material</th><th>Corte</th><th>Terminada</th
						><th>Esp</th><th>Veta</th><th>Cantos</th><th>Ops</th><th>kg</th>
					</tr>
				</thead>
				<tbody>
					{#each app.plan.partList as r (r.partIds[0])}
						<tr onclick={() => (app.selectedPart = r.partIds[0])}>
							<td class="mono">{r.partIds.join(', ')}</td>
							<td>{r.name}</td>
							<td class="num">{r.quantity}</td>
							<td>{r.material}</td>
							<td class="num">{r.cutLength} × {r.cutWidth}</td>
							<td class="num">{r.finishedLength} × {r.finishedWidth}</td>
							<td class="num">{r.thickness}</td>
							<td>{r.grain}</td>
							<td class="mono">{r.edges}</td>
							<td class="num">{r.operations}</td>
							<td class="num">{(r.weightKg * r.quantity).toFixed(2)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else if tab === 'bom' && app.plan}
			<div class="cols">
				<table>
					<thead><tr><th>Placa</th><th>Piezas</th><th>m²</th><th>Placas</th><th>Aprovech.</th></tr></thead>
					<tbody>
						{#each app.plan.bom.sheets as s (s.material)}
							<tr
								><td>{s.name}</td><td class="num">{s.parts}</td><td class="num">{s.netAreaM2.toFixed(3)}</td
								><td class="num">{s.estimatedSheets}</td><td class="num">{Math.round(s.yieldRatio * 100)}%</td></tr
							>
						{/each}
						{#each app.plan.bom.consumables as c (c.material)}
							<tr><td>{c.name}</td><td></td><td class="num">{c.lengthM.toFixed(2)} m</td><td></td><td></td></tr>
						{/each}
					</tbody>
				</table>
				<table>
					<thead><tr><th>Herraje</th><th>Cant</th></tr></thead>
					<tbody>
						{#each app.plan.bom.hardware as h (h.hardware)}
							<tr><td><b>{h.name}</b></td><td class="num">{h.quantity}</td></tr>
							{#each h.items as i (i.name)}
								<tr><td class="muted">&nbsp;&nbsp;{i.name}</td><td class="num">{i.quantity}</td></tr>
							{/each}
						{/each}
						<tr><td>Peso total</td><td class="num">{app.plan.bom.totalWeightKg.toFixed(2)} kg</td></tr>
					</tbody>
				</table>
				{#if app.plan.bom.totalCost > 0}
					{@const b = app.plan.bom}
					<table>
						<thead><tr><th>Costo estimado</th><th>{b.currency}</th></tr></thead>
						<tbody>
							{#each b.sheets as s (s.material)}<tr><td>{s.estimatedSheets} × {s.name}</td><td class="num">{s.cost.toFixed(2)}</td></tr>{/each}
							{#each b.hardware as h (h.hardware)}<tr><td>{h.quantity} × {h.name}</td><td class="num">{h.cost.toFixed(2)}</td></tr>{/each}
							{#each b.consumables as c (c.material)}<tr><td>{c.lengthM.toFixed(2)} m {c.name}</td><td class="num">{c.cost.toFixed(2)}</td></tr>{/each}
							<tr><td>Máquina ({minutes(app.plan.machining.totalSeconds)})</td><td class="num">{b.machiningCost.toFixed(2)}</td></tr>
							<tr><td><b>Total</b></td><td class="num"><b>{b.totalCost.toFixed(2)}</b></td></tr>
							{#if b.unpriced?.length}<tr><td class="muted" colspan="2">sin precio (el total es un piso): {b.unpriced.join(', ')}</td></tr>{/if}
						</tbody>
					</table>
				{/if}
			</div>
		{:else if tab === 'buy' && app.plan}
			<div class="cols">
				{#each app.plan.purchasing as po (po.supplier)}
					<table>
						<thead>
							<tr><th colspan="4">{po.name}{po.leadDays ? ` · entrega ≈ ${po.leadDays} días` : ''}</th></tr>
							<tr><th>Ítem</th><th>Cant</th><th>Unidad</th><th>Costo</th></tr>
						</thead>
						<tbody>
							{#each po.lines as l (l.kind + l.name + l.unitPrice)}
								<tr><td>{l.name}</td><td class="num">{l.quantity}</td><td>{l.unit}</td><td class="num">{l.cost ? l.cost.toFixed(2) : '—'}</td></tr>
							{/each}
							<tr><td><b>Total</b></td><td></td><td></td><td class="num"><b>{po.cost ? `${po.cost.toFixed(2)} ${app.plan.bom.currency ?? ''}` : '—'}</b></td></tr>
						</tbody>
					</table>
				{/each}
			</div>
		{:else if tab === 'nest'}
			<Nesting />
		{:else if tab === 'cnc' && app.plan}
			<p>
				Post <span class="mono">{app.plan.machining.postProcessor}</span> · {app.plan.machining.programs.length} programas ·
				tiempo de máquina estimado <b>{minutes(app.plan.machining.totalSeconds)}</b>
				<span class="muted">(avances del perfil; sin carga ni canteado; simulado desde el G-code)</span>
			</p>
			<table>
				<thead>
					<tr><th>Programa</th><th>Ops</th><th>Cambios</th><th>Corte (m)</th><th>Rápidos (m)</th><th>Tiempo</th></tr>
				</thead>
				<tbody>
					{#each app.plan.machining.programs as p (p.part + p.setup)}
						<tr onclick={() => (app.selectedPart = p.part)}>
							<td class="mono">{p.part}_{p.setup}</td>
							<td class="num">{p.operations}</td>
							<td class="num">{p.toolChanges}</td>
							<td class="num">{(p.cutMm / 1000).toFixed(2)}</td>
							<td class="num">{(p.rapidMm / 1000).toFixed(2)}</td>
							<td class="num">{minutes(p.seconds)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else if tab === 'spec'}
			<textarea spellcheck="false" value={app.specText} onchange={(e) => app.applySpecText(e.currentTarget.value)}
			></textarea>
			{#if app.specError}<div class="diag FATAL">JSON inválido: {app.specError}</div>{/if}
		{:else if tab === 'server'}
			<Server />
		{/if}
	</div>
</div>

<style>
	.bottom {
		display: flex;
		flex-direction: column;
		height: 100%;
		font-size: 12px;
	}
	.tabs {
		display: flex;
		gap: 2px;
		border-bottom: 1px solid #ddd;
		background: #fafafa;
		align-items: center;
	}
	.tabs button {
		border: none;
		background: none;
		padding: 6px 12px;
		cursor: pointer;
		font: inherit;
		border-bottom: 2px solid transparent;
	}
	.tabs button.active {
		border-bottom-color: #ff8c42;
		font-weight: 600;
	}
	.status {
		margin-left: auto;
		padding: 0 12px;
		font-weight: 600;
		text-transform: uppercase;
	}
	.status.ok {
		color: #2a7;
	}
	.status.warnings {
		color: #c90;
	}
	.status.errors,
	.status.blocked {
		color: #c33;
	}
	.badge {
		display: inline-block;
		min-width: 16px;
		padding: 0 5px;
		border-radius: 8px;
		color: #fff;
		font-size: 10px;
		margin-left: 4px;
	}
	.badge.fatal {
		background: #900;
	}
	.badge.error {
		background: #c40;
	}
	.badge.warning {
		background: #c90;
	}
	.dot {
		display: inline-block;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: #bbb;
		margin-left: 4px;
		vertical-align: middle;
	}
	.dot.ok {
		background: #2a7;
	}
	.dot.error {
		background: #c33;
	}
	.body {
		overflow: auto;
		flex: 1;
		padding: 6px;
	}
	.diag {
		display: grid;
		grid-template-columns: 110px 60px 1fr;
		gap: 8px;
		width: 100%;
		text-align: left;
		border: none;
		border-left: 4px solid #999;
		background: #f6f6f6;
		padding: 3px 8px;
		margin: 2px 0;
		font: inherit;
		cursor: pointer;
	}
	.diag i {
		grid-column: 3;
		color: #555;
	}
	.diag.FATAL {
		border-color: #900;
		background: #fee;
	}
	.diag.ERROR {
		border-color: #c40;
		background: #fee8e0;
	}
	.diag.WARNING {
		border-color: #c90;
		background: #fff6dd;
	}
	.diag.INFO {
		border-color: #39c;
		background: #eef6fc;
	}
	table {
		border-collapse: collapse;
		width: 100%;
	}
	th,
	td {
		text-align: left;
		padding: 2px 8px;
		border-bottom: 1px solid #eee;
		white-space: nowrap;
	}
	th {
		color: #666;
		font-weight: 600;
	}
	tbody tr:hover {
		background: #eef2f7;
		cursor: pointer;
	}
	.num {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.mono {
		font-family: ui-monospace, monospace;
	}
	.muted {
		color: #888;
	}
	.cols {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 16px;
		align-items: start;
	}
	textarea {
		width: 100%;
		height: 100%;
		min-height: 220px;
		box-sizing: border-box;
		font: 11px ui-monospace, monospace;
		border: 1px solid #ddd;
	}
</style>
