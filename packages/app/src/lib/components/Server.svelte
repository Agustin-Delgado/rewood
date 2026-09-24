<script lang="ts">
	/**
	 * The server tab: connection, projects, saved furniture and orders.
	 * Everything here is optional — the engine runs in the browser — so a
	 * server that is down only greys this tab out.
	 */
	import Plug from '@lucide/svelte/icons/plug';
	import Save from '@lucide/svelte/icons/save';
	import Send from '@lucide/svelte/icons/send';
	import { app } from '$lib/state.svelte';
	import { Badge, Button, Field, Input, Select, recipes } from '$lib/ui';
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
	const projectOptions = $derived(app.projects.map((p) => ({ value: p.id, label: p.name, hint: p.id })));

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

	const t = recipes.table();
	const link = recipes.button({ variant: 'link', class: 'text-xs' });
	const heading = 'mb-1.5 text-2xs font-semibold tracking-[0.06em] text-muted-foreground uppercase';
</script>

<div class="flex flex-col gap-3 text-xs">
	<div class="flex flex-wrap items-end gap-2">
		<Field label="Servidor" for="server-url" class="w-64">
			<Input id="server-url" mono bind:value={url} onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && run(() => app.connect(url))} />
		</Field>
		<Button onclick={() => run(() => app.connect(url))} disabled={busy}><Plug />Conectar</Button>
		<span class="flex h-7 items-center gap-1.5">
			{#if app.serverStatus === 'ok'}
				<Badge tone="success">conectado</Badge><span class="text-muted-foreground">motor {app.serverEngine}</span>
			{:else if app.serverStatus === 'error'}
				<Badge tone="danger">sin conexión</Badge><span class="text-muted-foreground">{app.serverError}</span>
			{:else}
				<Badge>sin conectar</Badge>
			{/if}
		</span>
	</div>
	{#if error}<div class="rounded-md border border-danger/30 bg-danger-soft px-2.5 py-1.5 text-danger">{error}</div>{/if}
	{#if app.serverStatus === 'ok' && app.serverError}
		<div class="rounded-md border border-danger/30 bg-danger-soft px-2.5 py-1.5 text-danger">{app.serverError}</div>
	{/if}

	{#if app.serverStatus === 'ok'}
		<div class="grid grid-cols-2 items-start gap-4">
			<section class="flex flex-col gap-2">
				<h4 class={heading}>Este mueble</h4>
				{#if app.current}
					<p class="flex items-center gap-1.5">
						<span class="font-mono">{app.current.id}</span>
						<Badge outline>v{app.current.version}</Badge>
						{#if app.dirty}<Badge tone="warning">con cambios sin guardar</Badge>{/if}
					</p>
				{:else}
					<p class="text-muted-foreground">Todavía no está en el servidor.</p>
					<Field label="Proyecto" for="server-project" class="w-64">
						<Select id="server-project" bind:value={projectId} options={projectOptions} />
					</Field>
				{/if}
				<div class="flex gap-2">
					<Button onclick={() => run(() => app.save(projectId))} disabled={busy || (!app.current && !projectId)}>
						<Save />{app.current ? 'Guardar nueva versión' : 'Guardar en el servidor'}
					</Button>
					<Button
						variant="primary"
						onclick={emit}
						disabled={busy || app.plan?.manufacturingBlocked || (!app.current && !projectId)}
						title={app.plan?.manufacturingBlocked ? 'Fabricación bloqueada: corregí los hallazgos FATAL' : ''}
					>
						<Send />Emitir orden de fabricación
					</Button>
				</div>
				{#if currentOrders.length}
					<h4 class="{heading} mt-2">Órdenes de este mueble</h4>
					<div class="overflow-hidden rounded-md border">
						<table class={t.root()}>
							<thead>
								<tr>
									<th class={t.th()}>Orden</th><th class={t.th({ class: 'text-right' })}>Versión</th>
									<th class={t.th()}>Estado</th><th class={t.th()}>Fecha</th><th class={t.th()}></th>
								</tr>
							</thead>
							<tbody>
								{#each currentOrders as o (o.id)}
									<tr class={t.tr()} data-selected={tracking === o.id || undefined}>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{o.id}</td>
										<td class={t.td({ class: 'num text-right' })}>v{o.furnitureVersion}</td>
										<td class={t.td()}><Badge>{o.status}</Badge></td>
										<td class={t.td({ class: 'text-muted-foreground' })}>{when(o.createdAt)}</td>
										<td class={t.td({ class: 'space-x-2 text-right' })}>
											<a class={link} href={app.server.packageUrl(o.id)}>zip</a>
											<a class={link} href={app.server.packageFileUrl(o.id, 'documentation/report.html')} target="_blank">informe</a>
											<Button variant="link" class="text-xs" onclick={() => (tracking = tracking === o.id ? null : o.id)}>producción</Button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
					{#if tracking && currentOrders.some((o) => o.id === tracking)}
						<Production orderId={tracking} />
					{/if}
				{/if}
			</section>

			<section class="flex flex-col gap-2">
				<h4 class={heading}>Proyectos</h4>
				<div class="flex gap-2">
					<Input placeholder="nuevo proyecto" bind:value={newProject} class="w-64" />
					<Button
						onclick={() =>
							run(async () => {
								const p = await app.createProject(newProject.trim());
								projectId = p.id;
								newProject = '';
							})}
						disabled={busy || !newProject.trim()}>Crear</Button
					>
				</div>
				<h4 class="{heading} mt-2">Muebles guardados</h4>
				{#if app.furnitureList.length === 0}
					<p class="text-muted-foreground">Ninguno.</p>
				{:else}
					<div class="overflow-hidden rounded-md border">
						<table class={t.root()}>
							<thead>
								<tr>
									<th class={t.th()}>ID</th><th class={t.th()}>Nombre</th><th class={t.th()}>Proyecto</th>
									<th class={t.th({ class: 'text-right' })}>Ver.</th><th class={t.th()}>Guardado</th><th class={t.th()}></th>
								</tr>
							</thead>
							<tbody>
								{#each app.furnitureList as f (f.id)}
									<tr class={t.tr()} data-selected={f.id === app.current?.id || undefined}>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{f.id}</td>
										<td class={t.td()}>{f.name}</td>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{f.projectId}</td>
										<td class={t.td({ class: 'num text-right' })}>{f.version}</td>
										<td class={t.td({ class: 'text-muted-foreground' })}>{when(f.updatedAt)}</td>
										<td class={t.td({ class: 'text-right' })}>
											<Button size="xs" onclick={() => run(() => app.open(f.id))} disabled={busy}>Abrir</Button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
				<h4 class="{heading} mt-2">Todas las órdenes</h4>
				{#if app.orders.length === 0}
					<p class="text-muted-foreground">Ninguna.</p>
				{:else}
					<div class="overflow-hidden rounded-md border">
						<table class={t.root()}>
							<thead>
								<tr>
									<th class={t.th()}>Orden</th><th class={t.th()}>Mueble</th><th class={t.th()}>Estado</th>
									<th class={t.th()}>SHA-256</th><th class={t.th()}></th>
								</tr>
							</thead>
							<tbody>
								{#each app.orders as o (o.id)}
									<tr class={t.tr()}>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{o.id}</td>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{o.furnitureId} v{o.furnitureVersion}</td>
										<td class={t.td()}><Badge>{o.status}</Badge></td>
										<td class={t.td({ class: 'font-mono text-2xs text-muted-foreground' })}>{o.packageSha256.slice(0, 12)}…</td>
										<td class={t.td({ class: 'text-right' })}><a class={link} href={app.server.packageUrl(o.id)}>zip</a></td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</section>
		</div>
	{/if}
</div>
