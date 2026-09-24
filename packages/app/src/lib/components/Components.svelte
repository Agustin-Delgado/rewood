<script lang="ts">
	/**
	 * Form editor for the spec's components. Every edit mutates `app.spec`
	 * in place and recompiles; the JSON tab shows the same thing. Options
	 * come from the engine's libraries, so nothing here is typed by hand.
	 */
	import type { ComponentSpec, HandleSpec, HardwareDef, JointSpec, LibraryOverrides, NumOrExpr, ZoneSpec } from '@rewood/engine/browser';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { app } from '$lib/state.svelte';
	import { Badge, Button, Checkbox, Input, Section, Select, Textarea, Toggle, recipes, type SelectOption } from '$lib/ui';

	const libs = $derived(app.libraries);
	const hardware = $derived(Object.values(libs?.hardware.items ?? {}));
	const materials = $derived(Object.values(libs?.materials.materials ?? {}));
	const edgeMaterials = $derived(Object.values(libs?.materials.edgeMaterials ?? {}));
	const byKind = (kinds: string[]) => hardware.filter((h) => kinds.includes(h.kind));
	const fasteners = $derived(
		hardware.filter((h) => !['hinge', 'slide', 'handle', 'leg', 'clip', 'rail_support', 'rail', 'hanger', 'pin_row', 'pin', 'catch', 'strike', 'spacer'].includes(h.kind))
	);
	// Base variants: the engine swaps the arm and the damper in itself.
	const hingeFamilies = $derived(byKind(['hinge']).filter((h) => !h.hinge?.softClose && h.hinge?.mount !== 'half_overlay' && h.hinge?.mount !== 'inset'));
	const plainSlides = $derived(byKind(['slide']).filter((h) => !h.slide?.softClose));
	const catches = $derived(byKind(['catch']));

	/** The "nothing chosen" option of a select: the list keys items by value, so it can't be ''. */
	const NONE = '__none';
	/** What the engine puts under a carcass whose `legs` names no hardware. */
	const DEFAULT_LEG = 'leg_adjustable_100';
	const opt = (items: { id: string; name: string }[]): SelectOption[] => items.map((x) => ({ value: x.id, label: x.name }));
	const hwOpt = (items: HardwareDef[]) => opt(items);
	const withNone = (label: string, items: SelectOption[]): SelectOption[] => [{ value: NONE, label }, ...items];
	const orNone = (v: string) => (v === NONE ? '' : v);
	const materialOptions = $derived(opt(materials));

	/** Number or expression from a text field. */
	function numOrExpr(raw: string): NumOrExpr {
		const t = raw.trim();
		return /^-?\d+(\.\d+)?$/.test(t) ? Number(t) : t;
	}
	function show(v: NumOrExpr | undefined): string {
		return v === undefined ? '' : String(v);
	}

	function setField(c: ComponentSpec, key: string, raw: string, optional = false) {
		const rec = c as unknown as Record<string, unknown>;
		if (optional && raw.trim() === '') delete rec[key];
		else rec[key] = numOrExpr(raw);
		app.touch();
	}
	function setText(c: ComponentSpec, key: string, raw: string, optional = false) {
		const rec = c as unknown as Record<string, unknown>;
		if (optional && raw === '') delete rec[key];
		else rec[key] = raw;
		app.touch();
	}
	function toggleHardware(c: { joint?: JointSpec }, id: string) {
		// Shelves may carry no joint yet (they were on pins): make one.
		const joint = (c.joint ??= { hardware: [] });
		const i = joint.hardware.indexOf(id);
		if (i >= 0) joint.hardware.splice(i, 1);
		else joint.hardware.push(id);
		app.touch();
	}
	function setZone(c: ComponentSpec, on: boolean) {
		const rec = c as unknown as { zone?: ZoneSpec };
		if (on) rec.zone = rec.zone ?? { from: 0, to: 'height' };
		else delete rec.zone;
		app.touch();
	}
	// --- constraints -------------------------------------------------------
	function addConstraint() {
		const list = (app.spec.constraints ??= []);
		const n = list.length + 1;
		list.push({ id: `rule_${n}`, expr: 'width <= 2400', severity: 'ERROR', message: '' });
		app.touch();
	}
	function removeConstraint(i: number) {
		app.spec.constraints?.splice(i, 1);
		if (app.spec.constraints?.length === 0) delete app.spec.constraints;
		app.touch();
	}
	/** The finding a constraint produced in the current plan, if any. */
	function findingOf(id: string) {
		return app.plan?.diagnostics.items.find((d) => d.code === 'CON-001' && d.entity === id);
	}

	// --- library overrides ---------------------------------------------------
	// Overrides are nested data (materials, hardware, profile): edited as a
	// scoped JSON block, validated on the way in.
	let libText = $state('');
	let libError: string | null = $state(null);
	let libOpen = $state(false);
	$effect(() => {
		libText = app.spec.libraries ? JSON.stringify(app.spec.libraries, null, 2) : '';
	});
	function applyLibraries(raw: string) {
		libText = raw;
		if (raw.trim() === '') {
			delete app.spec.libraries;
			libError = null;
			app.touch();
			return;
		}
		try {
			app.spec.libraries = JSON.parse(raw) as LibraryOverrides;
			libError = null;
			app.touch();
		} catch (e) {
			libError = (e as Error).message;
		}
	}
	const overrideCount = $derived.by(() => {
		const l = app.spec.libraries;
		if (!l) return 0;
		return (l.materials?.length ?? 0) + (l.edgeMaterials?.length ?? 0) + (l.hardware?.length ?? 0) + (l.profile ? 1 : 0);
	});

	function setOptionalJoint(c: ComponentSpec, key: 'hinge' | 'handle' | 'frontFixing', on: boolean, defaultHardware: string) {
		const rec = c as unknown as Record<string, JointSpec | HandleSpec | null | undefined>;
		if (on) rec[key] = { hardware: [defaultHardware] };
		else if (key === 'handle') delete rec[key];
		else rec[key] = null;
		app.touch();
	}

	let newType: ComponentSpec['type'] = $state('shelves');
	/** The first free id for a type: `shelves`, then `shelves_2`, `shelves_3`… */
	function freeId(type: string): string {
		const taken = new Set(app.spec.components.map((c) => c.id));
		if (!taken.has(type)) return type;
		let n = 2;
		while (taken.has(`${type}_${n}`)) n += 1;
		return `${type}_${n}`;
	}
	/** Ids key the list and every reference to a component: two alike break both. */
	let idError: { component: string; message: string } | null = $state(null);
	function rename(c: ComponentSpec, input: HTMLInputElement) {
		const id = input.value.trim();
		if (id === c.id) return;
		const clash = id === '' ? 'no puede quedar vacío' : app.spec.components.some((o) => o !== c && o.id === id) ? `ya hay un componente '${id}'` : null;
		if (clash) {
			idError = { component: c.id, message: clash };
			input.value = c.id;
			return;
		}
		idError = null;
		setText(c, 'id', id);
	}
	function addComponent() {
		const id = freeId(newType);
		let c: ComponentSpec;
		switch (newType) {
			case 'carcass':
				c = { type: 'carcass', id, joint: { hardware: ['minifix_15', 'dowel_8x30'] }, back: { material: 'hdf_3' } };
				break;
			case 'shelves':
				c = { type: 'shelves', id, count: 2, joint: { hardware: ['dowel_8x30'] } };
				break;
			case 'doors':
				c = { type: 'doors', id, count: 1 };
				break;
			case 'drawers':
				c = {
					type: 'drawers',
					id,
					count: 3,
					joint: { hardware: ['dowel_8x30'], placement: { endOffset: 40, maxSpacing: 150 } },
					slide: { hardware: ['slide_ball_450'] }
				};
				break;
			case 'rail':
				c = { type: 'rail', id };
				break;
			case 'worktop':
				c = { type: 'worktop', id, overhang: { front: 20 } };
				break;
			case 'panel':
				c = { type: 'panel', id, x: 0 };
				break;
			case 'modesty':
				c = { type: 'modesty', id, height: 300 };
				break;
			case 'sliding_doors':
				c = { type: 'sliding_doors', id, count: 2 };
				break;
			case 'sink':
				c = { type: 'sink', id, hardware: ['sink_ferrum_imola'], passage: {} };
				break;
		}
		app.spec.components.push(c);
		app.touch();
	}
	function remove(i: number) {
		app.spec.components.splice(i, 1);
		app.touch();
	}
	function move(i: number, d: number) {
		const j = i + d;
		if (j < 0 || j >= app.spec.components.length) return;
		const c = app.spec.components;
		[c[i], c[j]] = [c[j], c[i]];
		app.touch();
	}
	const inactive = $derived(new Set(app.plan?.inactive ?? []));
	const TYPE_ES: Record<ComponentSpec['type'], string> = {
		carcass: 'carcasa',
		shelves: 'estantes',
		doors: 'puertas',
		drawers: 'cajones',
		rail: 'barral',
		worktop: 'tapa',
		panel: 'lateral',
		modesty: 'faldón',
		sliding_doors: 'corredizas',
		sink: 'bacha'
	};
	const NEW_TYPES: SelectOption<ComponentSpec['type']>[] = [
		{ value: 'carcass', label: 'carcasa' },
		{ value: 'shelves', label: 'estantes' },
		{ value: 'doors', label: 'puertas' },
		{ value: 'drawers', label: 'cajones' },
		{ value: 'rail', label: 'barral' },
		{ value: 'worktop', label: 'tapa de trabajo' },
		{ value: 'panel', label: 'lateral de apoyo' },
		{ value: 'modesty', label: 'faldón' },
		{ value: 'sliding_doors', label: 'puertas corredizas' },
		{ value: 'sink', label: 'bacha' }
	];
	const SEVERITIES: SelectOption<'INFO' | 'WARNING' | 'ERROR' | 'FATAL'>[] = [
		{ value: 'INFO', label: 'INFO' },
		{ value: 'WARNING', label: 'WARNING' },
		{ value: 'ERROR', label: 'ERROR' },
		{ value: 'FATAL', label: 'FATAL' }
	];
	let open: Record<string, boolean> = $state({});
	// A finding clicked in the bottom panel unfolds its component.
	let cards: Record<string, HTMLElement> = $state({});
	$effect(() => {
		const id = app.focusComponent;
		if (!id) return;
		open[id] = true;
		app.focusComponent = null;
		requestAnimationFrame(() => cards[id]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
	});
	const RANK = { INFO: 0, WARNING: 1, ERROR: 2, FATAL: 3 } as const;
	type Severity = keyof typeof RANK;
	function worst(list: { severity: Severity }[]) {
		return list.reduce<Severity | null>((w, d) => (w === null || RANK[d.severity] > RANK[w] ? d.severity : w), null);
	}
	const TONE = { INFO: 'info', WARNING: 'warning', ERROR: 'danger', FATAL: 'danger' } as const;
	const WELL = { INFO: 'bg-info-soft', WARNING: 'bg-warning-soft', ERROR: 'bg-danger-soft', FATAL: 'bg-danger-soft' } as const;

	// A label and its control, on one line.
	const ROW = 'grid grid-cols-[104px_minmax(0,1fr)] items-center gap-2 py-0.5';
	const LBL = 'truncate text-xs text-muted-foreground';
	const INLINE = 'flex min-w-0 items-center gap-1.5';
</script>

<div class="h-full overflow-y-auto">
	<Section title="Mueble">
		<div class="flex flex-col gap-1">
			<label class={ROW}><span class={LBL}>id</span><Input size="xs" mono value={app.spec.id} onchange={(e) => { app.spec.id = e.currentTarget.value; app.touch(); }} /></label>
			<label class={ROW}><span class={LBL}>nombre</span><Input size="xs" value={app.spec.name} onchange={(e) => { app.spec.name = e.currentTarget.value; app.touch(); }} /></label>
			<div class={ROW}><span class={LBL}>material</span>
				<Select size="xs" aria-label="material" options={materialOptions} value={app.spec.material} onChange={(v) => { app.spec.material = v; app.touch(); }} />
			</div>
			<div class={ROW}><span class={LBL}>canto</span>
				<Select size="xs" aria-label="canto" options={withNone('— sin canto —', opt(edgeMaterials))} value={app.spec.edgeMaterial ?? NONE}
					onChange={(v) => { app.spec.edgeMaterial = orNone(v) || undefined; app.touch(); }} />
			</div>
		</div>
	</Section>

	<Section title="Componentes" count={app.spec.components.length}>
		<div class="flex flex-col gap-1.5">
			{#each app.spec.components as c, i (c.id)}
				{@const findings = app.findingsFor(c.id)}
				{@const level = worst(findings)}
				<div
					class="rounded-md border bg-depth-0 transition-colors {level === 'ERROR' || level === 'FATAL' ? 'border-danger/60' : level === 'WARNING' ? 'border-warning/60' : ''} {inactive.has(c.id) ? 'border-dashed opacity-60' : ''}"
					bind:this={cards[c.id]}
				>
					<div class="flex min-w-0 items-center gap-1 px-1 py-1">
						<Button variant="ghost" size="icon-xs" aria-label={open[c.id] ? 'plegar' : 'desplegar'} onclick={() => (open[c.id] = !open[c.id])}>
							<ChevronRight class="transition-transform {open[c.id] ? 'rotate-90' : ''}" />
						</Button>
						<span class="shrink-0 text-xs font-medium">{TYPE_ES[c.type]}</span>
						<Input size="xs" mono class="w-28" aria-label="id del componente" value={c.id} onchange={(e) => rename(c, e.currentTarget)} />
						{#if level}
							<button class={recipes.badge({ tone: TONE[level] })} title="ver hallazgos" onclick={() => (open[c.id] = true)}>{findings.length}</button>
						{/if}
						{#if inactive.has(c.id)}
							<Badge outline title={c.when !== undefined ? `condición: ${String(c.when)}` : 'depende de un componente apagado'}>apagado</Badge>
						{/if}
						<span class="flex-1"></span>
						<Button variant="ghost" size="icon-xs" title="subir" aria-label="subir" onclick={() => move(i, -1)}><ArrowUp /></Button>
						<Button variant="ghost" size="icon-xs" title="bajar" aria-label="bajar" onclick={() => move(i, 1)}><ArrowDown /></Button>
						<Button variant="danger" size="icon-xs" title="quitar" aria-label="quitar" onclick={() => remove(i)}><X /></Button>
					</div>
					{#if idError?.component === c.id}
						<p class="px-8 pb-1 text-2xs text-danger">{idError.message}</p>
					{/if}
					{#if open[c.id]}
						<div class="flex flex-col gap-0.5 border-t px-2.5 py-2">
							<label class={ROW}><span class={LBL}>condición</span>
								<Input size="xs" mono placeholder="siempre · ej. drawers > 0" value={show(c.when)}
									onchange={(e) => { const raw = e.currentTarget.value.trim(); const rec = c as { when?: NumOrExpr }; if (raw === '' ) delete rec.when; else rec.when = raw === 'true' ? true : raw === 'false' ? false : raw; app.touch(); }} />
							</label>
							{#if c.type === 'panel'}
								<label class={ROW}><span class={LBL}>x</span><Input size="xs" value={show(c.x ?? 0)} onchange={(e) => setField(c, 'x', e.currentTarget.value)} /></label>
								<div class={ROW}><span class={LBL}>mira hacia</span>
									<Select size="xs" aria-label="mira hacia" value={c.facing ?? 'right'} onChange={(v) => { c.facing = v; app.touch(); }}
										options={[{ value: 'right', label: 'la derecha (lateral izquierdo)' }, { value: 'left', label: 'la izquierda (lateral derecho)' }]} />
								</div>
								<label class={ROW}><span class={LBL}>profundidad</span><Input size="xs" value={show(c.depth ?? 'depth')} onchange={(e) => setField(c, 'depth', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>alto</span><Input size="xs" value={show(c.height ?? 'height')} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
							{:else if c.type === 'sliding_doors'}
								<label class={ROW}><span class={LBL}>carcasa</span><Input size="xs" placeholder="la única" value={c.carcass ?? ''} onchange={(e) => setText(c, 'carcass', e.currentTarget.value.trim(), true)} /></label>
								<label class={ROW}><span class={LBL}>puertas</span><Input size="xs" value={show(c.count)} onchange={(e) => setField(c, 'count', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>solape</span><Input size="xs" value={show(c.overlap ?? 30)} onchange={(e) => setField(c, 'overlap', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>luz a los lados</span><Input size="xs" value={show(c.gap ?? 2)} onchange={(e) => setField(c, 'gap', e.currentTarget.value)} /></label>
							{:else if c.type === 'sink'}
								<label class={ROW}><span class={LBL}>carcasa</span><Input size="xs" placeholder="la única" value={c.carcass ?? ''} onchange={(e) => setText(c, 'carcass', e.currentTarget.value.trim(), true)} /></label>
								<label class={ROW}><span class={LBL}>bahía</span><Input size="xs" placeholder="la única" value={show(c.bay)} onchange={(e) => setField(c, 'bay', e.currentTarget.value, true)} /></label>
								<div class={ROW}><span class={LBL}>bacha</span>
									<Select size="xs" aria-label="bacha" options={hwOpt(byKind(['sink']))} value={c.hardware[0] ?? ''} onChange={(v) => { c.hardware = [v]; app.touch(); }} />
								</div>
								<label class={ROW}><span class={LBL}>desde el frente</span><Input size="xs" placeholder="medio" value={show(c.fromFront)} onchange={(e) => setField(c, 'fromFront', e.currentTarget.value, true)} /></label>
								<div class={ROW}><span class={LBL}>pase de caños</span>
									<Checkbox aria-label="pase de caños" checked={!!c.passage} onChange={(on) => { if (on) c.passage = {}; else delete c.passage; app.touch(); }} />
								</div>
							{:else if c.type === 'modesty'}
								<label class={ROW}><span class={LBL}>tapa</span><Input size="xs" placeholder="la única" value={c.worktop ?? ''} onchange={(e) => setText(c, 'worktop', e.currentTarget.value.trim(), true)} /></label>
								<label class={ROW}><span class={LBL}>alto</span><Input size="xs" value={show(c.height ?? 300)} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>desde el fondo</span><Input size="xs" value={show(c.inset ?? 20)} onchange={(e) => setField(c, 'inset', e.currentTarget.value)} /></label>
							{/if}
							{#if c.type === 'carcass'}
								<label class={ROW}><span class={LBL}>ancho</span><Input size="xs" value={show(c.width ?? 'width')} onchange={(e) => setField(c, 'width', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>alto</span><Input size="xs" value={show(c.height ?? 'height')} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>profundidad</span><Input size="xs" value={show(c.depth ?? 'depth')} onchange={(e) => setField(c, 'depth', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>origen x,y,z</span>
									<Input size="xs" placeholder="0, 0, 0" value={c.origin ? [c.origin.x ?? 0, c.origin.y ?? 0, c.origin.z ?? 0].map(show).join(', ') : ''}
										onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) { const [x, y, z] = raw.split(',').map((v) => numOrExpr(v || '0')); c.origin = { ...(c.origin ?? {}), x: x ?? 0, y: y ?? 0, z: z ?? 0 }; } else if (c.origin?.rotation !== undefined) c.origin = { rotation: c.origin.rotation }; else delete c.origin; app.touch(); }} />
								</label>
								<div class={ROW} title="Alrededor del origen, visto desde arriba"><span class={LBL}>giro</span>
									<Select size="xs" aria-label="giro" value={String(c.origin?.rotation ?? 0)}
										onChange={(s) => { const v = Number(s); if (v) c.origin = { ...(c.origin ?? {}), rotation: v }; else if (c.origin) delete c.origin.rotation; app.touch(); }}
										options={[
											{ value: '0', label: 'sin girar (frente hacia adelante)' },
											{ value: '90', label: '90° (frente hacia la izquierda)' },
											{ value: '180', label: '180° (frente hacia atrás)' },
											{ value: '270', label: '270° (frente hacia la derecha)' }
										]} />
								</div>
								<label class={ROW} title="Lugar para el riel de puertas corredizas"><span class={LBL}>retiro divisores</span><Input size="xs" placeholder="0" value={show(c.dividerSetback)} onchange={(e) => setField(c, 'dividerSetback', e.currentTarget.value, true)} /></label>
								<label class={ROW}><span class={LBL}>bahías</span><Input size="xs" value={show(c.bays ?? 1)} onchange={(e) => setField(c, 'bays', e.currentTarget.value)} /></label>
								<label class={ROW}><span class={LBL}>anchos</span>
									<Input size="xs" placeholder="reparto parejo · ej. 500, auto, 400" value={(c.bayWidths ?? []).join(', ')}
										onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) c.bayWidths = raw.split(',').map((x) => numOrExpr(x)); else delete c.bayWidths; app.touch(); }} />
								</label>
								<div class={ROW}><span class={LBL}>fondo</span>
									<Select size="xs" aria-label="fondo" options={withNone('— sin fondo —', materialOptions)} value={c.back?.material ?? NONE}
										onChange={(s) => { const v = orNone(s); if (v) c.back = { ...(c.back ?? {}), material: v }; else delete c.back; app.touch(); }} />
								</div>
								<div class={ROW}><span class={LBL}>patas</span>
									<Select size="xs" aria-label="patas" options={withNone('— sin patas —', hwOpt(byKind(['leg'])))} value={c.legs ? (c.legs.hardware?.[0] ?? DEFAULT_LEG) : NONE}
										onChange={(s) => { const v = orNone(s); if (v) c.legs = { ...(c.legs ?? {}), hardware: [v] }; else delete c.legs; app.touch(); }} />
								</div>
								{#if c.legs}
									<label class={ROW}><span class={LBL}>retiro patas</span><Input size="xs" value={show(c.legs.inset ?? 50)} onchange={(e) => { c.legs!.inset = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
									<div class={ROW}><span class={LBL}>zócalo</span>
										<span class={INLINE}>
											<Checkbox aria-label="zócalo" checked={!!c.legs.plinth} onChange={(on) => { if (on) c.legs!.plinth = { setback: 40 }; else delete c.legs!.plinth; app.touch(); }} />
											{#if c.legs.plinth}
												<span class="text-xs text-muted-foreground">retiro</span>
												<Input size="xs" class="w-16" aria-label="retiro del zócalo" value={show(c.legs.plinth.setback ?? 40)} onchange={(e) => { c.legs!.plinth!.setback = numOrExpr(e.currentTarget.value); app.touch(); }} />
											{/if}
										</span>
									</div>
								{/if}
							{:else if c.type === 'shelves' || c.type === 'doors' || c.type === 'drawers' || c.type === 'rail'}
								<label class={ROW}><span class={LBL}>bahía</span><Input size="xs" placeholder="todas" value={show(c.bay)} onchange={(e) => setField(c, 'bay', e.currentTarget.value, true)} /></label>
								<label class={ROW}><span class={LBL}>hasta bahía</span><Input size="xs" placeholder="sólo esa" value={show(c.lastBay)} onchange={(e) => setField(c, 'lastBay', e.currentTarget.value, true)} /></label>
								<div class={ROW}><span class={LBL}>zona</span>
									<span class={INLINE}>
										<Checkbox aria-label="zona" checked={!!c.zone} onChange={(on) => setZone(c, on)} />
										{#if c.zone}
											<Input size="xs" class="w-16" aria-label="zona desde" value={show(c.zone.from)} onchange={(e) => { c.zone!.from = numOrExpr(e.currentTarget.value); app.touch(); }} />
											<span class="text-subtle-foreground">–</span>
											<Input size="xs" class="w-16" aria-label="zona hasta" value={show(c.zone.to)} onchange={(e) => { c.zone!.to = numOrExpr(e.currentTarget.value); app.touch(); }} />
										{/if}
									</span>
								</div>
								{#if c.type === 'rail'}
									<label class={ROW}><span class={LBL}>bajo la tapa</span><Input size="xs" value={show(c.fromTop ?? 60)} onchange={(e) => setField(c, 'fromTop', e.currentTarget.value)} /></label>
									<div class={ROW}><span class={LBL}>barral</span>
										<Select size="xs" aria-label="barral" options={hwOpt(byKind(['rail']))} value={c.hardware?.[0] ?? 'rail_oval_30'} onChange={(v) => { c.hardware = [v]; app.touch(); }} />
									</div>
									<div class={ROW}><span class={LBL}>soportes</span>
										<Select size="xs" aria-label="soportes" options={hwOpt(byKind(['rail_support']))} value={c.supports?.[0] ?? 'rail_support_oval'} onChange={(v) => { c.supports = [v]; app.touch(); }} />
									</div>
								{:else if c.type !== 'shelves' || !c.positions?.length}
									<label class={ROW}><span class={LBL}>cantidad</span><Input size="xs" value={show(c.count)} onchange={(e) => setField(c, 'count', e.currentTarget.value, c.type === 'shelves')} /></label>
								{/if}
								{#if c.type === 'doors'}
									<label class={ROW}><span class={LBL}>bahías que cubre</span><Input size="xs" placeholder="1" value={show(c.span)} onchange={(e) => setField(c, 'span', e.currentTarget.value, true)} /></label>
								{/if}
								{#if c.type === 'doors' || c.type === 'drawers'}
									<div class={ROW}><span class={LBL}>montaje</span>
										<Select size="xs" aria-label="montaje" value={c.mount ?? 'overlay'}
											onChange={(v) => { if (v === 'overlay') delete c.mount; else c.mount = 'inset'; app.touch(); }}
											options={[
												{ value: 'overlay', label: 'superpuesto (cubre la carcasa)' },
												{ value: 'inset', label: c.type === 'doors' ? 'embutido (dentro del hueco)' : 'interior (detrás de una puerta)' }
											]} />
									</div>
									{#if c.type === 'doors'}
										<div class={ROW} title="Con una puerta por bahía; con dos, cada una cuelga de su lado"><span class={LBL}>bisagra</span>
											<Select size="xs" aria-label="lado de la bisagra" value={c.hingeSide ?? 'auto'}
												onChange={(v) => { if (v === 'auto') delete c.hingeSide; else c.hingeSide = v; app.touch(); }}
												options={[
													{ value: 'auto', label: 'automática (hacia afuera del mueble)' },
													{ value: 'left', label: 'a la izquierda' },
													{ value: 'right', label: 'a la derecha' }
												]} />
										</div>
									{/if}
									{#if c.type === 'doors'}
										<div class={ROW} title="De costado, hacia arriba (basculante, colgada de la tapa) o hacia abajo (rebatible, de la base)"><span class={LBL}>abre</span>
											<Select size="xs" aria-label="abre" value={c.opening ?? 'side'}
												onChange={(v) => { if (v === 'side') delete c.opening; else { c.opening = v; c.count = 1; delete c.hingeSide; delete c.catch; } app.touch(); }}
												options={[
													{ value: 'side', label: 'de costado' },
													{ value: 'up', label: 'hacia arriba (basculante)' },
													{ value: 'down', label: 'hacia abajo (rebatible)' }
												]} />
										</div>
										<div class={ROW} title="Un espejo pegado sobre el frente de cada puerta"><span class={LBL}>espejo</span>
											<Checkbox aria-label="espejo" checked={!!c.facing} onChange={(on) => { if (on) { c.facing = {}; delete c.handle; } else delete c.facing; app.touch(); }} />
										</div>
									{/if}
									{#if c.type === 'drawers' && c.mount === 'inset'}
										<label class={ROW}><span class={LBL}>retranqueo</span><Input size="xs" placeholder="0" value={show(c.setback)} onchange={(e) => setField(c, 'setback', e.currentTarget.value, true)} /></label>
									{/if}
								{/if}
							{/if}
							{#if c.type === 'shelves'}
								<div class={ROW}><span class={LBL}>apoyo</span>
									<Select size="xs" aria-label="apoyo" value={c.support ?? 'joint'}
										onChange={(v) => { if (v === 'pins') { c.support = 'pins'; } else { delete c.support; c.joint ??= { hardware: ['dowel_8x30'] }; } app.touch(); }}
										options={[
											{ value: 'joint', label: 'unido con herrajes' },
											{ value: 'pins', label: 'regulable con soportes (Sistema 32)' }
										]} />
								</div>
								<label class={ROW}><span class={LBL}>fijos en</span>
									<Input size="xs" placeholder="alturas · ej. 1000, divider_z" value={(c.positions ?? []).map(show).join(', ')}
										onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) { c.positions = raw.split(',').map((x) => numOrExpr(x)); delete c.count; } else { delete c.positions; c.count ??= 2; } app.touch(); }} />
								</label>
								<label class={ROW}><span class={LBL}>retranqueo</span><Input size="xs" value={show(c.setback ?? (c.positions?.length ? 0 : 20))} onchange={(e) => setField(c, 'setback', e.currentTarget.value)} /></label>
							{/if}
							{#if c.type === 'doors' || c.type === 'drawers'}
								<label class={ROW}><span class={LBL}>luz</span><Input size="xs" value={show(c.gap ?? 2)} onchange={(e) => setField(c, 'gap', e.currentTarget.value)} /></label>
							{/if}
							{#if c.type === 'drawers'}
								<label class={ROW}><span class={LBL}>alto frente</span><Input size="xs" placeholder="reparto" value={show(c.frontHeight)} onchange={(e) => setField(c, 'frontHeight', e.currentTarget.value, true)} /></label>
								<label class={ROW}><span class={LBL}>alto caja</span><Input size="xs" placeholder="frente − 40" value={show(c.boxHeight)} onchange={(e) => setField(c, 'boxHeight', e.currentTarget.value, true)} /></label>
								<div class={ROW}><span class={LBL}>corredera</span>
									<Select size="xs" aria-label="corredera" options={hwOpt(plainSlides)} value={c.slide.hardware[0] ?? ''} onChange={(v) => { c.slide.hardware = [v]; app.touch(); }} />
								</div>
								<div class={ROW}><span class={LBL}>cierre suave</span>
									<Checkbox aria-label="cierre suave" checked={!!c.softClose} onChange={(on) => { if (on) c.softClose = true; else delete c.softClose; app.touch(); }} />
								</div>
							{/if}
							{#if c.type === 'worktop'}
								<label class={ROW}><span class={LBL}>vuelo frente</span><Input size="xs" value={show(c.overhang?.front ?? 20)} onchange={(e) => { c.overhang ??= {}; c.overhang.front = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
								<label class={ROW}><span class={LBL}>vuelo lados</span><Input size="xs" value={show(c.overhang?.sides ?? 0)} onchange={(e) => { c.overhang ??= {}; c.overhang.sides = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
								<label class={ROW}><span class={LBL}>sobre</span><Input size="xs" placeholder="todas las carcasas" value={(c.carcasses ?? []).join(', ')} onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) c.carcasses = raw.split(',').map((x) => x.trim()); else delete c.carcasses; app.touch(); }} /></label>
							{/if}
							{#if c.type === 'carcass'}
								<div class={ROW}><span class={LBL}>colgada</span>
									<Checkbox aria-label="colgada" checked={!!c.hanging} onChange={(on) => { if (on) { c.hanging = {}; delete c.legs; } else delete c.hanging; app.touch(); }} />
								</div>
							{/if}
							{#if c.type !== 'rail' && c.type !== 'sink'}
								<div class={ROW}><span class={LBL}>material</span>
									<Select size="xs" aria-label="material del componente" options={withNone('— el del mueble —', materialOptions)} value={c.material ?? NONE}
										onChange={(v) => setText(c, 'material', orNone(v), true)} />
								</div>
								<div class={ROW}><span class={LBL}>cantos</span>
									<Select size="xs" aria-label="cantos" value={c.edges ?? 'default'} onChange={(v) => setText(c, 'edges', v)}
										options={[
											{ value: 'default', label: 'por defecto' },
											{ value: 'none', label: 'ninguno' },
											{ value: 'front', label: 'frente' },
											{ value: 'all', label: 'todos' }
										]} />
								</div>
							{/if}
							{#if c.type !== 'doors' && c.type !== 'rail' && c.type !== 'worktop' && c.type !== 'sliding_doors' && c.type !== 'sink' && !(c.type === 'shelves' && c.support === 'pins')}
								{@const hw = c.joint?.hardware ?? []}
								<div class="grid grid-cols-[104px_minmax(0,1fr)] items-start gap-2 py-0.5"><span class="{LBL} pt-1">herrajes</span>
									<span class="flex flex-wrap gap-1">
										{#each fasteners as h (h.id)}
											<Toggle size="xs" variant="secondary" class="max-w-full min-w-0" title={h.name} selected={hw.includes(h.id)} onChange={() => toggleHardware(c, h.id)}>
												<span class="truncate">{h.name}</span>
											</Toggle>
										{/each}
									</span>
								</div>
							{/if}
							{#if c.type === 'doors'}
								<div class={ROW}><span class={LBL}>bisagra</span>
									<span class={INLINE}>
										<Checkbox aria-label="con bisagra" checked={c.hinge !== null} onChange={(on) => setOptionalJoint(c, 'hinge', on, 'hinge_35_overlay')} />
										{#if c.hinge !== null}
											<Select size="xs" class="min-w-0 flex-1" aria-label="bisagra" options={hwOpt(hingeFamilies)} value={c.hinge?.hardware[0] ?? 'hinge_35_overlay'}
												onChange={(v) => { c.hinge = { hardware: [v] }; app.touch(); }} />
										{/if}
									</span>
								</div>
								{#if c.hinge !== null}
									<div class={ROW}><span></span>
										<Checkbox checked={!!c.softClose} onChange={(on) => { if (on) c.softClose = true; else delete c.softClose; app.touch(); }}>
											<span class="text-xs">cierre suave</span>
										</Checkbox>
									</div>
								{/if}
								<div class={ROW}><span class={LBL}>cierre</span>
									<Select size="xs" aria-label="cierre" options={withNone('— ninguno —', hwOpt(catches))} value={c.catch?.hardware?.[0] ?? NONE}
										onChange={(s) => { const v = orNone(s); if (v) c.catch = { hardware: [v] }; else delete c.catch; app.touch(); }} />
								</div>
							{/if}
							{#if c.type === 'doors' || c.type === 'drawers'}
								<div class={ROW}><span class={LBL}>tirador</span>
									<span class={INLINE}>
										<Checkbox aria-label="con tirador" checked={!!c.handle} onChange={(on) => setOptionalJoint(c, 'handle', on, 'handle_bar_128')} />
										{#if c.handle}
											<Select size="xs" class="min-w-0 flex-1" aria-label="tirador" options={hwOpt(byKind(['handle']))} value={c.handle.hardware[0]}
												onChange={(v) => { c.handle!.hardware = [v]; app.touch(); }} />
										{/if}
									</span>
								</div>
							{/if}
						</div>
						{#if findings.length}
							<div class="flex flex-col gap-1 px-2.5 pb-2.5">
								{#each findings as d (d.code + (d.entity ?? '') + (d.location ?? '') + d.message)}
									<div class="rounded-md px-2 py-1.5 text-xs {WELL[d.severity]}">
										<div class="flex items-start gap-1.5">
											<Badge tone={TONE[d.severity]} class="bg-depth-0/70">{d.severity}</Badge>
											<span class="pt-0.5 font-mono text-2xs text-muted-foreground">{d.code}</span>
										</div>
										<p class="mt-1 text-foreground">{d.message}</p>
										{#if d.suggestion}<p class="mt-0.5 text-muted-foreground">{d.suggestion}</p>{/if}
										{#if d.fix}<Button size="xs" class="mt-1.5" onclick={() => app.applyFix(d.fix!)}>{d.fix.label}</Button>{/if}
									</div>
								{/each}
							</div>
						{/if}
					{/if}
				</div>
			{/each}
			<div class="mt-1 flex gap-1.5">
				<Select size="sm" class="flex-1" aria-label="tipo de componente" options={NEW_TYPES} bind:value={newType} />
				<Button variant="soft" onclick={addComponent}><Plus />Agregar</Button>
			</div>
		</div>
	</Section>

	<Section title="Restricciones" count={app.spec.constraints?.length ?? 0}>
		<div class="flex flex-col gap-1.5">
			{#each app.spec.constraints ?? [] as k, i (i)}
				{@const finding = findingOf(k.id)}
				<div class="rounded-md border bg-depth-0 {finding ? 'border-danger/60' : ''}">
					<div class="flex items-center gap-1.5 px-1.5 py-1">
						<Input size="xs" mono class="min-w-0 flex-1" aria-label="id de la restricción" value={k.id} onchange={(e) => { k.id = e.currentTarget.value; app.touch(); }} />
						<Select size="xs" class="w-28" aria-label="severidad" options={SEVERITIES} value={k.severity ?? 'ERROR'} onChange={(v) => { k.severity = v; app.touch(); }} />
						<Button variant="danger" size="icon-xs" title="quitar" aria-label="quitar" onclick={() => removeConstraint(i)}><X /></Button>
					</div>
					<div class="flex flex-col gap-0.5 border-t px-2.5 py-2">
						<label class={ROW}><span class={LBL}>expresión</span><Input size="xs" mono value={k.expr} onchange={(e) => { k.expr = e.currentTarget.value; app.touch(); }} /></label>
						<label class={ROW}><span class={LBL}>mensaje</span><Input size="xs" value={k.message ?? ''} onchange={(e) => { const v = e.currentTarget.value; if (v) k.message = v; else delete k.message; app.touch(); }} /></label>
						{#if finding}
							<p class="mt-1 rounded-md px-2 py-1.5 text-xs {WELL[finding.severity]}">
								<Badge tone={TONE[finding.severity]} class="mr-1 bg-depth-0/70">{finding.severity}</Badge>{finding.message}
							</p>
						{/if}
					</div>
				</div>
			{/each}
			<div><Button variant="soft" onclick={addConstraint}><Plus />Restricción</Button></div>
		</div>
	</Section>

	<Section title="Sólo para este mueble" count={`${overrideCount} cambio${overrideCount === 1 ? '' : 's'}`} bind:open={libOpen}>
		<p class="mb-2 text-xs leading-relaxed text-muted-foreground">
			Los cambios que valen para todos los muebles van en la pestaña Biblioteca. Acá, en JSON, los de este mueble solo, por encima de esos:
			<code class="font-mono">materials</code>, <code class="font-mono">edgeMaterials</code>, <code class="font-mono">hardware</code> (listas, por id) y
			<code class="font-mono">profile</code>. Vacío = ninguno.
		</p>
		<Textarea mono class="min-h-36" spellcheck={false} aria-label="cambios de biblioteca de este mueble" value={libText}
			onchange={(e) => applyLibraries(e.currentTarget.value)} placeholder={'{ "hardware": [ { "id": "minifix_15", ... } ] }'} />
		{#if libError}<p class="mt-1 rounded-md bg-danger-soft px-2 py-1 text-xs text-danger">JSON inválido: {libError}</p>{/if}
	</Section>
</div>
