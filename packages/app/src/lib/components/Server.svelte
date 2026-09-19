<script lang="ts">
	/**
	 * The server tab: connection, projects, saved furniture and orders.
	 * Everything here is optional — the engine runs in the browser — so a
	 * server that is down only greys this tab out.
	 */
	import { app } from '$lib/state.svelte';
	import Production from './Production.svelte';

	let tracking: string | null = $state(null);

	let url = $state('');
	let projectId = $state('');
	let newProject = $state('');
	let busy = $state(false);
	let error: string | null = $state(null);

	$effect(() => {
		url = app.server.base;
	});
	$effect(() => {
		if (!projectId && app.projects.length) projectId = app.projects[0].id;
		if (app.current) projectId = app.current.projectId;
	});

	async function run(action: () => Promise<unknown>) {
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

	function when(ts: string) {
		const n = Number(ts);
		return Number.isFinite(n) ? new Date(n * 1000).toLocaleString() : ts;
	}

	const currentOrders = $derived(app.orders.filter((o) => o.furnitureId === app.current?.id));

	/** ERROR findings do not block (the operator decides), so ask. */
	function emit() {
		if (
			app.plan?.status === 'errors' &&
			!confirm('El plan tiene hallazgos ERROR. ¿Emitir la orden igual?')
		) {
			return;
		}
		run(() => app.emitOrder(projectId));
	}
</script>

<div class="server">
	<div class="row">
		<label>
			servidor
			<input bind:value={url} onkeydown={(e) => e.key === 'Enter' && run(() => app.connect(url))} />
		</label>
		<button onclick={() => run(() => app.connect(url))} disabled={busy}>Conectar</button>
		<span class="state {app.serverStatus}">
			{#if app.serverStatus === 'ok'}conectado · motor {app.serverEngine}{:else if app.serverStatus === 'error'}sin
				conexión: {app.serverError}{:else}sin conectar{/if}
		</span>
	</div>
	{#if error}<div class="error">{error}</div>{/if}
	{#if app.serverStatus === 'ok' && app.serverError}<div class="error">{app.serverError}</div>{/if}

	{#if app.serverStatus === 'ok'}
		<div class="cols">
			<section>
				<h4>Este mueble</h4>
				{#if app.current}
					<p>
						<span class="mono">{app.current.id}</span> · versión {app.current.version}
						{#if app.dirty}<b class="dirty">· con cambios sin guardar</b>{/if}
					</p>
				{:else}
					<p class="muted">Todavía no está en el servidor.</p>
					<label>
						proyecto
						<select bind:value={projectId}>
							{#each app.projects as p (p.id)}
								<option value={p.id}>{p.name} ({p.id})</option>
							{/each}
						</select>
					</label>
				{/if}
				<div class="actions">
					<button onclick={() => run(() => app.save(projectId))} disabled={busy || (!app.current && !projectId)}>
						{app.current ? 'Guardar nueva versión' : 'Guardar en el servidor'}
					</button>
					<button
						class="primary"
						onclick={emit}
						disabled={busy || app.plan?.manufacturingBlocked || (!app.current && !projectId)}
						title={app.plan?.manufacturingBlocked ? 'Fabricación bloqueada: corregí los hallazgos FATAL' : ''}
					>
						Emitir orden de fabricación
					</button>
				</div>
				{#if currentOrders.length}
					<h4>Órdenes de este mueble</h4>
					<table>
						<thead><tr><th>Orden</th><th>Versión</th><th>Estado</th><th>Fecha</th><th></th></tr></thead>
						<tbody>
							{#each currentOrders as o (o.id)}
								<tr>
									<td class="mono">{o.id}</td>
									<td class="num">v{o.furnitureVersion}</td>
									<td>{o.status}</td>
									<td>{when(o.createdAt)}</td>
									<td>
										<a href={app.server.packageUrl(o.id)}>zip</a>
										·
										<a href={app.server.packageFileUrl(o.id, 'documentation/report.html')} target="_blank">informe</a>
										·
										<button class="link" onclick={() => (tracking = tracking === o.id ? null : o.id)}>producción</button>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
					{#if tracking && currentOrders.some((o) => o.id === tracking)}
						<Production orderId={tracking} />
					{/if}
				{/if}
			</section>

			<section>
				<h4>Proyectos</h4>
				<div class="row">
					<input placeholder="nuevo proyecto" bind:value={newProject} />
					<button
						onclick={() =>
							run(async () => {
								const p = await app.createProject(newProject.trim());
								projectId = p.id;
								newProject = '';
							})}
						disabled={busy || !newProject.trim()}>Crear</button
					>
				</div>
				<h4>Muebles guardados</h4>
				{#if app.furnitureList.length === 0}
					<p class="muted">Ninguno.</p>
				{:else}
					<table>
						<thead><tr><th>ID</th><th>Nombre</th><th>Proyecto</th><th>Ver.</th><th>Guardado</th><th></th></tr></thead>
						<tbody>
							{#each app.furnitureList as f (f.id)}
								<tr class:current={f.id === app.current?.id}>
									<td class="mono">{f.id}</td>
									<td>{f.name}</td>
									<td class="mono">{f.projectId}</td>
									<td class="num">{f.version}</td>
									<td>{when(f.updatedAt)}</td>
									<td><button onclick={() => run(() => app.open(f.id))} disabled={busy}>Abrir</button></td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
				<h4>Todas las órdenes</h4>
				{#if app.orders.length === 0}
					<p class="muted">Ninguna.</p>
				{:else}
					<table>
						<thead><tr><th>Orden</th><th>Mueble</th><th>Estado</th><th>SHA-256</th><th></th></tr></thead>
						<tbody>
							{#each app.orders as o (o.id)}
								<tr>
									<td class="mono">{o.id}</td>
									<td class="mono">{o.furnitureId} v{o.furnitureVersion}</td>
									<td>{o.status}</td>
									<td class="mono">{o.packageSha256.slice(0, 12)}…</td>
									<td><a href={app.server.packageUrl(o.id)}>zip</a></td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</section>
		</div>
	{/if}
</div>

<style>
	.server {
		font-size: 12px;
	}
	.row {
		display: flex;
		gap: 8px;
		align-items: center;
		margin-bottom: 6px;
	}
	.row label {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	input,
	select {
		font: inherit;
		padding: 2px 6px;
		border: 1px solid #bbb;
		border-radius: 3px;
	}
	.row input {
		width: 220px;
	}
	button {
		font: inherit;
		padding: 3px 10px;
		border: 1px solid #bbb;
		border-radius: 4px;
		background: #fff;
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	button.link {
		border: none;
		background: none;
		color: #246;
		padding: 0;
		text-decoration: underline;
	}
	button.primary {
		background: #ff8c42;
		border-color: #ff8c42;
		color: #fff;
	}
	.state.ok {
		color: #2a7;
	}
	.state.error {
		color: #c33;
	}
	.state.off {
		color: #888;
	}
	.error {
		color: #c33;
		background: #fee;
		padding: 4px 8px;
		margin-bottom: 6px;
	}
	.cols {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		align-items: start;
	}
	h4 {
		margin: 8px 0 4px;
		color: #666;
		font-size: 11px;
		text-transform: uppercase;
	}
	p {
		margin: 4px 0;
	}
	.actions {
		display: flex;
		gap: 8px;
		margin: 6px 0;
	}
	.dirty {
		color: #c90;
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
	tr.current {
		background: #fff3e8;
	}
	.num {
		text-align: right;
	}
	.mono {
		font-family: ui-monospace, monospace;
	}
	.muted {
		color: #888;
	}
</style>
