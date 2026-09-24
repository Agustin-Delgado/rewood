<script lang="ts">
	/**
	 * The parts, by component, in words: "Cajones · módulo 2" and not
	 * `drawers_m2`, a drawer's six pieces under "Cajón 1", sizes rounded to
	 * what a tape measure reads. Groups start folded; the part picked in
	 * the viewer unfolds its own.
	 */
	import type { Part } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';
	import { componentLabels, partName, size } from '$lib/labels';
	import { Button } from '$lib/ui';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Eye from '@lucide/svelte/icons/eye';
	import EyeOff from '@lucide/svelte/icons/eye-off';

	type Sub = { key: string; label: string; parts: Part[] };
	type Group = { component: string; label: string; count: number; subs: Sub[]; loose: Part[] };

	const labels = $derived(componentLabels(app.spec, app.plan));
	const groups = $derived.by(() => {
		const map = new Map<string, Group>();
		for (const p of app.plan?.parts ?? []) {
			let g = map.get(p.component);
			if (!g) {
				g = { component: p.component, label: labels.get(p.component) ?? p.component, count: 0, subs: [], loose: [] };
				map.set(p.component, g);
			}
			g.count += 1;
			// One drawer = one line that unfolds to its pieces.
			const m = p.role.match(/^(?:bay(\d+)_)?(?:.*_)?drawer_(\d+)_/);
			if (m) {
				const key = `${p.component}:${m[1] ?? ''}:${m[2]}`;
				let sub = g.subs.find((s) => s.key === key);
				if (!sub) {
					sub = { key, label: `Cajón ${m[2]}${m[1] ? ` (hueco ${m[1]})` : ''}`, parts: [] };
					g.subs.push(sub);
				}
				sub.parts.push(p);
			} else g.loose.push(p);
		}
		return [...map.values()];
	});

	let open = $state<Set<string>>(new Set());
	function toggle(key: string) {
		const next = new Set(open);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		open = next;
	}
	// The part selected in the viewer shows where it is.
	$effect(() => {
		const id = app.selectedPart;
		if (!id) return;
		const g = groups.find((x) => x.loose.some((p) => p.id === id) || x.subs.some((s) => s.parts.some((p) => p.id === id)));
		if (!g) return;
		const next = new Set(open);
		next.add(g.component);
		for (const s of g.subs) if (s.parts.some((p) => p.id === id)) next.add(s.key);
		if (next.size !== open.size) open = next;
	});
</script>

<div class="h-full overflow-y-auto py-1 text-sm">
	<div class="flex h-9 items-center gap-1.5 px-3">
		<h3 class="text-2xs font-semibold tracking-[0.06em] text-muted-foreground uppercase">Piezas</h3>
		<span class="num text-2xs text-subtle-foreground">{app.plan?.parts.length ?? 0}</span>
	</div>
	<div class="px-1.5">
		{#each groups as g (g.component)}
			{@const hidden = app.hiddenComponents.has(g.component)}
			<div class="group/row flex items-center gap-0.5">
				<button
					class="flex h-7 min-w-0 flex-1 items-center gap-1.5 rounded-sm px-1.5 text-left font-medium outline-none hover:bg-depth-2 focus-visible:ring-2 focus-visible:ring-ring/60 {hidden ? 'text-subtle-foreground' : ''}"
					title={g.component}
					aria-expanded={open.has(g.component)}
					onclick={() => toggle(g.component)}
				>
					<ChevronRight class="size-3.5 shrink-0 text-subtle-foreground transition-transform duration-150 {open.has(g.component) ? 'rotate-90' : ''}" />
					<span class="min-w-0 truncate">{g.label}</span>
					<span class="num ml-auto pl-2 text-xs font-normal text-subtle-foreground">{g.count}</span>
				</button>
				<Button
					variant="ghost"
					size="icon-xs"
					class={hidden ? 'text-subtle-foreground' : 'opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100'}
					title={hidden ? 'Mostrar en el 3D' : 'Ocultar en el 3D'}
					aria-label={hidden ? 'Mostrar en el 3D' : 'Ocultar en el 3D'}
					onclick={() => app.toggleComponent(g.component)}
				>
					{#if hidden}<EyeOff />{:else}<Eye />{/if}
				</Button>
			</div>
			{#if open.has(g.component)}
				{#each g.loose as part (part.id)}
					{@render row(part)}
				{/each}
				{#each g.subs as s (s.key)}
					<button
						class="flex h-7 w-full items-center gap-1.5 rounded-sm pr-1.5 pl-6 text-left outline-none hover:bg-depth-2 focus-visible:ring-2 focus-visible:ring-ring/60"
						aria-expanded={open.has(s.key)}
						onclick={() => toggle(s.key)}
					>
						<ChevronRight class="size-3.5 shrink-0 text-subtle-foreground transition-transform duration-150 {open.has(s.key) ? 'rotate-90' : ''}" />
						<span class="min-w-0 truncate">{s.label}</span>
						<span class="num ml-auto pl-2 text-xs text-subtle-foreground">{s.parts.length} piezas</span>
					</button>
					{#if open.has(s.key)}
						{#each s.parts as part (part.id)}
							{@render row(part, true)}
						{/each}
					{/if}
				{/each}
			{/if}
		{/each}
	</div>
</div>

{#snippet row(part: Part, nested = false)}
	{@const selected = app.selectedPart === part.id}
	<button
		class="flex h-7 w-full items-center justify-between gap-2 rounded-sm pr-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/60 {nested ? 'pl-12' : 'pl-7'} {selected
			? 'bg-primary-soft font-medium text-primary-soft-foreground'
			: 'hover:bg-depth-2'}"
		title="{part.id} · {part.material}"
		aria-current={selected || undefined}
		onclick={() => app.selectPart(part.id)}
	>
		<span class="min-w-0 truncate">{partName(part.name)}</span>
		<span class="num shrink-0 font-mono text-2xs whitespace-nowrap {selected ? 'text-primary-soft-foreground/80' : 'text-subtle-foreground'}">{size(part.dims)}</span>
	</button>
{/snippet}
