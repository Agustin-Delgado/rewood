<script lang="ts">
	/**
	 * Production tracking and QC of one order (§52): steps per part, order
	 * steps, measurements against the plan, and the provider packages.
	 */
	import Ruler from '@lucide/svelte/icons/ruler';
	import { app } from '$lib/state.svelte';
	import type { Production, ProviderRole } from '$lib/server';
	import { Badge, Button, Checkbox, Input, Select, recipes } from '$lib/ui';

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
		{ role: 'purchasing', label: 'compras' },
		{ role: 'supplier', label: 'proveedor de placas' }
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
	const partOptions = $derived(partIds.map((id) => ({ value: id, label: id })));
	const pct = $derived(Math.round((prod?.summary.progress ?? 0) * 100));
	const statusTone = (s: string) =>
		s === 'done' ? 'success' : s === 'in_progress' ? 'warning' : s === 'cancelled' ? 'danger' : 'neutral';

	const t = recipes.table();
	const link = recipes.button({ variant: 'link', class: 'text-xs' });
</script>

<div class="mt-1 flex flex-col gap-3 rounded-lg border bg-depth-1 p-3 text-xs">
	{#if error}<div class="rounded-md border border-danger/30 bg-danger-soft px-2.5 py-1.5 text-danger">{error}</div>{/if}
	{#if prod}
		<div class="flex flex-wrap items-center gap-x-2 gap-y-1">
			<span class="font-mono font-medium">{orderId}</span>
			<Badge tone={statusTone(prod.status)}>{prod.status}</Badge>
			<span class="num text-muted-foreground">
				{prod.summary.cut}/{prod.summary.parts} cortadas · {prod.summary.machined} mecanizadas · {prod.summary.edged} canteadas
			</span>
			<span class="flex items-center gap-1.5">
				<span class="h-1.5 w-20 overflow-hidden rounded-full bg-depth-4"><span class="block h-full bg-success" style="width:{pct}%"></span></span>
				<span class="num">{pct}%</span>
			</span>
			{#if prod.summary.qcRecords}
				<Badge tone={prod.summary.qcFailed ? 'danger' : 'neutral'}>QC {prod.summary.qcRecords} ({prod.summary.qcFailed} fuera)</Badge>
			{/if}
			<span class="ml-auto flex flex-wrap gap-2">
				{#each ROLES as r (r.role)}<a class={link} href={app.server.packageUrl(orderId, r.role)}>zip {r.label}</a>{/each}
			</span>
		</div>
		<div class="grid grid-cols-[auto_1fr] items-start gap-4">
			<div class="overflow-hidden rounded-md border bg-depth-0">
				<table class={t.root({ class: 'w-auto' })}>
					<thead>
						<tr>
							<th class={t.th()}>Pieza</th>
							{#each STEPS as s (s)}<th class={t.th({ class: 'text-center' })}>{LABEL[s]}</th>{/each}
						</tr>
					</thead>
					<tbody>
						{#each partIds as id (id)}
							<tr class={t.tr()}>
								<td class={t.td({ class: 'font-mono text-2xs' })}>{id}</td>
								{#each STEPS as s (s)}
									<td class={t.td({ class: 'text-center' })}>
										<Checkbox
											aria-label="{id} {LABEL[s]}"
											checked={prod.parts[id][s]}
											disabled={busy}
											onChange={(c) => run(() => toggle(id, s, c))}
										/>
									</td>
								{/each}
							</tr>
						{/each}
						<tr>
							<td class={t.td({ class: 'font-medium' })}>orden</td>
							<td class={t.td()} colspan="3">
								<div class="flex flex-wrap gap-3">
									{#each ['assembled', 'delivered'] as s (s)}
										<Checkbox
											checked={prod.orderSteps[s]}
											disabled={busy}
											onChange={(c) => run(() => toggle(undefined, s, c))}>{LABEL[s]}</Checkbox
										>
									{/each}
									<Checkbox
										checked={prod.status === 'cancelled'}
										disabled={busy}
										onChange={(c) =>
											run(async () => {
												prod = await app.server.setStatus(orderId, c ? 'cancelled' : 'planned');
											})}>cancelada</Checkbox
									>
								</div>
							</td>
						</tr>
					</tbody>
				</table>
			</div>
			<div class="flex min-w-0 flex-col gap-2">
				<h4 class="text-2xs font-semibold tracking-[0.06em] text-muted-foreground uppercase">Control de calidad</h4>
				<form
					class="flex flex-wrap items-center gap-1.5"
					onsubmit={(e) => {
						e.preventDefault();
						run(qc);
					}}
				>
					<Select aria-label="Pieza" bind:value={qcPart} options={partOptions} class="w-28" />
					<Input placeholder="largo" inputmode="decimal" bind:value={qcL} required class="w-20" />
					<Input placeholder="ancho" inputmode="decimal" bind:value={qcW} required class="w-20" />
					<Input placeholder="esp." inputmode="decimal" bind:value={qcT} required class="w-16" />
					<Button type="submit" disabled={busy}><Ruler />Medir</Button>
				</form>
				{#if prod.qc.length}
					<div class="overflow-hidden rounded-md border bg-depth-0">
						<table class={t.root()}>
							<thead>
								<tr>
									<th class={t.th()}>Pieza</th><th class={t.th({ class: 'text-right' })}>Medido</th>
									<th class={t.th({ class: 'text-right' })}>Desvío</th><th class={t.th({ class: 'text-right' })}>±</th>
									<th class={t.th()}></th>
								</tr>
							</thead>
							<tbody>
								{#each prod.qc as q, i (i)}
									<tr class={t.tr({ class: q.pass ? '' : 'text-danger' })}>
										<td class={t.td({ class: 'font-mono text-2xs' })}>{q.part}</td>
										<td class={t.td({ class: 'num text-right' })}>{q.length} × {q.width} × {q.thickness}</td>
										<td class={t.td({ class: 'num text-right' })}>{q.deviation[0]} / {q.deviation[1]}</td>
										<td class={t.td({ class: 'num text-right' })}>{q.tolerance}</td>
										<td class={t.td()}><Badge tone={q.pass ? 'success' : 'danger'}>{q.pass ? 'ok' : 'fuera'}</Badge></td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}
			</div>
		</div>
	{:else}
		<p class="text-muted-foreground">cargando…</p>
	{/if}
</div>
