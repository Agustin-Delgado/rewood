<script lang="ts">
	/** Sheet layouts from the plan, one SVG per sheet; click a part to select it. */
	import { app } from '$lib/state.svelte';

	const layouts = $derived(app.plan?.nesting ?? []);
	const W = 560;
</script>

<div class="sheets">
	{#each layouts as l (l.material + l.index)}
		{@const scale = W / l.sheetLength}
		{@const H = l.sheetWidth * scale}
		<figure>
			<figcaption>
				{l.material} — placa {l.index} · {l.sheetLength}×{l.sheetWidth} · aprovechamiento {Math.round((1 - l.wasteRatio) * 100)}% ·
				{l.parts.length} piezas
			</figcaption>
			<svg width={W} height={H} viewBox="0 0 {W} {H}">
				<rect x="0" y="0" width={W} height={H} fill="#fff" stroke="#333" />
				{#each l.parts as p (p.part)}
					<g onclick={() => (app.selectedPart = p.part)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && (app.selectedPart = p.part)}>
						<rect
							x={p.x * scale}
							y={H - (p.y + p.width) * scale}
							width={p.length * scale}
							height={p.width * scale}
							fill={app.selectedPart === p.part ? '#ff8c42' : p.rotated ? '#f6e8c8' : '#e8eef6'}
							stroke="#345"
							stroke-width="0.8"
						>
							<title>{p.part} {p.length}×{p.width}{p.rotated ? ' (girada)' : ''}</title>
						</rect>
						{#if p.length * scale > 30 && p.width * scale > 12}
							<text x={p.x * scale + 3} y={H - (p.y + p.width) * scale + 10} font-size="8" fill="#111">{p.part}</text>
						{/if}
					</g>
				{/each}
				{#each l.cuts ?? [] as c (c.order)}
					<line
						x1={c.x0 * scale}
						y1={H - c.y0 * scale}
						x2={c.x1 * scale}
						y2={H - c.y1 * scale}
						stroke={c.stage === 1 ? '#c00' : c.stage === 2 ? '#06c' : '#0a0'}
						stroke-width={c.stage === 1 ? 1.2 : 0.8}
						stroke-dasharray={c.stage === 3 ? '3 2' : undefined}
					>
						<title>corte {c.order} (etapa {c.stage})</title>
					</line>
				{/each}
			</svg>
			{#if l.cuts?.length}
				<div class="cuts">sierra: {l.cuts.length} cortes · rojo tiras, azul transversales, verde recortes</div>
			{/if}
		</figure>
	{/each}
	{#if layouts.length === 0}
		<p class="muted">Sin placas.</p>
	{/if}
</div>

<style>
	.sheets {
		display: flex;
		flex-wrap: wrap;
		gap: 16px;
	}
	figure {
		margin: 0;
	}
	figcaption {
		font-size: 11px;
		margin-bottom: 4px;
		color: #333;
	}
	.cuts {
		font-size: 10px;
		color: #666;
	}
	g {
		cursor: pointer;
	}
	.muted {
		color: #888;
	}
</style>
