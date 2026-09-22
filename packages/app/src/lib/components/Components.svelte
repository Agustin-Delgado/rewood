<script lang="ts">
	/**
	 * Form editor for the spec's components. Every edit mutates `app.spec`
	 * in place and recompiles; the JSON tab shows the same thing. Options
	 * come from the engine's libraries, so nothing here is typed by hand.
	 */
	import type { ComponentSpec, HandleSpec, JointSpec, LibraryOverrides, NumOrExpr, ZoneSpec } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';

	const libs = $derived(app.libraries);
	const hardware = $derived(Object.values(libs?.hardware.items ?? {}));
	const materials = $derived(Object.values(libs?.materials.materials ?? {}));
	const edgeMaterials = $derived(Object.values(libs?.materials.edgeMaterials ?? {}));
	const byKind = (kinds: string[]) => hardware.filter((h) => kinds.includes(h.kind));
	const fasteners = $derived(
		hardware.filter((h) => !['hinge', 'slide', 'handle', 'leg', 'clip', 'rail_support', 'rail', 'hanger', 'pin_row', 'pin', 'catch', 'strike'].includes(h.kind))
	);
	// Base variants: the engine swaps the arm and the damper in itself.
	const hingeFamilies = $derived(byKind(['hinge']).filter((h) => !h.hinge?.softClose && h.hinge?.mount !== 'half_overlay' && h.hinge?.mount !== 'inset'));
	const plainSlides = $derived(byKind(['slide']).filter((h) => !h.slide?.softClose));
	const catches = $derived(byKind(['catch']));

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
	function addComponent() {
		const n = app.spec.components.filter((c) => c.type === newType).length + 1;
		const id = n === 1 ? newType : `${newType}_${n}`;
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
		modesty: 'faldón'
	};
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
	function worst(list: { severity: keyof typeof RANK }[]) {
		return list.reduce<keyof typeof RANK | null>((w, d) => (w === null || RANK[d.severity] > RANK[w] ? d.severity : w), null);
	}
</script>

