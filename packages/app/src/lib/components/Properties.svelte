<script lang="ts">
	/**
	 * Parameters of the spec, editable live, and the selected part's
	 * details. Numbers become number inputs; expressions stay text.
	 */
	import { app, JOINT_KIND_ES } from '$lib/state.svelte';
	import type { Operation } from '@rewood/engine/browser';
	import { componentLabels, mm, partName, size } from '$lib/labels';
	import { Badge, Button, Checkbox, EmptyState, Field, Input, NumberField, Section, Segmented, recipes } from '$lib/ui';
	import MousePointerClick from '@lucide/svelte/icons/mouse-pointer-click';

	const t = recipes.table();

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

	/**
	 * The doors component a selected door leaf belongs to, when its hinge
	 * side is the user's to pick: one side-hung leaf per bay. With two per
	 * bay each hangs on its own side, and a flap has no side at all.
	 */
	const doorOf = $derived.by(() => {
		const p = app.selected;
		if (!p || !/door_\d+$/.test(p.role)) return null;
		const c = app.spec.components.find((x) => x.id === p.component);
		if (!c || c.type !== 'doors' || (c.opening ?? 'side') !== 'side' || c.hinge === null) return null;
		const prefix = p.role.replace(/\d+$/, '');
		const pair = (app.plan?.parts ?? []).some(
			(q) => q.id !== p.id && q.component === p.component && q.role.replace(/\d+$/, '') === prefix && Math.abs(q.aabb.min[2] - p.aabb.min[2]) < 1
		);
		return { c, pair };
	});

	function setHingeSide(side: 'auto' | 'left' | 'right') {
		const c = doorOf?.c;
		if (!c || c.type !== 'doors') return;
		if (side === 'auto') delete c.hingeSide;
		else c.hingeSide = side;
		app.touch();
	}

	function onInput(name: string, raw: string, wasNumber: boolean) {
		const trimmed = raw.trim();
		if (trimmed === '') return;
		const asNumber = Number(trimmed);
		if (wasNumber && Number.isFinite(asNumber)) app.setParameter(name, asNumber);
		else if (!wasNumber && /^-?\d+(\.\d+)?$/.test(trimmed)) app.setParameter(name, asNumber);
		else app.setParameter(name, trimmed);
	}
</script>

