<script lang="ts">
	/** Sheet layouts from the plan, one SVG per sheet; click a part to select it. */
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import { app } from '$lib/state.svelte';
	import { Badge, EmptyState } from '$lib/ui';

	const layouts = $derived(app.plan?.nesting ?? []);
	const W = 560;
	const cutColor = (stage: number) => (stage === 1 ? 'var(--danger)' : stage === 2 ? 'var(--info)' : 'var(--success)');
</script>

{#if layouts.length === 0}
	<EmptyState title="Sin placas" description="El plan no tiene piezas para acomodar en placas.">
		{#snippet icon()}<LayoutGrid />{/snippet}
	</EmptyState>
{:else}
	<div class="flex flex-wrap gap-4 p-3">
		{#each layouts as l (l.material + l.index)}
			{@const scale = W / l.sheetLength}
			{@const H = l.sheetWidth * scale}
			<figure class="m-0 overflow-hidden rounded-md border bg-depth-0">
				<figcaption class="flex flex-wrap items-center gap-1.5 border-b bg-depth-1 px-2.5 py-1.5 text-xs">
					<span class="font-medium">{l.material}</span>
					<span class="text-muted-foreground">placa {l.index}</span>
					<span class="num font-mono text-2xs text-muted-foreground">{l.sheetLength}×{l.sheetWidth}</span>
					<span class="ml-auto flex gap-1">
						<Badge tone="primary">{Math.round((1 - l.wasteRatio) * 100)}% aprovechado</Badge>
						<Badge>{l.parts.length} piezas</Badge>
					</span>
				</figcaption>
				<svg width={W} height={H} viewBox="0 0 {W} {H}" class="block">
					<rect x="0" y="0" width={W} height={H} fill="var(--depth-1)" stroke="var(--border-strong)" />
					{#each l.parts as p (p.part)}
						<g
							class="cursor-pointer"
							onclick={() => app.selectPart(p.part)}
							role="button"
							tabindex="0"
							onkeydown={(e) => e.key === 'Enter' && app.selectPart(p.part)}
						>
							<rect
								x={p.x * scale}
								y={H - (p.y + p.width) * scale}
								width={p.length * scale}
								height={p.width * scale}
								fill={app.selectedPart === p.part ? 'var(--primary)' : p.rotated ? 'var(--warning-soft)' : 'var(--info-soft)'}
								stroke="var(--muted-foreground)"
								stroke-width="0.8"
							>
								<title>{p.part} {p.length}×{p.width}{p.rotated ? ' (girada)' : ''}</title>
							</rect>
							{#if p.length * scale > 30 && p.width * scale > 12}
								<text
									x={p.x * scale + 3}
									y={H - (p.y + p.width) * scale + 10}
									font-size="8"
									fill={app.selectedPart === p.part ? 'var(--primary-foreground)' : 'var(--foreground)'}>{p.part}</text
								>
							{/if}
						</g>
					{/each}
					{#each l.cuts ?? [] as c (c.order)}
						<line
							x1={c.x0 * scale}
							y1={H - c.y0 * scale}
							x2={c.x1 * scale}
							y2={H - c.y1 * scale}
							stroke={cutColor(c.stage)}
							stroke-width={c.stage === 1 ? 1.2 : 0.8}
							stroke-dasharray={c.stage === 3 ? '3 2' : undefined}
						>
							<title>corte {c.order} (etapa {c.stage})</title>
						</line>
					{/each}
				</svg>
				{#if l.cuts?.length}
					<div class="flex items-center gap-3 border-t px-2.5 py-1 text-2xs text-muted-foreground">
						<span>sierra: {l.cuts.length} cortes</span>
						<span class="flex items-center gap-1"><span class="h-0.5 w-3 bg-danger"></span>tiras</span>
						<span class="flex items-center gap-1"><span class="h-0.5 w-3 bg-info"></span>transversales</span>
						<span class="flex items-center gap-1"><span class="h-0.5 w-3 bg-success"></span>recortes</span>
					</div>
				{/if}
			</figure>
		{/each}
	</div>
{/if}