<div class="comps">
	<h3>Mueble</h3>
	<label class="row"><span>id</span><input value={app.spec.id} onchange={(e) => { app.spec.id = e.currentTarget.value; app.touch(); }} /></label>
	<label class="row"><span>nombre</span><input value={app.spec.name} onchange={(e) => { app.spec.name = e.currentTarget.value; app.touch(); }} /></label>
	<label class="row"><span>material</span>
		<select value={app.spec.material} onchange={(e) => { app.spec.material = e.currentTarget.value; app.touch(); }}>
			{#each materials as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
		</select>
	</label>
	<label class="row"><span>canto</span>
		<select value={app.spec.edgeMaterial ?? ''} onchange={(e) => { app.spec.edgeMaterial = e.currentTarget.value || undefined; app.touch(); }}>
			<option value="">— sin canto —</option>
			{#each edgeMaterials as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
		</select>
	</label>

	<h3>Componentes</h3>
	{#each app.spec.components as c, i (c.id)}
		{@const findings = app.findingsFor(c.id)}
		{@const level = worst(findings)}
		<div class="card" class:failing={level === 'ERROR' || level === 'FATAL'} class:warning={level === 'WARNING'} class:off={inactive.has(c.id)} bind:this={cards[c.id]}>
			<div class="head">
				<button class="fold" onclick={() => (open[c.id] = !open[c.id])}>{open[c.id] ? '▾' : '▸'}</button>
				<span class="type">{TYPE_ES[c.type]}</span>
				<input class="id" value={c.id} onchange={(e) => setText(c, 'id', e.currentTarget.value)} />
				{#if level}<button class="badge {level}" title="ver hallazgos" onclick={() => (open[c.id] = true)}>{findings.length}</button>{/if}
				{#if inactive.has(c.id)}<span class="offtag" title={c.when !== undefined ? `condición: ${String(c.when)}` : 'depende de un componente apagado'}>apagado</span>{/if}
				<span class="spacer"></span>
				<button title="subir" onclick={() => move(i, -1)}>↑</button>
				<button title="bajar" onclick={() => move(i, 1)}>↓</button>
				<button title="quitar" onclick={() => remove(i)}>✕</button>
			</div>
			{#if open[c.id]}
				<div class="fields">
					<label class="row"><span>condición</span>
						<input class="mono" placeholder="siempre · ej. drawers > 0" value={show(c.when)}
							onchange={(e) => { const raw = e.currentTarget.value.trim(); const rec = c as { when?: NumOrExpr }; if (raw === '' ) delete rec.when; else rec.when = raw === 'true' ? true : raw === 'false' ? false : raw; app.touch(); }} />
					</label>
					{#if c.type === 'panel'}
						<label class="row"><span>x</span><input value={show(c.x ?? 0)} onchange={(e) => setField(c, 'x', e.currentTarget.value)} /></label>
						<label class="row"><span>mira hacia</span>
							<select value={c.facing ?? 'right'} onchange={(e) => { c.facing = e.currentTarget.value as 'right' | 'left'; app.touch(); }}>
								<option value="right">la derecha (lateral izquierdo)</option>
								<option value="left">la izquierda (lateral derecho)</option>
							</select>
						</label>
						<label class="row"><span>profundidad</span><input value={show(c.depth ?? 'depth')} onchange={(e) => setField(c, 'depth', e.currentTarget.value)} /></label>
						<label class="row"><span>alto</span><input value={show(c.height ?? 'height')} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
					{:else if c.type === 'modesty'}
						<label class="row"><span>tapa</span><input placeholder="la única" value={c.worktop ?? ''} onchange={(e) => setText(c, 'worktop', e.currentTarget.value.trim(), true)} /></label>
						<label class="row"><span>alto</span><input value={show(c.height ?? 300)} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
						<label class="row"><span>desde el fondo</span><input value={show(c.inset ?? 20)} onchange={(e) => setField(c, 'inset', e.currentTarget.value)} /></label>
					{/if}
					{#if c.type === 'carcass'}
						<label class="row"><span>ancho</span><input value={show(c.width ?? 'width')} onchange={(e) => setField(c, 'width', e.currentTarget.value)} /></label>
						<label class="row"><span>alto</span><input value={show(c.height ?? 'height')} onchange={(e) => setField(c, 'height', e.currentTarget.value)} /></label>
						<label class="row"><span>profundidad</span><input value={show(c.depth ?? 'depth')} onchange={(e) => setField(c, 'depth', e.currentTarget.value)} /></label>
						<label class="row"><span>origen x,y,z</span>
							<input placeholder="0, 0, 0" value={c.origin ? [c.origin.x ?? 0, c.origin.y ?? 0, c.origin.z ?? 0].map(show).join(', ') : ''}
								onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) { const [x, y, z] = raw.split(',').map((v) => numOrExpr(v || '0')); c.origin = { x: x ?? 0, y: y ?? 0, z: z ?? 0 }; } else delete c.origin; app.touch(); }} />
						</label>
						<label class="row"><span>bahías</span><input value={show(c.bays ?? 1)} onchange={(e) => setField(c, 'bays', e.currentTarget.value)} /></label>
						<label class="row"><span>anchos</span>
							<input placeholder="reparto parejo · ej. 500, auto, 400" value={(c.bayWidths ?? []).join(', ')}
								onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) c.bayWidths = raw.split(',').map((x) => numOrExpr(x)); else delete c.bayWidths; app.touch(); }} />
						</label>
						<label class="row"><span>fondo</span>
							<select value={c.back?.material ?? ''} onchange={(e) => { const v = e.currentTarget.value; if (v) c.back = { ...(c.back ?? {}), material: v }; else delete c.back; app.touch(); }}>
								<option value="">— sin fondo —</option>
								{#each materials as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
							</select>
						</label>
						<label class="row"><span>patas</span>
							<select value={c.legs?.hardware?.[0] ?? ''} onchange={(e) => { const v = e.currentTarget.value; if (v) c.legs = { ...(c.legs ?? {}), hardware: [v] }; else delete c.legs; app.touch(); }}>
								<option value="">— sin patas —</option>
								{#each byKind(['leg']) as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
							</select>
						</label>
						{#if c.legs}
							<label class="row"><span>retiro patas</span><input value={show(c.legs.inset ?? 50)} onchange={(e) => { c.legs!.inset = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
							<label class="row"><span>zócalo</span>
								<span class="inline">
									<input type="checkbox" checked={!!c.legs.plinth} onchange={(e) => { if (e.currentTarget.checked) c.legs!.plinth = { setback: 40 }; else delete c.legs!.plinth; app.touch(); }} />
									{#if c.legs.plinth}
										retiro <input class="short" value={show(c.legs.plinth.setback ?? 40)} onchange={(e) => { c.legs!.plinth!.setback = numOrExpr(e.currentTarget.value); app.touch(); }} />
									{/if}
								</span>
							</label>
						{/if}
					{:else if c.type === 'shelves' || c.type === 'doors' || c.type === 'drawers' || c.type === 'rail'}
						<label class="row"><span>bahía</span><input placeholder="todas" value={show(c.bay)} onchange={(e) => setField(c, 'bay', e.currentTarget.value, true)} /></label>
						<label class="row"><span>hasta bahía</span><input placeholder="sólo esa" value={show(c.lastBay)} onchange={(e) => setField(c, 'lastBay', e.currentTarget.value, true)} /></label>
						<label class="row"><span>zona</span>
							<span class="inline">
								<input type="checkbox" checked={!!c.zone} onchange={(e) => setZone(c, e.currentTarget.checked)} />
								{#if c.zone}
									<input class="short" value={show(c.zone.from)} onchange={(e) => { c.zone!.from = numOrExpr(e.currentTarget.value); app.touch(); }} />
									–
									<input class="short" value={show(c.zone.to)} onchange={(e) => { c.zone!.to = numOrExpr(e.currentTarget.value); app.touch(); }} />
								{/if}
							</span>
						</label>
						{#if c.type === 'rail'}
							<label class="row"><span>bajo la tapa</span><input value={show(c.fromTop ?? 60)} onchange={(e) => setField(c, 'fromTop', e.currentTarget.value)} /></label>
							<div class="row"><span>barral</span>
								<select value={c.hardware?.[0] ?? 'rail_oval_30'} onchange={(e) => { c.hardware = [e.currentTarget.value]; app.touch(); }}>
									{#each byKind(['rail']) as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
								</select>
							</div>
							<div class="row"><span>soportes</span>
								<select value={c.supports?.[0] ?? 'rail_support_oval'} onchange={(e) => { c.supports = [e.currentTarget.value]; app.touch(); }}>
									{#each byKind(['rail_support']) as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
								</select>
							</div>
						{:else if c.type !== 'shelves' || !c.positions?.length}
							<label class="row"><span>cantidad</span><input value={show(c.count)} onchange={(e) => setField(c, 'count', e.currentTarget.value, c.type === 'shelves')} /></label>
						{/if}
						{#if c.type === 'doors'}
							<label class="row"><span>bahías que cubre</span><input placeholder="1" value={show(c.span)} onchange={(e) => setField(c, 'span', e.currentTarget.value, true)} /></label>
						{/if}
						{#if c.type === 'doors' || c.type === 'drawers'}
							<label class="row"><span>montaje</span>
								<select value={c.mount ?? 'overlay'} onchange={(e) => { const v = e.currentTarget.value; if (v === 'overlay') delete c.mount; else c.mount = 'inset'; app.touch(); }}>
									<option value="overlay">superpuesto (cubre la carcasa)</option>
									<option value="inset">{c.type === 'doors' ? 'embutido (dentro del hueco)' : 'interior (detrás de una puerta)'}</option>
								</select>
							</label>
							{#if c.type === 'drawers' && c.mount === 'inset'}
								<label class="row"><span>retranqueo</span><input placeholder="0" value={show(c.setback)} onchange={(e) => setField(c, 'setback', e.currentTarget.value, true)} /></label>
							{/if}
						{/if}
					{/if}
					{#if c.type === 'shelves'}
						<label class="row"><span>apoyo</span>
							<select value={c.support ?? 'joint'} onchange={(e) => { const v = e.currentTarget.value; if (v === 'pins') { c.support = 'pins'; } else { delete c.support; c.joint ??= { hardware: ['dowel_8x30'] }; } app.touch(); }}>
								<option value="joint">unido con herrajes</option>
								<option value="pins">regulable con soportes (Sistema 32)</option>
							</select>
						</label>
						<label class="row"><span>fijos en</span>
							<input placeholder="alturas · ej. 1000, divider_z" value={(c.positions ?? []).map(show).join(', ')}
								onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) { c.positions = raw.split(',').map((x) => numOrExpr(x)); delete c.count; } else { delete c.positions; c.count ??= 2; } app.touch(); }} />
						</label>
						<label class="row"><span>retranqueo</span><input value={show(c.setback ?? (c.positions?.length ? 0 : 20))} onchange={(e) => setField(c, 'setback', e.currentTarget.value)} /></label>
					{/if}
					{#if c.type === 'doors' || c.type === 'drawers'}
						<label class="row"><span>luz</span><input value={show(c.gap ?? 2)} onchange={(e) => setField(c, 'gap', e.currentTarget.value)} /></label>
					{/if}
					{#if c.type === 'drawers'}
						<label class="row"><span>alto frente</span><input placeholder="reparto" value={show(c.frontHeight)} onchange={(e) => setField(c, 'frontHeight', e.currentTarget.value, true)} /></label>
						<label class="row"><span>alto caja</span><input placeholder="frente − 40" value={show(c.boxHeight)} onchange={(e) => setField(c, 'boxHeight', e.currentTarget.value, true)} /></label>
						<div class="row"><span>corredera</span>
							<select value={c.slide.hardware[0] ?? ''} onchange={(e) => { c.slide.hardware = [e.currentTarget.value]; app.touch(); }}>
								{#each plainSlides as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
							</select>
						</div>
						<label class="row"><span>cierre suave</span><input type="checkbox" checked={!!c.softClose} onchange={(e) => { if (e.currentTarget.checked) c.softClose = true; else delete c.softClose; app.touch(); }} /></label>
					{/if}
					{#if c.type === 'worktop'}
						<label class="row"><span>vuelo frente</span><input value={show(c.overhang?.front ?? 20)} onchange={(e) => { c.overhang ??= {}; c.overhang.front = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
						<label class="row"><span>vuelo lados</span><input value={show(c.overhang?.sides ?? 0)} onchange={(e) => { c.overhang ??= {}; c.overhang.sides = numOrExpr(e.currentTarget.value); app.touch(); }} /></label>
						<label class="row"><span>sobre</span><input placeholder="todas las carcasas" value={(c.carcasses ?? []).join(', ')} onchange={(e) => { const raw = e.currentTarget.value.trim(); if (raw) c.carcasses = raw.split(',').map((x) => x.trim()); else delete c.carcasses; app.touch(); }} /></label>
					{/if}
					{#if c.type === 'carcass'}
						<label class="row"><span>colgada</span><input type="checkbox" checked={!!c.hanging} onchange={(e) => { if (e.currentTarget.checked) { c.hanging = {}; delete c.legs; } else delete c.hanging; app.touch(); }} /></label>
					{/if}
					{#if c.type !== 'rail'}
					<label class="row"><span>material</span>
						<select value={c.material ?? ''} onchange={(e) => setText(c, 'material', e.currentTarget.value, true)}>
							<option value="">— el del mueble —</option>
							{#each materials as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
						</select>
					</label>
					<label class="row"><span>cantos</span>
						<select value={c.edges ?? 'default'} onchange={(e) => setText(c, 'edges', e.currentTarget.value)}>
							<option value="default">por defecto</option><option value="none">ninguno</option><option value="front">frente</option><option value="all">todos</option>
						</select>
					</label>
					{/if}
					{#if c.type !== 'doors' && c.type !== 'rail' && c.type !== 'worktop' && !(c.type === 'shelves' && c.support === 'pins')}
						{@const hw = c.joint?.hardware ?? []}
						<div class="row"><span>herrajes</span>
							<span class="chips">
								{#each fasteners as h (h.id)}
									<label class="chip" class:on={hw.includes(h.id)}>
										<input type="checkbox" checked={hw.includes(h.id)} onchange={() => toggleHardware(c, h.id)} />{h.name}
									</label>
								{/each}
							</span>
						</div>
					{/if}
					{#if c.type === 'doors'}
						<div class="row"><span>bisagra</span>
							<span class="inline">
								<input type="checkbox" checked={c.hinge !== null} onchange={(e) => setOptionalJoint(c, 'hinge', e.currentTarget.checked, 'hinge_35_overlay')} />
								{#if c.hinge !== null}
									<select value={c.hinge?.hardware[0] ?? 'hinge_35_overlay'} onchange={(e) => { c.hinge = { hardware: [e.currentTarget.value] }; app.touch(); }}>
										{#each hingeFamilies as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
									</select>
									<label class="inline"><input type="checkbox" checked={!!c.softClose} onchange={(e) => { if (e.currentTarget.checked) c.softClose = true; else delete c.softClose; app.touch(); }} /> cierre suave</label>
								{/if}
							</span>
						</div>
						<div class="row"><span>cierre</span>
							<select value={c.catch?.hardware?.[0] ?? ''} onchange={(e) => { const v = e.currentTarget.value; if (v) c.catch = { hardware: [v] }; else delete c.catch; app.touch(); }}>
								<option value="">— ninguno —</option>
								{#each catches as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
							</select>
						</div>
					{/if}
					{#if c.type === 'doors' || c.type === 'drawers'}
						<div class="row"><span>tirador</span>
							<span class="inline">
								<input type="checkbox" checked={!!c.handle} onchange={(e) => setOptionalJoint(c, 'handle', e.currentTarget.checked, 'handle_bar_128')} />
								{#if c.handle}
									<select value={c.handle.hardware[0]} onchange={(e) => { c.handle!.hardware = [e.currentTarget.value]; app.touch(); }}>
										{#each byKind(['handle']) as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
									</select>
								{/if}
							</span>
						</div>
					{/if}
				</div>
				{#each findings as d (d.code + (d.entity ?? '') + (d.location ?? '') + d.message)}
					<div class="finding {d.severity}">
						<b>{d.severity} {d.code}</b> {d.message}
						{#if d.suggestion}<i>{d.suggestion}</i>{/if}
						{#if d.fix}<button class="fix" onclick={() => app.applyFix(d.fix!)}>{d.fix.label}</button>{/if}
					</div>
				{/each}
			{/if}
		</div>
	{/each}
	<div class="add">
		<select bind:value={newType}>
			<option value="carcass">carcasa</option><option value="shelves">estantes</option><option value="doors">puertas</option><option value="drawers">cajones</option><option value="rail">barral</option><option value="worktop">tapa de trabajo</option><option value="panel">lateral de apoyo</option><option value="modesty">faldón</option>
		</select>
		<button onclick={addComponent}>+ agregar</button>
	</div>

	<h3>Restricciones</h3>
	{#each app.spec.constraints ?? [] as k, i (i)}
		{@const finding = findingOf(k.id)}
		<div class="card" class:failing={!!finding}>
			<div class="head">
				<input class="id" value={k.id} onchange={(e) => { k.id = e.currentTarget.value; app.touch(); }} />
				<select value={k.severity ?? 'ERROR'} onchange={(e) => { k.severity = e.currentTarget.value as typeof k.severity; app.touch(); }}>
					<option>INFO</option><option>WARNING</option><option>ERROR</option><option>FATAL</option>
				</select>
				<span class="spacer"></span>
				<button title="quitar" onclick={() => removeConstraint(i)}>✕</button>
			</div>
			<div class="fields">
				<label class="row"><span>expresión</span><input class="mono" value={k.expr} onchange={(e) => { k.expr = e.currentTarget.value; app.touch(); }} /></label>
				<label class="row"><span>mensaje</span><input value={k.message ?? ''} onchange={(e) => { const v = e.currentTarget.value; if (v) k.message = v; else delete k.message; app.touch(); }} /></label>
				{#if finding}<div class="finding">{finding.severity}: {finding.message}</div>{/if}
			</div>
		</div>
	{/each}
	<div class="add"><button onclick={addConstraint}>+ restricción</button></div>

	<h3>
		<button class="fold" onclick={() => (libOpen = !libOpen)}>{libOpen ? '▾' : '▸'}</button>
		Biblioteca ({overrideCount} sobreescritura{overrideCount === 1 ? '' : 's'})
	</h3>
	{#if libOpen}
		<p class="hint">
			`materials`, `edgeMaterials`, `hardware` (listas, por id) y `profile`: reemplazan o agregan sin tocar el motor. JSON; vacío = sin sobreescrituras.
		</p>
		<textarea class="lib" spellcheck="false" value={libText} onchange={(e) => applyLibraries(e.currentTarget.value)}
			placeholder={'{ "hardware": [ { "id": "minifix_15", ... } ] }'}></textarea>
		{#if libError}<div class="finding">JSON inválido: {libError}</div>{/if}
	{/if}
</div>

<style>
	.comps {
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
	.card.failing {
		border-color: #c40;
	}
	.card.off {
		opacity: 0.6;
		border-style: dashed;
	}
	.offtag {
		font-size: 10px;
		color: #6b7280;
		border: 1px solid #d1d5db;
		border-radius: 8px;
		padding: 0 6px;
	}
	.card.warning {
		border-color: #d9a400;
	}
	.finding {
		grid-column: 1 / -1;
		color: #c40;
		background: #fee8e0;
		padding: 2px 6px;
		margin-top: 4px;
		font-size: 12px;
	}
	.finding.WARNING {
		color: #7a5b00;
		background: #fff4d6;
	}
	.finding.INFO {
		color: #345;
		background: #e8f0f8;
	}
	.finding i {
		display: block;
		color: #666;
		font-style: normal;
	}
	.finding .fix {
		display: inline-block;
		margin-top: 2px;
		font: inherit;
		font-size: 11px;
		padding: 0 6px;
		border: 1px solid #2a7;
		border-radius: 3px;
		background: #fff;
		color: #2a7;
		cursor: pointer;
	}
	.head .badge {
		border-radius: 8px;
		padding: 0 6px;
		font-size: 11px;
		font-weight: 600;
		color: #fff;
		border: none;
		cursor: pointer;
	}
	.head .badge.ERROR,
	.head .badge.FATAL {
		background: #c40;
	}
	.head .badge.WARNING {
		background: #d9a400;
	}
	.head .badge.INFO {
		background: #6a8fb5;
	}
	.hint {
		color: #666;
		margin: 2px 0 4px;
	}
	.lib {
		width: 100%;
		min-height: 140px;
		box-sizing: border-box;
		font: 11px ui-monospace, monospace;
		border: 1px solid #ddd;
	}
	input.mono {
		font-family: ui-monospace, monospace;
	}
	.row {
		display: grid;
		grid-template-columns: 90px 1fr;
		gap: 8px;
		align-items: center;
		padding: 2px 0;
	}
	.row > span:first-child {
		color: #345;
	}
	input,
	select {
		width: 100%;
		box-sizing: border-box;
		font: inherit;
		padding: 2px 4px;
		border: 1px solid #ccc;
		border-radius: 3px;
	}
	input[type='checkbox'] {
		width: auto;
	}
	.short {
		width: 70px;
	}
	.inline {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	.card {
		border: 1px solid #ddd;
		border-radius: 4px;
		margin: 6px 0;
		background: #fafafa;
	}
	.head {
		display: flex;
		gap: 4px;
		align-items: center;
		padding: 4px 6px;
	}
	.head button {
		border: none;
		background: none;
		cursor: pointer;
		font: inherit;
		color: #555;
		padding: 0 3px;
	}
	.type {
		font-weight: 600;
		color: #765;
	}
	.id {
		width: 120px;
		font-family: ui-monospace, monospace;
	}
	.spacer {
		flex: 1;
	}
	.fields {
		padding: 4px 8px 8px;
		border-top: 1px solid #eee;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.chip {
		border: 1px solid #ccc;
		border-radius: 10px;
		padding: 1px 7px;
		font-size: 11px;
		cursor: pointer;
		background: #fff;
	}
	.chip input {
		display: none;
	}
	.chip.on {
		background: #ffe0c7;
		border-color: #ff8c42;
	}
	.add {
		display: flex;
		gap: 6px;
		margin-top: 8px;
	}
	.add button {
		font: inherit;
		padding: 2px 10px;
		border: 1px solid #bbb;
		border-radius: 4px;
		background: #fff;
		cursor: pointer;
	}
</style>
