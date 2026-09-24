<script lang="ts">
	import Box from '@lucide/svelte/icons/box';
	import Download from '@lucide/svelte/icons/download';
	import Eye from '@lucide/svelte/icons/eye';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Drill from '@lucide/svelte/icons/drill';
	import {
		Badge,
		Button,
		Checkbox,
		Dialog,
		EmptyState,
		Field,
		Input,
		Menu,
		NumberField,
		Popover,
		Section,
		Segmented,
		Select,
		Slider,
		Swatches,
		Switch,
		Tabs,
		TabsList,
		TabsPanel,
		TabsTab,
		Textarea,
		Toggle,
		Tooltip,
		recipes
	} from '$lib/ui';

	const colors = [
		['depth-0', 'superficie elevada'],
		['depth-1', 'fondo'],
		['depth-2', 'pozo'],
		['depth-3', 'pozo profundo'],
		['depth-4', 'borde de pozo'],
		['border', 'borde'],
		['border-strong', 'borde fuerte'],
		['foreground', 'texto'],
		['muted-foreground', 'texto secundario'],
		['subtle-foreground', 'texto terciario'],
		['primary', 'acento (madera)'],
		['primary-soft', 'acento suave'],
		['success', 'éxito'],
		['warning', 'aviso'],
		['danger', 'error'],
		['info', 'información']
	];

	let width = $state<number | null>(1800);
	let depth = $state(500);
	let view = $state<'3d' | 'front' | 'top'>('3d');
	let bays = $state<'2' | '3' | '4'>('3');
	let material = $state('mel18');
	let decor = $state('blanco_nature');
	let holes = $state(true);
	let hardware = $state(false);
	let open = $state(false);
	let tab = $state('a');
	let pill = $state('bom');
	let name = $state('Placard de módulos');
	let note = $state('');
	const t = recipes.table();
</script>

<svelte:head><title>Sistema de diseño · rewood</title></svelte:head>

