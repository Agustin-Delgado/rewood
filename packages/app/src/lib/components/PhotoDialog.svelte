<script lang="ts">
	/**
	 * The realistic photo of what the viewer shows. It starts grainy and
	 * clears up sample by sample (path tracing); it can be saved at any
	 * point, and is best once the count reaches the goal.
	 */
	import * as THREE from 'three';
	import Download from '@lucide/svelte/icons/download';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import { app } from '$lib/state.svelte';
	import { Button, Dialog } from '$lib/ui';

	const GOAL = 300;
	const WIDTH = 1440;
	const HEIGHT = 900;

	let canvas: HTMLCanvasElement | undefined = $state();
	let samples = $state(0);
	let status: 'idle' | 'loading' | 'rendering' | 'done' | 'error' = $state('idle');
	let error = $state('');

	$effect(() => {
		if (!app.photoOpen || !canvas) return;
		const source = app.photoSource?.();
		const plan = app.plan;
		if (!source || !plan) return;
		let render: { stop(): void; dispose(): void } | null = null;
		let cancelled = false;
		samples = 0;
		status = 'loading';
		(async () => {
			try {
				const photo = await import('$lib/photo');
				const cat = {
					materials: plan.catalog?.materials ?? {},
					decors: { ...(app.libraries?.materials.decors ?? {}), ...(plan.catalog?.decors ?? {}) }
				};
				const scene = await photo.buildPhotoScene(source, plan, cat);
				if (cancelled || !canvas) return;
				// Aim at the furniture: the first children are its parts, the
				// room comes last.
				const furniture = new THREE.Box3();
				for (const c of scene.children.slice(0, -4)) furniture.expandByObject(c);
				const camera = photo.photoCamera(source, WIDTH / HEIGHT, furniture);
				status = 'rendering';
				render = photo.startRender(canvas, scene, camera, GOAL, (n) => {
					samples = n;
					if (n >= GOAL) status = 'done';
				});
			} catch (e) {
				status = 'error';
				error = e instanceof Error ? e.message : String(e);
			}
		})();
		return () => {
			cancelled = true;
			render?.dispose();
		};
	});

	function download() {
		if (!canvas) return;
		const a = document.createElement('a');
		a.href = canvas.toDataURL('image/png');
		a.download = `${app.spec.id}-foto.png`;
		a.click();
	}
</script>

<Dialog bind:open={app.photoOpen} title="Foto realista" description="El mueble tal como está en el visor: mismas medidas, colores, puertas y herrajes." size="xl" bodyClass="p-3">
	<div class="relative overflow-hidden rounded-md border bg-depth-2">
		<canvas bind:this={canvas} width={WIDTH} height={HEIGHT} class="block aspect-[16/10] w-full" aria-label="Foto realista del mueble"></canvas>
		{#if status === 'loading'}
			<div class="absolute inset-0 grid place-items-center text-sm text-muted-foreground">
				<span class="flex items-center gap-2"><LoaderCircle class="size-4 animate-spin" />Preparando la escena…</span>
			</div>
		{:else if status === 'error'}
			<div class="absolute inset-0 grid place-items-center p-6 text-center text-sm text-danger">No se pudo generar la foto: {error}</div>
		{/if}
	</div>
	{#snippet footer()}
		<div class="flex w-full items-center gap-3">
			<div class="flex min-w-0 flex-1 items-center gap-2 text-xs text-muted-foreground">
				<div class="h-1.5 w-40 overflow-hidden rounded-full bg-depth-3">
					<div class="h-full bg-primary transition-[width]" style="width: {Math.min(100, (samples / GOAL) * 100)}%"></div>
				</div>
				<span class="num">
					{#if status === 'done'}Lista{:else if status === 'rendering'}Aclarando la imagen… {samples}/{GOAL}{:else}&nbsp;{/if}
				</span>
			</div>
			<Button variant="primary" onclick={download} disabled={status !== 'rendering' && status !== 'done'}><Download />Descargar PNG</Button>
		</div>
	{/snippet}
</Dialog>
