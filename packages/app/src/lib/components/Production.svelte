<script lang="ts">
	/**
	 * Production tracking and QC of one order (§52): steps per part, order
	 * steps, measurements against the plan, and the provider packages.
	 */
	import { app } from '$lib/state.svelte';
	import type { Production, ProviderRole } from '$lib/server';

	let { orderId }: { orderId: string } = $props();

	let prod = $state<Production | null>(null);
	let error: string | null = $state(null);
	let busy = $state(false);
	let qcPart = $state('');
	let qcL = $state('');
	let qcW = $state('');
	let qcT = $state('');

	const STEPS = ['cut', 'machined', 'edged'] as const;
	const LABEL: Record<string, string> = { cut: 'cortada', machined: 'mecanizada', edged: 'canteada', assembled: 'armado', delivered: 'entregado' };
	const ROLES: { role: ProviderRole; label: string }[] = [
		{ role: 'all', label: 'todo' },
		{ role: 'cnc', label: 'CNC' },
		{ role: 'cutting', label: 'seccionadora' },
		{ role: 'assembly', label: 'armado' },
		{ role: 'purchasing', label: 'compras' }
	];

	async function run(action: () => Promise<Production | unknown>) {
		busy = true;
		error = null;
		try {
			await action();
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	async function load() {
		prod = await app.server.production(orderId);
		if (!qcPart) qcPart = Object.keys(prod.parts)[0] ?? '';
	}
	$effect(() => {
		orderId;
		run(load);
	});
	async function toggle(part: string | undefined, step: string, done: boolean) {
		prod = await app.server.setStep(orderId, step, done, part);
	}
	async function qc() {
		await app.server.recordQc(orderId, qcPart, Number(qcL), Number(qcW), Number(qcT));
		await load();
		qcL = qcW = qcT = '';
	}
	const partIds = $derived(Object.keys(prod?.parts ?? {}));
	const pct = $derived(Math.round((prod?.summary.progress ?? 0) * 100));
</script>

<div class="prod">
	{#if error}<div class="error">{error}</div>{/if}
	{#if prod}
		<div class="head">
			<b>{orderId}</b> · estado <span class="st {prod.status}">{prod.status}</span> ·
			{prod.summary.cut}/{prod.summary.parts} cortadas · {prod.summary.machined} mecanizadas · {prod.summary.edged} canteadas
			· <span class="bar"><span style="width:{pct}%"></span></span> {pct}%
			{#if prod.summary.qcRecords}· QC {prod.summary.qcRecords} ({prod.summary.qcFailed} fuera){/if}
			<span class="spacer"></span>
			{#each ROLES as r (r.role)}<a href={app.server.packageUrl(orderId, r.role)}>zip {r.label}</a>{/each}
		</div>
		<div class="cols">
			<table>
				<thead><tr><th>Pieza</th>{#each STEPS as s (s)}<th>{LABEL[s]}</th>{/each}</tr></thead>
				<tbody>
					{#each partIds as id (id)}
						<tr>
							<td class="mono">{id}</td>
							{#each STEPS as s (s)}
								<td><input type="checkbox" checked={prod.parts[id][s]} disabled={busy} onchange={(e) => run(() => toggle(id, s, e.currentTarget.checked))} /></td>
							{/each}
						</tr>
					{/each}
					<tr>
						<td>orden</td>
						<td colspan="3">
							{#each ['assembled', 'delivered'] as s (s)}
								<label><input type="checkbox" checked={prod.orderSteps[s]} disabled={busy} onchange={(e) => run(() => toggle(undefined, s, e.currentTarget.checked))} /> {LABEL[s]}</label>
							{/each}
							<label>
								<input type="checkbox" checked={prod.status === 'cancelled'} disabled={busy}
									onchange={(e) => run(async () => { prod = await app.server.setStatus(orderId, e.currentTarget.checked ? 'cancelled' : 'planned'); })} /> cancelada
							</label>
						</td>
					</tr>
				</tbody>
			</table>
			<div>
				<h4>Control de calidad</h4>
				<form class="qc" onsubmit={(e) => { e.preventDefault(); run(qc); }}>
					<select bind:value={qcPart}>{#each partIds as id (id)}<option value={id}>{id}</option>{/each}</select>
					<input placeholder="largo" bind:value={qcL} required />
					<input placeholder="ancho" bind:value={qcW} required />
					<input placeholder="esp." bind:value={qcT} required />
					<button type="submit" disabled={busy}>Medir</button>
				</form>
				{#if prod.qc.length}
					<table>
						<thead><tr><th>Pieza</th><th>Medido</th><th>Desvío</th><th>±</th><th></th></tr></thead>
						<tbody>
							{#each prod.qc as q, i (i)}
								<tr class:fail={!q.pass}>
									<td class="mono">{q.part}</td>
									<td class="num">{q.length} × {q.width} × {q.thickness}</td>
									<td class="num">{q.deviation[0]} / {q.deviation[1]}</td>
									<td class="num">{q.tolerance}</td>
									<td>{q.pass ? 'ok' : 'fuera'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>
		</div>
	{:else}
		<p class="muted">cargando…</p>
	{/if}
</div>

<style>
	.prod {
		font-size: 12px;
		border: 1px solid #ddd;
		border-radius: 4px;
		padding: 6px 8px;
		margin: 4px 0 8px;
	}
	.head {
		display: flex;
		gap: 6px;
		align-items: center;
		flex-wrap: wrap;
		margin-bottom: 6px;
	}
	.head a {
		margin-left: 6px;
	}
	.spacer {
		flex: 1;
	}
	.st.done {
		color: #2a7;
	}
	.st.in_progress {
		color: #c90;
	}
	.st.cancelled {
		color: #c33;
	}
	.bar {
		display: inline-block;
		width: 80px;
		height: 8px;
		background: #eee;
		border-radius: 4px;
		overflow: hidden;
		vertical-align: middle;
	}
	.bar span {
		display: block;
		height: 100%;
		background: #2a7;
	}
	.cols {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 16px;
		align-items: start;
	}
	table {
		border-collapse: collapse;
	}
	th,
	td {
		padding: 1px 8px;
		text-align: left;
		border-bottom: 1px solid #eee;
		white-space: nowrap;
	}
	tr.fail td {
		color: #c33;
	}
	.num {
		text-align: right;
	}
	.mono {
		font-family: ui-monospace, monospace;
	}
	h4 {
		margin: 0 0 4px;
		color: #666;
		font-size: 11px;
		text-transform: uppercase;
	}
	.qc {
		display: flex;
		gap: 4px;
		margin-bottom: 6px;
	}
	.qc input {
		width: 60px;
	}
	input,
	select,
	button {
		font: inherit;
	}
	.error {
		color: #c33;
	}
	.muted {
		color: #888;
	}
</style>