<div class="h-full overflow-y-auto text-sm">
	{#if !app.fastener && !app.selected}
		<EmptyState
			class="py-6"
			title="Nada seleccionado"
			description="Tocá una pieza o un herraje en el 3D para ver su detalle; tocala de nuevo, tocá el fondo o apretá Esc para soltarla. En una puerta elegís hacia qué lado abre."
		>
			{#snippet icon()}<MousePointerClick />{/snippet}
		</EmptyState>
	{/if}

	{#if app.fastener}
		{@const { joint, fastener } = app.fastener}
		{@const edge = app.partById(joint.edgePart)}
		{@const face = app.partById(joint.facePart)}
		<div class="border-b px-3 pt-3 pb-2.5">
			<p class="text-2xs font-semibold tracking-[0.06em] text-subtle-foreground uppercase">Herraje</p>
			<p class="text-base font-semibold">{app.hardwareName(fastener.hardware)}</p>
			<p class="mt-0.5 text-xs text-muted-foreground">
				{JOINT_KIND_ES[joint.kind]} · <span class="font-mono">{joint.id}</span> · {(app.selectedFastener?.index ?? 0) + 1} de {joint.fasteners.length}
			</p>
		</div>
		<dl class="grid grid-cols-[6.5rem_minmax(0,1fr)] items-center gap-x-3 gap-y-1.5 px-3 py-2.5">
			{#if joint.edgePart === joint.facePart}
				<dt class="text-xs text-muted-foreground">sobre</dt>
				<dd>{@render partLink(joint.edgePart, edge?.name)}</dd>
			{:else}
				<dt class="text-xs text-muted-foreground">une</dt>
				<dd>{@render partLink(joint.edgePart, edge?.name)}</dd>
				<dt class="text-xs text-muted-foreground">con</dt>
				<dd>{@render partLink(joint.facePart, face?.name)}</dd>
			{/if}
			<dt class="text-xs text-muted-foreground">posición</dt>
			<dd class="num font-mono text-xs">({fastener.position.map((v) => Math.round(v * 10) / 10).join(', ')})</dd>
		</dl>
		<Section title="Perforaciones de esta unión" static bodyClass="px-0 pb-2">
			<table class={t.root()}>
				<tbody>
					{#each [edge, face].filter((p, i, a) => p && a.indexOf(p) === i) as p (p?.id)}
						{#each (p?.operations ?? []).filter((op) => op.source?.joint === joint.id) as op (op.id)}
							<tr class={t.tr()}>
								<td class={t.td({ class: 'pl-3 font-mono whitespace-nowrap' })}>{op.id}</td>
								<td class={t.td()}>{op.face}</td>
								<td class={t.td({ class: 'pr-3 font-mono whitespace-nowrap' })}>{@render opText(op, false)}</td>
							</tr>
						{/each}
					{/each}
				</tbody>
			</table>
		</Section>
	{/if}

	{#if app.selected}
		{@const p = app.selected}
		{@const hardware = app.hardwareOf(p.id)}
		<div class="border-b px-3 pt-3 pb-2.5">
			<p class="truncate text-2xs font-semibold tracking-[0.06em] text-subtle-foreground uppercase">{labels.get(p.component) ?? p.component}</p>
			<p class="text-base leading-snug font-semibold">{partName(p.name)}</p>
			<p class="mt-0.5 font-mono text-2xs text-muted-foreground">{p.id}</p>
		</div>
		{#if doorOf}
			<div class="border-b px-3 py-2.5">
				<Field label="Abre hacia" hint={doorOf.pair ? 'Cada puerta de su lado (son dos por bahía).' : undefined}>
					{#if !doorOf.pair}
						{@const side = doorOf.c.type === 'doors' ? (doorOf.c.hingeSide ?? 'auto') : 'auto'}
						<Segmented
							fill
							aria-label="Lado de las bisagras"
							value={side}
							onChange={setHingeSide}
							options={[
								{ value: 'left', label: 'izquierda', title: 'Bisagras a la izquierda' },
								{ value: 'right', label: 'derecha', title: 'Bisagras a la derecha' },
								{ value: 'auto', label: 'auto', title: 'Hacia afuera del mueble; en una puerta sola, del lado que no choca' }
							]}
						/>
					{/if}
				</Field>
			</div>
		{/if}
		<dl class="grid grid-cols-[6.5rem_minmax(0,1fr)] items-center gap-x-3 gap-y-1.5 px-3 py-2.5">
			<dt class="text-xs text-muted-foreground">material</dt>
			<dd><Badge tone="primary">{p.material}</Badge></dd>
			<dt class="text-xs text-muted-foreground">terminada</dt>
			<dd class="num font-mono text-xs">{size(p.dims)}</dd>
			<dt class="text-xs text-muted-foreground">corte</dt>
			<dd class="num font-mono text-xs">{mm(p.cut.length)} × {mm(p.cut.width)}</dd>
			<dt class="text-xs text-muted-foreground">veta</dt>
			<dd class="text-xs">{p.grain}</dd>
			<dt class="self-start pt-0.5 text-xs text-muted-foreground">cantos</dt>
			<dd class="flex flex-wrap gap-1">
				{#each Object.entries(p.edges) as [f, m] (f)}
					<Badge title={f}><span class="text-subtle-foreground">{f}</span> {m}</Badge>
				{:else}
					<span class="text-xs text-subtle-foreground">—</span>
				{/each}
			</dd>
		</dl>
		<Section title="Herrajes" count={hardware.reduce((n, r) => n + r.count, 0)} static bodyClass="px-0 pb-2">
			{#if hardware.length === 0}
				<p class="px-3 text-xs text-muted-foreground">Ninguno: no participa de ninguna unión.</p>
			{:else}
				<ul class="grid gap-px">
					{#each hardware as r (r.joint.id + r.hardware)}
						<li class="flex gap-2 px-3 py-1.5 hover:bg-depth-1">
							<span class="num w-6 shrink-0 font-mono text-xs text-muted-foreground">{r.count}×</span>
							<div class="min-w-0 flex-1">
								<p class="text-xs leading-snug">{r.name}</p>
								<p class="mt-0.5 text-2xs leading-snug text-muted-foreground">
									{#if r.other}
										con {@render partLink(r.other.id, r.other.name)}
									{:else}
										sobre la pieza
									{/if}
									· {JOINT_KIND_ES[r.joint.kind]} · <span class="font-mono">{r.joint.id}</span>
								</p>
							</div>
						</li>
					{/each}
				</ul>
			{/if}
		</Section>
		<Section title="Mecanizado" count="{p.operations.length} operaciones" open={false} bodyClass="px-0 pb-2">
			<table class={t.root()}>
				<tbody>
					{#each p.operations as op (op.id)}
						<tr class={t.tr()}>
							<td class={t.td({ class: 'pl-3 font-mono whitespace-nowrap' })}>{op.id.slice(p.id.length + 1)}</td>
							<td class={t.td()}>{op.type}</td>
							<td class={t.td()}>{op.face}</td>
							<td class={t.td({ class: 'font-mono whitespace-nowrap' })}>{@render opText(op, true)}</td>
							<td class={t.td({ class: 'pr-3 text-muted-foreground' })}>{op.source ? `${op.source.joint} ${op.source.hardware}` : ''}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</Section>
	{/if}

	<div class={app.selected || app.fastener ? 'border-t' : ''}>
		<Section title="Parámetros" count={named.length || undefined} static bodyClass="grid gap-1.5 px-3 pb-3">
			{#each named as [name, value] (name)}
				{@render param(name, value, optionOf.get(name)?.label ?? name, optionOf.get(name)?.unit)}
			{:else}
				{#if !other.length}<p class="text-xs text-muted-foreground">Esta spec no tiene parámetros.</p>{/if}
			{/each}
		</Section>
		{#if other.length}
			<Section
				title={named.length ? 'Otros parámetros' : 'Parámetros de la spec'}
				count={other.length}
				open={named.length === 0}
				bodyClass="grid gap-1.5 px-3 pb-3"
			>
				{#each other as [name, value] (name)}
					{@render param(name, value, name)}
				{/each}
			</Section>
		{/if}

		{#if derivedGroups.length}
			<Section title="Valores calculados" open={false} bodyClass="px-3 pb-3">
				<p class="mb-2 text-xs text-muted-foreground">Para usar en expresiones.</p>
				{#each derivedGroups as [component, values] (component)}
					<p class="mt-2 mb-1 text-xs font-semibold">
						{labels.get(component) ?? component} <span class="font-mono font-normal text-subtle-foreground">{component}</span>
					</p>
					<dl class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-0.5 text-xs text-muted-foreground">
						{#each values as [name, value] (name)}
							<dt class="truncate font-mono text-2xs" title={name}>{name.slice(component.length + 1) || name}</dt>
							<dd class="num text-right">{mm(value)}</dd>
						{/each}
					</dl>
				{/each}
			</Section>
		{/if}
	</div>
</div>

{#snippet partLink(id: string, name?: string)}
	<Button variant="link" class="text-left text-[length:inherit] whitespace-normal" onclick={() => app.selectPart(id)}>
		<span class="font-mono">{id}</span>
		{name ?? ''}
	</Button>
{/snippet}

{#snippet opText(op: Operation, full: boolean)}
	{#if op.type === 'DRILL'}
		Ø{op.diameter} {op.through ? 'pasante' : `×${op.depth}`} @ ({op.u}, {op.v})
	{:else if op.type === 'GROOVE'}
		{op.width}×{op.depth}{#if full}&nbsp;({op.from.join(',')})–({op.to.join(',')}){/if}
	{:else if op.type === 'CUTOUT'}
		recorte {op.width}×{op.height} r{op.radius} @ ({op.u}, {op.v})
	{:else if full}
		{op.material} {op.length} mm
	{/if}
{/snippet}

{#snippet param(name: string, value: unknown, label: string, unit?: string)}
	<Field label="{label}{unit ? ` (${unit})` : ''}" layout="inline" for="param-{name}">
		{#if typeof value === 'number'}
			<NumberField
				id="param-{name}"
				size="xs"
				{value}
				decimals={3}
				title={name}
				onChange={(v) => v !== null && Number.isFinite(v) && app.setParameter(name, v)}
			/>
		{:else if typeof value === 'boolean'}
			<Checkbox checked={value} aria-label={label} onChange={(v) => app.setParameter(name, v)} />
		{:else}
			<Input
				id="param-{name}"
				size="xs"
				mono
				value={String(value)}
				title={name}
				onchange={(e: Event) => onInput(name, (e.currentTarget as HTMLInputElement).value, false)}
			/>
		{/if}
	</Field>
{/snippet}
