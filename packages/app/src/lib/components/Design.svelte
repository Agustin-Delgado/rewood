<script lang="ts">
	/**
	 * The template's options as controls: numbers with a slider, switches,
	 * choices as buttons. Bounds, what applies right now and what is out of
	 * range all come from the plan (the engine resolved them); a change
	 * writes the parameter back and recompiles.
	 */
	import type { PlanOption } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';
	import { findVariant, overall } from '$lib/catalog';

	const options = $derived((app.plan?.options ?? []).filter((o) => o.active));
	const groups = $derived.by(() => {
		const map = new Map<string, PlanOption[]>();
		for (const o of options) {
			const g = o.group ?? 'Opciones';
			if (!map.has(g)) map.set(g, []);
			map.get(g)!.push(o);
		}
		return [...map.entries()];
	});
	const current = $derived(app.variant ? findVariant(app.variant) : null);
	const size = $derived(app.plan ? overall(app.plan) : null);
	const items = $derived(app.plan?.diagnostics.items ?? []);
	const errors = $derived(items.filter((d) => d.severity === 'ERROR' || d.severity === 'FATAL'));
	/** What stands between the piece and the workshop (an out-of-range value shows on its option). */
	const blocking = $derived(errors.filter((d) => d.code !== 'SPEC-503'));
	const warnings = $derived(items.filter((d) => d.severity === 'WARNING'));
	const hardwareCount = $derived(app.plan?.joints.reduce((n, j) => n + j.fasteners.length, 0) ?? 0);

	/** The finding an option's value triggered, if any. */
	const outOfRange = (o: PlanOption) => items.find((d) => d.code === 'SPEC-503' && d.entity === o.param);

	// A slider fires on every pixel: the value on screen follows at once,
	// the recompile at most once a frame.
	let drafts: Record<string, number> = $state({});
	let pending: { param: string; value: number } | null = null;
	let frame = 0;
	function slide(param: string, value: number) {
		drafts[param] = value;
		pending = { param, value };
		if (!frame)
			frame = requestAnimationFrame(() => {
				frame = 0;
				if (pending) app.setParameter(pending.param, pending.value);
				pending = null;
			});
	}
	function set(o: PlanOption, value: number | boolean) {
		delete drafts[o.param];
		app.setParameter(o.param, value);
	}
	function num(o: PlanOption): number {
		return drafts[o.param] ?? (o.value as number);
	}
	function stepOf(o: PlanOption) {
		return o.step ?? 1;
	}
	function nudge(o: PlanOption, d: number) {
		let v = num(o) + d * stepOf(o);
		if (o.min !== undefined) v = Math.max(o.min, v);
		if (o.max !== undefined) v = Math.min(o.max, v);
		set(o, Math.round(v * 1000) / 1000);
	}
	/** A typed value: empty or not a number puts the current one back (`Number('')` is 0). */
	function typed(o: PlanOption, input: HTMLInputElement) {
		const raw = input.value.trim().replace(',', '.');
		const v = Number(raw);
		if (raw === '' || !Number.isFinite(v)) {
			input.value = String(num(o));
			return;
		}
		if (v !== num(o)) set(o, v);
	}
	function reset() {
		if (app.variant) app.loadVariant(app.variant);
	}
</script>

