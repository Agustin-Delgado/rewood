<script lang="ts">
	/** Diagnostics, cut list, BOM and the raw spec, as tabs. */
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import { app } from '$lib/state.svelte';
	import { Badge, Button, EmptyState, Tabs, TabsList, TabsPanel, TabsTab, Textarea, recipes } from '$lib/ui';
	import Nesting from './Nesting.svelte';
	import Server from './Server.svelte';

	type Tab = 'diag' | 'cut' | 'bom' | 'buy' | 'nest' | 'cnc' | 'spec' | 'server';
	let tab: Tab = $state('diag');

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
			app.selectPart(part);
			const owner = app.plan?.parts.find((p) => p.id === part)?.component;
			if (owner) app.focusComponent = owner;
			return;
		}
		if (d.entity && app.spec.components.some((c) => c.id === d.entity)) app.focusComponent = d.entity;
	}

	const severityTone = (s: string) =>
		s === 'FATAL' || s === 'ERROR' ? 'danger' : s === 'WARNING' ? 'warning' : 'info';
	const severityBar: Record<string, string> = {
		FATAL: 'border-l-danger',
		ERROR: 'border-l-danger',
		WARNING: 'border-l-warning',
		INFO: 'border-l-info'
	};
	const statusTone = (s: string) => (s === 'ok' ? 'success' : s === 'warnings' ? 'warning' : 'danger');
	const serverDot: Record<string, string> = { ok: 'bg-success', error: 'bg-danger' };

	const t = recipes.table();
	const num = 'num text-right whitespace-nowrap';
	const panel = 'overflow-auto';
</script>