<div class="mx-auto max-w-5xl px-6 py-10">
	<header class="mb-10 flex items-end justify-between gap-4 border-b pb-6">
		<div>
			<p class="text-2xs font-semibold tracking-[0.08em] text-primary uppercase">rewood</p>
			<h1 class="mt-1 text-xl font-semibold">Sistema de diseño</h1>
			<p class="mt-1 max-w-xl text-sm text-muted-foreground">
				Primitivas de <code class="font-mono text-xs">@human-kit/ui</code> con el aspecto de rewood. Las pantallas importan de
				<code class="font-mono text-xs">$lib/ui</code> y no escriben colores: todo sale de los tokens.
			</p>
		</div>
		<Button href="/" variant="ghost">Volver al editor</Button>
	</header>

	<h2 class="mb-3 text-sm font-semibold">Color</h2>
	<div class="mb-10 grid grid-cols-4 gap-3">
		{#each colors as [name, use] (name)}
			<div class="overflow-hidden rounded-lg border bg-depth-0">
				<div class="h-12 border-b" style="background: var(--{name === 'foreground' ? 'foreground' : name})"></div>
				<div class="px-2.5 py-1.5">
					<p class="font-mono text-2xs">--{name}</p>
					<p class="text-2xs text-muted-foreground">{use}</p>
				</div>
			</div>
		{/each}
	</div>

	<h2 class="mb-3 text-sm font-semibold">Tipografía</h2>
	<div class="mb-10 space-y-1.5 rounded-lg border bg-depth-0 p-4">
		<p class="text-xl font-semibold">Placard de módulos <span class="text-sm font-normal text-muted-foreground">text-xl</span></p>
		<p class="text-lg font-semibold">Con cajonera <span class="text-sm font-normal text-muted-foreground">text-lg</span></p>
		<p class="text-base">Costado izquierdo, 18 mm <span class="text-sm text-muted-foreground">text-base</span></p>
		<p class="text-sm">Cuerpo de la interfaz: 13 px <span class="text-muted-foreground">text-sm</span></p>
		<p class="text-xs text-muted-foreground">Secundario: medidas, notas <span>text-xs</span></p>
		<p class="text-2xs tracking-[0.06em] text-subtle-foreground uppercase">Rótulo de sección · text-2xs</p>
		<p class="num font-mono text-xs">1800 × 2200 × 518 mm · font-mono + num</p>
	</div>

	<h2 class="mb-3 text-sm font-semibold">Botones</h2>
	<div class="mb-10 space-y-3 rounded-lg border bg-depth-0 p-4">
		<div class="flex flex-wrap items-center gap-2">
			<Button variant="primary"><Download />Descargar paquete</Button>
			<Button>Informe</Button>
			<Button variant="soft"><Plus />Agregar</Button>
			<Button variant="ghost">Restablecer</Button>
			<Button variant="danger"><Trash2 />Quitar</Button>
			<Button variant="link">abrir spec…</Button>
			<Button variant="primary" disabled>Fabricación bloqueada</Button>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<Button size="xs">xs</Button>
			<Button size="sm">sm</Button>
			<Button size="md">md</Button>
			<Button size="icon-xs" variant="ghost" aria-label="Editar"><Pencil /></Button>
			<Button size="icon-sm" variant="ghost" aria-label="Ver"><Eye /></Button>
			<Button size="icon" aria-label="Más"><Ellipsis /></Button>
			<Toggle bind:selected={holes}>perforaciones</Toggle>
			<Toggle bind:selected={hardware} variant="secondary">herrajes</Toggle>
			<Tooltip text="Mostrar perforaciones">
				<Toggle size="icon-sm" bind:selected={holes} aria-label="Perforaciones"><Drill /></Toggle>
			</Tooltip>
			<span class="text-2xs text-subtle-foreground">← botón de ícono con Tooltip (hover o Tab)</span>
		</div>
	</div>

	<h2 class="mb-3 text-sm font-semibold">Selección</h2>
	<div class="mb-10 grid grid-cols-2 gap-4">
		<div class="space-y-3 rounded-lg border bg-depth-0 p-4">
			<Swatches
				aria-label="Color"
				value={decor}
				onChange={(v) => (decor = v)}
				options={[
					{ value: 'blanco_nature', label: 'Blanco Nature', hex: '#f2f1ec', hint: 'Faplac 135NAT' },
					{ value: 'gris_humo', label: 'Gris Humo', hex: '#8d8c88', hint: 'Faplac 108TXT' },
					{ value: 'grafito', label: 'Grafito', hex: '#4a4b4d', hint: 'Faplac 107TXT' },
					{ value: 'carvalho_mezzo', label: 'Carvalho Mezzo', hex: '#a8804f', hint: 'Faplac 042NAT', grain: true },
					{ value: 'nogal_terracota', label: 'Nogal Terracota', hex: '#6f4a30', hint: 'Faplac 046NAT', grain: true }
				]}
			/>
			<Segmented
				aria-label="Vista"
				bind:value={view}
				options={[
					{ value: '3d', label: '3D' },
					{ value: 'front', label: 'frente' },
					{ value: 'top', label: 'planta' }
				]}
			/>
			<Segmented
				aria-label="Módulos"
				size="md"
				fill
				bind:value={bays}
				options={[
					{ value: '2', label: '2' },
					{ value: '3', label: '3' },
					{ value: '4', label: '4' }
				]}
			/>
			<div class="flex gap-4">
				<Checkbox bind:checked={holes}>perforaciones</Checkbox>
				<Checkbox checked indeterminate>mixto</Checkbox>
				<Switch bind:checked={hardware}>herrajes</Switch>
			</div>
		</div>
		<div class="space-y-3 rounded-lg border bg-depth-0 p-4">
			<Field label="Material" for="mat">
				<Select
					id="mat"
					bind:value={material}
					options={[
						{ value: 'mel18', label: 'Melamina 18 mm', hint: '1,83 × 2,75', group: 'Placas' },
						{ value: 'mdf18', label: 'MDF 18 mm', hint: '1,83 × 2,60', group: 'Placas' },
						{ value: 'hdf3', label: 'HDF 3 mm', hint: 'fondo', group: 'Fondos' }
					]}
				/>
			</Field>
			<Field label="Ancho" hint="Más ancho, el fondo de HDF no sale de una placa." for="w">
				<NumberField id="w" bind:value={width} min={972} max={1830} unit="mm" steppers />
			</Field>
			<Slider aria-label="Profundidad" bind:value={depth} min={480} max={650} />
		</div>
	</div>

	<h2 class="mb-3 text-sm font-semibold">Campos</h2>
	<div class="mb-10 grid grid-cols-2 gap-4 rounded-lg border bg-depth-0 p-4">
		<Field label="Nombre" for="n"><Input id="n" bind:value={name} /></Field>
		<Field label="id" for="i"><Input id="i" value="wardrobe_modules" mono /></Field>
		<Field label="Nota" for="t" class="col-span-2"><Textarea id="t" bind:value={note} placeholder="Algo para el taller…" /></Field>
		<Field label="Espesor" layout="inline" for="e"><NumberField id="e" value={18} unit="mm" size="xs" /></Field>
		<Field label="Inválido" layout="inline" for="x" error="Supera el largo de la placa"><NumberField id="x" value={3000} unit="mm" size="xs" invalid /></Field>
	</div>

	<h2 class="mb-3 text-sm font-semibold">Pestañas, secciones, tabla</h2>
	<div class="mb-10 grid grid-cols-2 gap-4">
		<div class="overflow-hidden rounded-lg border bg-depth-0">
			<Tabs bind:value={tab}>
				<TabsList aria-label="Panel">
					<TabsTab value="a">Diseño</TabsTab>
					<TabsTab value="b">Parámetros</TabsTab>
					<TabsTab value="c">Biblioteca</TabsTab>
				</TabsList>
				<TabsPanel value="a">
					<Section title="Medidas" count={3}>
						<p class="text-xs text-muted-foreground">Ancho, alto y profundidad.</p>
					</Section>
					<Section title="Frentes" open={false}>
						<p class="text-xs text-muted-foreground">Cerrado de entrada.</p>
					</Section>
					<Section title="Fijo" static>
						{#snippet actions()}<Button size="icon-xs" variant="ghost" aria-label="Agregar"><Plus /></Button>{/snippet}
						<p class="text-xs text-muted-foreground">Sin plegar, con una acción.</p>
					</Section>
				</TabsPanel>
				<TabsPanel value="b"><p class="p-3 text-xs text-muted-foreground">Parámetros de la pieza.</p></TabsPanel>
				<TabsPanel value="c"><p class="p-3 text-xs text-muted-foreground">Biblioteca del taller.</p></TabsPanel>
			</Tabs>
		</div>
		<div class="space-y-3">
			<Tabs bind:value={pill} variant="pill">
				<TabsList aria-label="Salida">
					<TabsTab value="bom">BOM</TabsTab>
					<TabsTab value="cnc">CNC</TabsTab>
					<TabsTab value="dxf">Placas</TabsTab>
				</TabsList>
			</Tabs>
			<div class="max-h-48 overflow-auto rounded-lg border bg-depth-0">
				<table class={t.root()}>
					<thead><tr><th class={t.th()}>Pieza</th><th class={t.th({ class: 'text-right' })}>Largo</th><th class={t.th({ class: 'text-right' })}>Ancho</th><th class={t.th()}>Estado</th></tr></thead>
					<tbody>
						{#each [['Costado izq.', 2182, 500, 'ok'], ['Techo', 1764, 500, 'ok'], ['Puerta 1', 2136, 596, 'aviso'], ['Fondo', 2176, 1776, 'error']] as [p, l, a, s] (p)}
							<tr class={t.tr()} data-selected={p === 'Techo' || undefined}>
								<td class={t.td()}>{p}</td>
								<td class={t.td({ class: 'num text-right font-mono' })}>{l}</td>
								<td class={t.td({ class: 'num text-right font-mono' })}>{a}</td>
								<td class={t.td()}>
									<Badge tone={s === 'ok' ? 'success' : s === 'aviso' ? 'warning' : 'danger'}>{s}</Badge>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			<div class="flex flex-wrap gap-1.5">
				<Badge>35 piezas</Badge><Badge tone="primary">fabricable</Badge><Badge tone="info">INFO</Badge><Badge tone="warning">WARNING</Badge><Badge tone="danger">FATAL</Badge><Badge outline>v1.0</Badge>
			</div>
		</div>
	</div>

	<h2 class="mb-3 text-sm font-semibold">Superficies flotantes</h2>
	<div class="mb-10 flex flex-wrap items-center gap-3 rounded-lg border bg-depth-0 p-4">
		<Button variant="primary" onclick={() => (open = true)}>Abrir diálogo</Button>
		<Menu
			aria-label="Acciones"
			variant="secondary"
			items={[
				{ heading: 'Componente' },
				{ label: 'Duplicar', onSelect: () => {}, hint: 'Ctrl D' },
				{ label: 'Subir', onSelect: () => {} },
				{ separator: true },
				{ label: 'Quitar', onSelect: () => {}, icon: Trash2, danger: true }
			]}
		>
			{#snippet trigger()}Acciones<Ellipsis />{/snippet}
		</Menu>
		<Popover>
			{#snippet trigger()}Popover{/snippet}
			<p class="w-56 text-xs text-muted-foreground">Contenido libre anclado a un botón.</p>
		</Popover>
		<div class="flex-1"></div>
		<div class="w-72 rounded-lg border border-dashed">
			<EmptyState title="Sin hallazgos" description="El mueble se puede fabricar tal como está.">
				{#snippet icon()}<Box />{/snippet}
			</EmptyState>
		</div>
	</div>

	<Dialog bind:open title="Elegir un mueble" description="Los ejemplos del catálogo, por categoría." size="md">
		<div class="grid grid-cols-2 gap-3 p-4">
			{#each ['Bajo mesada', 'Alacena', 'Placard', 'Biblioteca'] as n (n)}
				<button class={recipes.card({ interactive: true, class: 'p-3' })}>
					<p class="text-sm font-medium">{n}</p>
					<p class="text-xs text-muted-foreground">3 variantes</p>
				</button>
			{/each}
		</div>
		{#snippet footer()}
			<Button variant="ghost" onclick={() => (open = false)}>Cancelar</Button>
			<Button variant="primary" onclick={() => (open = false)}>Usar</Button>
		{/snippet}
	</Dialog>
</div>