<div class="design">
	<div class="summary">
		<div class="title">
			{#if current}
				<span class="crumb">{current.category.name}</span>
				<span class="name">{current.variant.name}</span>
			{:else}
				<span class="name">{app.spec.name}</span>
			{/if}
		</div>
		{#if size}
			<div class="size">{size[0]} <i>ancho</i> × {size[1]} <i>alto</i> × {size[2]} <i>prof.</i> mm</div>
		{/if}
		<div class="stats">
			<span>{app.plan?.parts.length ?? 0} piezas</span>
			<span>{hardwareCount} herrajes</span>
			{#if errors.length}
				<span class="bad">{errors.length} {errors.length === 1 ? 'error' : 'errores'}</span>
			{:else if warnings.length}
				<span class="warn">{warnings.length} {warnings.length === 1 ? 'aviso' : 'avisos'}</span>
			{:else}
				<span class="ok">fabricable</span>
			{/if}
		</div>
		<div class="actions">
			<button onclick={() => (app.catalogOpen = true)}>Cambiar mueble…</button>
			{#if app.variant}<button onclick={reset} title="Volver a los valores de esta variante">Restablecer</button>{/if}
		</div>
	</div>

	{#if options.length === 0}
		<p class="empty">
			Este mueble no declara opciones. Sus medidas están en <b>Parámetros</b> y sus partes en <b>Componentes</b>.
		</p>
	{/if}

	{#each groups as [group, list] (group)}
		<h3>{group}</h3>
		{#each list as o (o.param)}
			{@const bad = outOfRange(o)}
			<div class="option" class:invalid={!!bad}>
				<div class="label">
					<span>{o.label}</span>
					{#if o.kind === 'number'}
						<span class="value">
							<button class="nudge" onclick={() => nudge(o, -1)} disabled={o.min !== undefined && num(o) <= o.min} aria-label="menos">−</button>
							<input
								type="text"
								inputmode="decimal"
								value={num(o)}
								onchange={(e) => typed(o, e.currentTarget)}
								onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
							/>
							{#if o.unit}<span class="unit">{o.unit}</span>{/if}
							<button class="nudge" onclick={() => nudge(o, 1)} disabled={o.max !== undefined && num(o) >= o.max} aria-label="más">+</button>
						</span>
					{:else if o.kind === 'toggle'}
						<button
							class="switch"
							role="switch"
							aria-checked={o.value === true}
							aria-label={o.label}
							class:on={o.value === true}
							onclick={() => set(o, o.value !== true)}
						><span></span></button>
					{/if}
				</div>
				{#if o.kind === 'number' && o.min !== undefined && o.max !== undefined && o.max > o.min}
					{#if (o.max - o.min) / stepOf(o) <= 8}
						<div class="ticks">
							{#each Array.from({ length: Math.round((o.max - o.min) / stepOf(o)) + 1 }, (_, i) => o.min! + i * stepOf(o)) as v (v)}
								<button class:on={Math.abs(num(o) - v) < 1e-6} onclick={() => set(o, v)}>{v}</button>
							{/each}
						</div>
					{:else}
						<input
							class="slider"
							type="range"
							min={o.min}
							max={o.max}
							step={stepOf(o)}
							value={num(o)}
							oninput={(e) => slide(o.param, Number(e.currentTarget.value))}
							onchange={(e) => set(o, Number(e.currentTarget.value))}
						/>
						<div class="range"><span>{o.min}</span><span>{o.max}</span></div>
					{/if}
				{:else if o.kind === 'choice'}
					<div class="choices">
						{#each o.choices ?? [] as c (c.value)}
							<button class:on={o.value === c.value} onclick={() => set(o, c.value)}>{c.label}</button>
						{/each}
					</div>
				{/if}
				{#if o.help}<p class="help">{o.help}</p>{/if}
				{#if bad}
					<p class="finding">
						{bad.message}
						{#if bad.fix}<button class="fix" onclick={() => app.applyFix(bad.fix!)}>{bad.fix.label}</button>{/if}
					</p>
				{/if}
			</div>
		{/each}
	{/each}

	{#if blocking.length}
		<h3>Para poder fabricarlo</h3>
		{#each blocking.slice(0, 4) as d (d.code + (d.entity ?? '') + d.message)}
			<p class="finding">
				{d.message}
				{#if d.fix}<button class="fix" onclick={() => app.applyFix(d.fix!)}>{d.fix.label}</button>{/if}
			</p>
		{/each}
	{/if}
</div>

<style>
	.design {
		font-size: 13px;
		overflow: auto;
		height: 100%;
		padding: 10px 12px 16px;
		box-sizing: border-box;
	}
	.summary {
		border: 1px solid #e5e7eb;
		border-radius: 8px;
		padding: 10px 12px;
		background: #fcfbf9;
		display: grid;
		gap: 4px;
	}
	.title {
		display: flex;
		flex-direction: column;
	}
	.crumb {
		color: #9ca3af;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}
	.name {
		font-size: 15px;
		font-weight: 600;
	}
	.size {
		font-variant-numeric: tabular-nums;
		color: #374151;
	}
	.size i {
		font-style: normal;
		color: #9ca3af;
		font-size: 11px;
	}
	.stats {
		display: flex;
		gap: 10px;
		color: #6b7280;
		font-size: 12px;
	}
	.ok {
		color: #047857;
	}
	.warn {
		color: #92400e;
	}
	.bad {
		color: #b91c1c;
		font-weight: 600;
	}
	.actions {
		display: flex;
		gap: 6px;
		margin-top: 4px;
	}
	button {
		font: inherit;
		cursor: pointer;
	}
	.actions button {
		font-size: 12px;
		padding: 3px 10px;
		border: 1px solid #d1d5db;
		border-radius: 5px;
		background: #fff;
	}
	.actions button:first-child {
		border-color: #ff8c42;
		color: #c2410c;
	}
	h3 {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: #6b7280;
		margin: 16px 0 6px;
	}
	.empty {
		color: #6b7280;
	}
	.option {
		padding: 8px 0;
		border-bottom: 1px solid #f3f4f6;
	}
	.option.invalid .value input {
		border-color: #dc2626;
	}
	.label {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		min-height: 26px;
	}
	.value {
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.value input {
		width: 64px;
		text-align: right;
		font: inherit;
		font-variant-numeric: tabular-nums;
		padding: 2px 6px;
		border: 1px solid #d1d5db;
		border-radius: 4px;
	}
	.unit {
		color: #9ca3af;
		font-size: 11px;
	}
	.nudge {
		width: 24px;
		height: 24px;
		border: 1px solid #d1d5db;
		border-radius: 4px;
		background: #fff;
		line-height: 1;
	}
	.nudge:disabled {
		color: #d1d5db;
		cursor: default;
	}
	.slider {
		width: 100%;
		accent-color: #ff8c42;
		margin: 6px 0 0;
	}
	.range {
		display: flex;
		justify-content: space-between;
		color: #9ca3af;
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}
	.ticks,
	.choices {
		display: flex;
		gap: 4px;
		margin-top: 6px;
		flex-wrap: wrap;
	}
	.ticks button,
	.choices button {
		border: 1px solid #d1d5db;
		background: #fff;
		border-radius: 5px;
		padding: 3px 10px;
		min-width: 32px;
	}
	.ticks button.on,
	.choices button.on {
		background: #ff8c42;
		border-color: #ff8c42;
		color: #fff;
	}
	.choices button {
		flex: 1;
	}
	.switch {
		width: 36px;
		height: 20px;
		border-radius: 10px;
		border: none;
		background: #d1d5db;
		position: relative;
		padding: 0;
		transition: background 0.15s;
	}
	.switch span {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: #fff;
		transition: left 0.15s;
	}
	.switch.on {
		background: #ff8c42;
	}
	.switch.on span {
		left: 18px;
	}
	.help {
		margin: 4px 0 0;
		color: #6b7280;
		font-size: 12px;
	}
	.finding {
		margin: 6px 0 0;
		color: #b91c1c;
		background: #fef2f2;
		border-radius: 4px;
		padding: 4px 8px;
		font-size: 12px;
	}
	.fix {
		display: inline-block;
		margin-left: 6px;
		font-size: 11px;
		padding: 0 6px;
		border: 1px solid #059669;
		border-radius: 3px;
		background: #fff;
		color: #059669;
	}
</style>