<div class="flex h-full min-h-0 flex-col bg-depth-0">
	<Tabs bind:value={tab} class="min-h-0 flex-1">
		<TabsList aria-label="Salidas del plan">
			<TabsTab value="diag">
				Hallazgos
				{#if counts.fatal}<Badge tone="danger">{counts.fatal}</Badge>{/if}
				{#if counts.error}<Badge tone="danger">{counts.error}</Badge>{/if}
				{#if counts.warning}<Badge tone="warning">{counts.warning}</Badge>{/if}
			</TabsTab>
			<TabsTab value="cut">Despiece</TabsTab>
			<TabsTab value="bom">BOM</TabsTab>
			<TabsTab value="buy">Compras</TabsTab>
			<TabsTab value="nest">Placas</TabsTab>
			<TabsTab value="cnc">CNC</TabsTab>
			<TabsTab value="spec">Spec JSON</TabsTab>
			<TabsTab value="server">
				Servidor
				<span class="size-1.5 rounded-full {serverDot[app.serverStatus] ?? 'bg-depth-4'}" aria-hidden="true"></span>
			</TabsTab>
			{#snippet end()}
				{#if app.plan}
					<Badge tone={statusTone(app.plan.status)} class="uppercase">{app.plan.status}</Badge>
					{#if app.plan.manufacturingBlocked}<Badge tone="danger">FABRICACIÓN BLOQUEADA</Badge>{/if}
				{/if}
			{/snippet}
		</TabsList>

		<TabsPanel value="diag" class={panel}>
			{#if diags.length === 0}
				<EmptyState title="Sin hallazgos" description="El mueble se puede fabricar tal como está.">
					{#snippet icon()}<CircleCheck />{/snippet}
				</EmptyState>
			{:else}
				<ul class="divide-y divide-border/70">
					{#each diags as d, i (i)}
						<li class="flex items-start gap-3 border-l-2 py-1.5 pr-3 pl-2.5 hover:bg-depth-1 {severityBar[d.severity] ?? 'border-l-border'}">
							<button
								class="grid min-w-0 flex-1 cursor-pointer grid-cols-[4rem_11rem_minmax(0,1fr)] items-baseline gap-x-2.5 gap-y-0.5 text-left text-xs"
								onclick={() => selectEntity(d)}
							>
								<Badge tone={severityTone(d.severity)} class="justify-self-start">{d.severity}</Badge>
								<span class="truncate font-mono text-2xs font-medium text-muted-foreground" title={d.entity}>{d.code}{d.entity ? ` · ${d.entity}` : ''}</span>
								<span class="text-foreground">{d.message}</span>
								{#if d.suggestion}<span class="col-start-3 text-muted-foreground italic">{d.suggestion}</span>{/if}
							</button>
							{#if d.fix}
								<Button variant="soft" size="xs" title="Aplica el cambio a la spec y recompila" onclick={() => app.applyFix(d.fix!)}>
									{d.fix.label}
								</Button>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</TabsPanel>

		<TabsPanel value="cut" class={panel}>
			{#if app.plan}
				<table class={t.root()}>
					<thead>
						<tr>
							<th class={t.th()}>ID</th><th class={t.th()}>Pieza</th><th class={t.th({ class: 'text-right' })}>Cant</th>
							<th class={t.th()}>Material</th><th class={t.th({ class: 'text-right' })}>Corte</th>
							<th class={t.th({ class: 'text-right' })}>Terminada</th><th class={t.th({ class: 'text-right' })}>Esp</th>
							<th class={t.th()}>Veta</th><th class={t.th()}>Cantos</th><th class={t.th({ class: 'text-right' })}>Ops</th>
							<th class={t.th({ class: 'text-right' })}>kg</th>
						</tr>
					</thead>
					<tbody>
						{#each app.plan.partList as r (r.partIds[0])}
							<tr
								class={t.tr({ class: 'cursor-pointer' })}
								data-selected={r.partIds.includes(app.selectedPart ?? '') || undefined}
								onclick={() => app.selectPart(r.partIds[0])}
							>
								<td class={t.td({ class: 'font-mono text-2xs' })}>{r.partIds.join(', ')}</td>
								<td class={t.td()}>{r.name}</td>
								<td class={t.td({ class: num })}>{r.quantity}</td>
								<td class={t.td()}>{r.material}</td>
								<td class={t.td({ class: num })}>{r.cutLength} × {r.cutWidth}</td>
								<td class={t.td({ class: num })}>{r.finishedLength} × {r.finishedWidth}</td>
								<td class={t.td({ class: num })}>{r.thickness}</td>
								<td class={t.td()}>{r.grain}</td>
								<td class={t.td({ class: 'font-mono text-2xs' })}>{r.edges}</td>
								<td class={t.td({ class: num })}>{r.operations}</td>
								<td class={t.td({ class: num })}>{(r.weightKg * r.quantity).toFixed(2)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</TabsPanel>

		<TabsPanel value="bom" class={panel}>
			{#if app.plan}
				<div class="grid grid-cols-3 items-start gap-4 p-3">
					<div class="overflow-hidden rounded-md border">
						<table class={t.root()}>
							<thead>
								<tr>
									<th class={t.th()}>Placa</th><th class={t.th({ class: 'text-right' })}>Piezas</th>
									<th class={t.th({ class: 'text-right' })}>m²</th><th class={t.th({ class: 'text-right' })}>Placas</th>
									<th class={t.th({ class: 'text-right' })}>Aprovech.</th>
								</tr>
							</thead>
							<tbody>
								{#each app.plan.bom.sheets as s (s.material)}
									<tr class={t.tr()}>
										<td class={t.td()}>{s.name}</td><td class={t.td({ class: num })}>{s.parts}</td>
										<td class={t.td({ class: num })}>{s.netAreaM2.toFixed(3)}</td>
										<td class={t.td({ class: num })}>{s.estimatedSheets}</td>
										<td class={t.td({ class: num })}>{Math.round(s.yieldRatio * 100)}%</td>
									</tr>
								{/each}
								{#each app.plan.bom.consumables as c (c.material)}
									<tr class={t.tr()}>
										<td class={t.td()}>{c.name}</td><td class={t.td()}></td>
										<td class={t.td({ class: num })}>{c.lengthM.toFixed(2)} m</td><td class={t.td()}></td><td class={t.td()}></td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
					<div class="overflow-hidden rounded-md border">
						<table class={t.root()}>
							<thead><tr><th class={t.th()}>Herraje</th><th class={t.th({ class: 'text-right' })}>Cant</th></tr></thead>
							<tbody>
								{#each app.plan.bom.hardware as h (h.hardware)}
									<tr class={t.tr()}><td class={t.td({ class: 'font-medium' })}>{h.name}</td><td class={t.td({ class: num })}>{h.quantity}</td></tr>
									{#each h.items as i (i.name)}
										<tr class={t.tr()}>
											<td class={t.td({ class: 'pl-5 text-muted-foreground' })}>{i.name}</td>
											<td class={t.td({ class: `${num} text-muted-foreground` })}>{i.quantity}</td>
										</tr>
									{/each}
								{/each}
								<tr class={t.tr()}>
									<td class={t.td({ class: 'font-medium' })}>Peso total</td>
									<td class={t.td({ class: `${num} font-medium` })}>{app.plan.bom.totalWeightKg.toFixed(2)} kg</td>
								</tr>
							</tbody>
						</table>
					</div>
					{#if app.plan.bom.totalCost > 0}
						{@const b = app.plan.bom}
						<div class="overflow-hidden rounded-md border">
							<table class={t.root()}>
								<thead><tr><th class={t.th()}>Costo estimado</th><th class={t.th({ class: 'text-right' })}>{b.currency}</th></tr></thead>
								<tbody>
									{#each b.sheets as s (s.material)}
										<tr class={t.tr()}><td class={t.td()}>{s.estimatedSheets} × {s.name}</td><td class={t.td({ class: num })}>{s.cost.toFixed(2)}</td></tr>
									{/each}
									{#each b.hardware as h (h.hardware)}
										<tr class={t.tr()}><td class={t.td()}>{h.quantity} × {h.name}</td><td class={t.td({ class: num })}>{h.cost.toFixed(2)}</td></tr>
									{/each}
									{#each b.consumables as c (c.material)}
										<tr class={t.tr()}><td class={t.td()}>{c.lengthM.toFixed(2)} m {c.name}</td><td class={t.td({ class: num })}>{c.cost.toFixed(2)}</td></tr>
									{/each}
									<tr class={t.tr()}>
										<td class={t.td()}>Máquina ({minutes(app.plan.machining.totalSeconds)})</td>
										<td class={t.td({ class: num })}>{b.machiningCost.toFixed(2)}</td>
									</tr>
									<tr class={t.tr()}>
										<td class={t.td({ class: 'font-semibold' })}>Total</td>
										<td class={t.td({ class: `${num} font-semibold` })}>{b.totalCost.toFixed(2)}</td>
									</tr>
									{#if b.unpriced?.length}
										<tr>
											<td class={t.td({ class: 'whitespace-normal text-muted-foreground' })} colspan="2">
												sin precio (el total es un piso): {b.unpriced.join(', ')}
											</td>
										</tr>
									{/if}
								</tbody>
							</table>
						</div>
					{/if}
				</div>
			{/if}
		</TabsPanel>

		<TabsPanel value="buy" class={panel}>
			{#if app.plan}
				<div class="grid grid-cols-3 items-start gap-4 p-3">
					{#each app.plan.purchasing as po (po.supplier)}
						<div class="overflow-hidden rounded-md border">
							<div class="flex items-baseline gap-2 border-b bg-depth-1 px-2 py-1.5">
								<span class="text-xs font-semibold">{po.name}</span>
								{#if po.leadDays}<span class="text-2xs text-muted-foreground">entrega ≈ {po.leadDays} días</span>{/if}
							</div>
							<table class={t.root()}>
								<thead>
									<tr>
										<th class={t.th({ class: 'static' })}>Ítem</th><th class={t.th({ class: 'static text-right' })}>Cant</th>
										<th class={t.th({ class: 'static' })}>Unidad</th><th class={t.th({ class: 'static text-right' })}>Costo</th>
									</tr>
								</thead>
								<tbody>
									{#each po.lines as l (l.kind + l.name + l.unitPrice)}
										<tr class={t.tr()}>
											<td class={t.td()}>{l.name}</td><td class={t.td({ class: num })}>{l.quantity}</td>
											<td class={t.td()}>{l.unit}</td>
											<td class={t.td({ class: num })}>{l.cost ? l.cost.toFixed(2) : '—'}</td>
										</tr>
									{/each}
									<tr class={t.tr()}>
										<td class={t.td({ class: 'font-semibold' })}>Total</td><td class={t.td()}></td><td class={t.td()}></td>
										<td class={t.td({ class: `${num} font-semibold` })}>
											{po.cost ? `${po.cost.toFixed(2)} ${app.plan.bom.currency ?? ''}` : '—'}
										</td>
									</tr>
								</tbody>
							</table>
						</div>
					{/each}
				</div>
			{/if}
		</TabsPanel>

		<TabsPanel value="nest" class={panel}>
			<Nesting />
		</TabsPanel>

		<TabsPanel value="cnc" class={panel}>
			{#if app.plan}
				<p class="flex flex-wrap items-baseline gap-x-1.5 px-3 py-2 text-xs">
					Post <span class="font-mono">{app.plan.machining.postProcessor}</span> · {app.plan.machining.programs.length} programas ·
					tiempo de máquina estimado <span class="num font-semibold">{minutes(app.plan.machining.totalSeconds)}</span>
					<span class="text-muted-foreground">(avances del perfil; sin carga ni canteado; simulado desde el G-code)</span>
				</p>
				<table class={t.root()}>
					<thead>
						<tr>
							<th class={t.th()}>Programa</th><th class={t.th({ class: 'text-right' })}>Ops</th>
							<th class={t.th({ class: 'text-right' })}>Cambios</th><th class={t.th({ class: 'text-right' })}>Corte (m)</th>
							<th class={t.th({ class: 'text-right' })}>Rápidos (m)</th><th class={t.th({ class: 'text-right' })}>Tiempo</th>
						</tr>
					</thead>
					<tbody>
						{#each app.plan.machining.programs as p (p.part + p.setup)}
							<tr
								class={t.tr({ class: 'cursor-pointer' })}
								data-selected={app.selectedPart === p.part || undefined}
								onclick={() => app.selectPart(p.part)}
							>
								<td class={t.td({ class: 'font-mono text-2xs' })}>{p.part}_{p.setup}</td>
								<td class={t.td({ class: num })}>{p.operations}</td>
								<td class={t.td({ class: num })}>{p.toolChanges}</td>
								<td class={t.td({ class: num })}>{(p.cutMm / 1000).toFixed(2)}</td>
								<td class={t.td({ class: num })}>{(p.rapidMm / 1000).toFixed(2)}</td>
								<td class={t.td({ class: num })}>{minutes(p.seconds)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</TabsPanel>

		<TabsPanel value="spec" class="flex flex-col gap-2 bg-depth-1 p-2">
			<Textarea
				mono
				spellcheck="false"
				value={app.specText}
				onchange={(e: Event) => app.applySpecText((e.currentTarget as HTMLTextAreaElement).value)}
				class="min-h-0 flex-1 resize-none text-2xs"
			/>
			{#if app.specError}
				<div class="rounded-md border border-danger/30 bg-danger-soft px-2.5 py-1.5 text-xs text-danger">JSON inválido: {app.specError}</div>
			{/if}
		</TabsPanel>

		<TabsPanel value="server" class="{panel} p-3">
			<Server />
		</TabsPanel>
	</Tabs>
</div>
