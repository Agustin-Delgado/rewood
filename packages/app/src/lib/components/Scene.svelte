<script lang="ts">
	/**
	 * Every part as a box at its furniture-space AABB, every hole as a short
	 * cylinder into its face, every groove as a slot and every fastener as
	 * the shape of its hardware where its holes are. Nothing here is
	 * geometry of its own: it is the plan, drawn.
	 */
	import { T, useThrelte } from '@threlte/core';
	import { OrbitControls, interactivity } from '@threlte/extras';
	import * as THREE from 'three';
	import type { OrbitControls as OrbitControlsImpl } from 'three/examples/jsm/controls/OrbitControls.js';
	import type { Part, Vec3 } from '@rewood/engine/browser';
	import { untrack } from 'svelte';
	import { app, type ViewName } from '$lib/state.svelte';
	import { explodeOffsets, faceNormalWorld, faceUvToWorld, toThree } from '$lib/geometry';
	import { hardwareSymbols, type HardwareSymbol, type Prim } from '$lib/hardware3d';

	interactivity();
	const { size } = useThrelte();

	const parts = $derived(app.plan?.parts.filter((p) => !app.hiddenComponents.has(p.component)) ?? []);
	// Offsets from every part, hidden ones included, so hiding a component
	// does not move the rest.
	const offsets = $derived(explodeOffsets(app.plan?.parts ?? [], app.explode));
	function offsetOf(part: Part): [number, number, number] {
		return toThree(offsets.get(part.id) ?? [0, 0, 0]);
	}

	const bounds = $derived.by(() => {
		let max = [1, 1, 1];
		for (const p of app.plan?.parts ?? []) {
			max = [Math.max(max[0], p.aabb.max[0]), Math.max(max[1], p.aabb.max[1]), Math.max(max[2], p.aabb.max[2])];
		}
		return max;
	});
	/** Lowest Z: the floor sits under the legs, not under the carcass. */
	const floor = $derived((app.plan?.parts ?? []).reduce((m, p) => Math.min(m, p.aabb.min[2]), 0));
	const target = $derived<[number, number, number]>([bounds[0] / 2, (bounds[2] + floor) / 2, bounds[1] / 2]);
	const distance = $derived(Math.max(bounds[0], bounds[2]) * 1.6 + bounds[1]);

	// --- camera and views ---------------------------------------------------
	// A perspective corner for the 3D view; the elevations look along one
	// axis without perspective, the way the drawings do, and still orbit.
	const DIRS: Record<ViewName, [number, number, number]> = {
		iso: [0.7, 0.45, 1],
		front: [0, 0, 1],
		back: [0, 0, -1],
		left: [-1, 0, 0],
		right: [1, 0, 0],
		top: [0, 1, 0.0001]
	};
	const ortho = $derived(app.view !== 'iso');
	let controls = $state<OrbitControlsImpl>();
	let persp = $state<THREE.PerspectiveCamera>();
	let orthoCam = $state<THREE.OrthographicCamera>();
	const camera = $derived(ortho ? orthoCam : persp);
	/** What the elevation has to fit on screen (width × height of the view, mm). */
	function extent(view: ViewName): [number, number] {
		const w = bounds[0];
		const d = bounds[1];
		const h = bounds[2] - floor;
		switch (view) {
			case 'front':
			case 'back':
				return [w, h];
			case 'left':
			case 'right':
				return [d, h];
			case 'top':
				return [w, d];
			default:
				return [w, h];
		}
	}
	// Frames on request only: a recompile that changes the size must not
	// throw away where the user orbited to.
	$effect(() => {
		void app.viewTick;
		const cam = camera;
		const ctl = controls;
		if (!cam || !ctl) return;
		const view = app.view;
		untrack(() => frame(cam, ctl, view));
	});
	function frame(cam: THREE.PerspectiveCamera | THREE.OrthographicCamera, ctl: OrbitControlsImpl, view: ViewName) {
		const dir = DIRS[view];
		const far = ortho ? distance * 3 : distance;
		cam.position.set(target[0] + dir[0] * far, target[1] + dir[1] * far, target[2] + dir[2] * far);
		cam.up.set(0, 1, 0);
		ctl.target.set(target[0], target[1], target[2]);
		if (cam instanceof THREE.OrthographicCamera) {
			// Threlte keeps the frustum at the canvas size in pixels, so the
			// zoom is pixels per millimetre: fit the view with a margin.
			const [w, h] = extent(view);
			cam.zoom = Math.min($size.width / (w * 1.25), $size.height / (h * 1.25));
			cam.updateProjectionMatrix();
		}
		ctl.update();
	}

	/**
	 * Rails have no panel: draw a bar between the two support fixtures of
	 * each rail component (one pair per bay).
	 */
	const rails = $derived.by(() => {
		const out: { id: string; pos: [number, number, number]; len: number }[] = [];
		const joints = (app.plan?.joints ?? []).filter((j) => j.kind === 'fixture' && j.hardware.some((h) => h.startsWith('rail_support')));
		const byComponent = new Map<string, typeof joints>();
		for (const j of joints) byComponent.set(j.component, [...(byComponent.get(j.component) ?? []), j]);
		for (const [component, list] of byComponent) {
			for (let i = 0; i + 1 < list.length; i += 2) {
				const a = list[i].fasteners[0]?.position;
				const b = list[i + 1].fasteners[0]?.position;
				if (!a || !b) continue;
				out.push({
					id: `${component}:${i}`,
					pos: toThree([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2]),
					len: Math.abs(b[0] - a[0])
				});
			}
		}
		return out;
	});

	// --- hardware -------------------------------------------------------------
	const symbols = $derived.by(() => {
		if (!app.showHardware || !app.plan) return [] as HardwareSymbol[];
		return hardwareSymbols(app.plan, parts, offsets, (id) => app.libraries?.hardware.items[id], app.explode > 0);
	});
	function symbolColour(s: HardwareSymbol, prim: Prim): string {
		const sel = app.selectedFastener;
		if (sel && sel.joint === s.joint.id && sel.index === s.index) return '#ff3b1f';
		// The selected part's hardware lights up with it.
		if (app.selectedPart && (s.joint.edgePart === app.selectedPart || s.joint.facePart === app.selectedPart)) return '#ff8c42';
		return prim.colour;
	}
	const UP = new THREE.Vector3(0, 1, 0);
	function quatAlong(axis: Vec3): [number, number, number, number] {
		const q = new THREE.Quaternion().setFromUnitVectors(UP, new THREE.Vector3(...toThree(axis)).normalize());
		return [q.x, q.y, q.z, q.w];
	}
	/** Every leader of the exploded view in one geometry. */
	const leaders = $derived.by(() => {
		const pts: number[] = [];
		for (const s of symbols) {
			for (const [a, b] of s.leaders) pts.push(...toThree(a), ...toThree(b));
		}
		const g = new THREE.BufferGeometry();
		g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
		return g;
	});

	// --- parts ------------------------------------------------------------------
	function colourOf(part: Part): string {
		if (part.id === app.selectedPart) return '#ff8c42';
		// A part with a finding shows it: red for what blocks or should be
		// fixed, amber for what is probably wrong.
		const level = app.severityOfPart(part.id);
		if (level === 'ERROR' || level === 'FATAL') return '#d95f4b';
		if (level === 'WARNING') return '#e3c25c';
		if (part.material.startsWith('hdf')) return '#8a6a4a';
		// Fronts (doors, drawer fronts) lighter than the carcass, so the
		// furniture reads at a glance.
		if (part.role.includes('door') || (part.role.endsWith('_front') && !part.role.includes('box_front'))) return '#eadbc0';
		if (part.role.includes('drawer')) return '#c9b18f';
		return '#c8a67a';
	}

	function centre(part: Part): [number, number, number] {
		return toThree([
			(part.aabb.min[0] + part.aabb.max[0]) / 2,
			(part.aabb.min[1] + part.aabb.max[1]) / 2,
			(part.aabb.min[2] + part.aabb.max[2]) / 2
		]);
	}
	function size3(part: Part): [number, number, number] {
		return toThree([
			part.aabb.max[0] - part.aabb.min[0],
			part.aabb.max[1] - part.aabb.min[1],
			part.aabb.max[2] - part.aabb.min[2]
		]);
	}
	/** One box per part, shared by its faces and its outline. */
	const boxes = $derived(new Map(parts.map((p) => [p.id, new THREE.BoxGeometry(...size3(p))])));

	type Hole = { pos: [number, number, number]; quat: [number, number, number, number]; r: number; len: number };
	type Slot = { pos: [number, number, number]; size: [number, number, number] };

	function holesOf(part: Part): Hole[] {
		const out: Hole[] = [];
		for (const op of part.operations) {
			if (op.type !== 'DRILL') continue;
			const n = faceNormalWorld(part.placement, op.face);
			const depth = op.depth ?? (op.face === 'front' || op.face === 'back' ? part.dims.thickness : 0);
			const surface = faceUvToWorld(part, op.face, op.u, op.v);
			// Sits half inside the panel; the visible half marks the hole mouth.
			const c = [
				surface[0] - (n[0] * depth) / 2 + n[0] * 0.2,
				surface[1] - (n[1] * depth) / 2 + n[1] * 0.2,
				surface[2] - (n[2] * depth) / 2 + n[2] * 0.2
			] as const;
			out.push({
				pos: toThree([c[0], c[1], c[2]]),
				quat: quatAlong(n),
				r: op.diameter / 2,
				len: depth + 0.4
			});
		}
		return out;
	}

	function slotsOf(part: Part): Slot[] {
		const out: Slot[] = [];
		for (const op of part.operations) {
			if (op.type !== 'GROOVE') continue;
			const a = faceUvToWorld(part, op.face, op.from[0], op.from[1]);
			const b = faceUvToWorld(part, op.face, op.to[0], op.to[1]);
			const n = faceNormalWorld(part.placement, op.face);
			const mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
			const d = [Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]), Math.abs(b[2] - a[2])];
			const s: number[] = [0, 0, 0];
			for (let i = 0; i < 3; i++) {
				if (n[i] !== 0) s[i] = op.depth;
				else if (d[i] > 0.001) s[i] = d[i];
				else s[i] = op.width;
			}
			const c = [mid[0] - (n[0] * op.depth) / 2 + n[0] * 0.2, mid[1] - (n[1] * op.depth) / 2 + n[1] * 0.2, mid[2] - (n[2] * op.depth) / 2 + n[2] * 0.2];
			out.push({ pos: toThree([c[0], c[1], c[2]]), size: toThree([s[0], s[1], s[2]]) });
		}
		return out;
	}
</script>

<!-- A near plane in proportion to the scene: at 0.1 mm the depth buffer
     cannot tell a hole mouth from the face it sits in and the faces flicker. -->
{#if ortho}
	<T.OrthographicCamera makeDefault bind:ref={orthoCam} near={-distance * 4} far={distance * 8} position={[target[0], target[1], target[2] + distance * 3]}>
		<OrbitControls bind:ref={controls} {target} enableDamping />
	</T.OrthographicCamera>
{:else}
	<T.PerspectiveCamera makeDefault bind:ref={persp} position={[target[0] + distance * 0.7, target[1] + distance * 0.45, target[2] + distance]} fov={40} near={Math.max(2, distance / 200)} far={distance * 20}>
		<OrbitControls bind:ref={controls} {target} enableDamping />
	</T.PerspectiveCamera>
{/if}

<T.AmbientLight intensity={0.7} />
<T.DirectionalLight position={[2000, 4000, 3000]} intensity={1.4} />
<T.DirectionalLight position={[-2000, 1000, -1500]} intensity={0.5} />

{#each rails as r (r.id)}
	<T.Mesh position={r.pos} rotation={[0, 0, Math.PI / 2]}>
		<T.CylinderGeometry args={[12, 12, r.len, 12]} />
		<T.MeshStandardMaterial color="#9aa0a6" metalness={0.6} roughness={0.35} />
	</T.Mesh>
{/each}

<T.GridHelper args={[Math.max(bounds[0], bounds[1]) * 2, 20, '#bbb', '#ddd']} position={[bounds[0] / 2, floor - 1, bounds[1] / 2]} />

{#each parts as part (part.id)}
	{@const box = boxes.get(part.id)}
	<T.Group position={offsetOf(part)}>
		<T.Mesh position={centre(part)} geometry={box} onclick={(e: { stopPropagation: () => void }) => { e.stopPropagation(); app.selectPart(part.id); }}>
			<T.MeshStandardMaterial color={colourOf(part)} roughness={0.8} />
		</T.Mesh>
		<!-- The outline makes the elevations readable: face against face
		     of the same colour has no edge otherwise. -->
		<T.LineSegments position={centre(part)}>
			<T.EdgesGeometry args={[box]} />
			<T.LineBasicMaterial color="#3a3025" transparent opacity={0.35} />
		</T.LineSegments>
		{#if app.showHoles}
			{#each holesOf(part) as hole, i (part.id + ':h' + i)}
				<T.Mesh position={hole.pos} quaternion={hole.quat}>
					<T.CylinderGeometry args={[hole.r, hole.r, hole.len, 16]} />
					<T.MeshStandardMaterial color="#2b2b2b" />
				</T.Mesh>
			{/each}
			{#each slotsOf(part) as slot, i (part.id + ':g' + i)}
				<T.Mesh position={slot.pos}>
					<T.BoxGeometry args={slot.size} />
					<T.MeshStandardMaterial color="#3a3a3a" />
				</T.Mesh>
			{/each}
		{/if}
	</T.Group>
{/each}

{#each symbols as s (s.key)}
	{#each s.prims as prim, i (s.key + ':' + i)}
		{@const colour = symbolColour(s, prim)}
		{#if prim.shape === 'cyl'}
			<T.Mesh position={toThree(prim.pos)} quaternion={quatAlong(prim.axis)} onclick={(e: { stopPropagation: () => void }) => { e.stopPropagation(); app.selectFastener(s.joint.id, s.index); }}>
				<T.CylinderGeometry args={[prim.r, prim.r, prim.len, 20]} />
				<T.MeshStandardMaterial color={colour} metalness={0.5} roughness={0.4} />
			</T.Mesh>
		{:else if prim.shape === 'box'}
			<T.Mesh position={toThree(prim.pos)} onclick={(e: { stopPropagation: () => void }) => { e.stopPropagation(); app.selectFastener(s.joint.id, s.index); }}>
				<T.BoxGeometry args={toThree(prim.size)} />
				<T.MeshStandardMaterial color={colour} metalness={0.5} roughness={0.4} />
			</T.Mesh>
		{:else}
			<T.Mesh position={toThree(prim.pos)} onclick={(e: { stopPropagation: () => void }) => { e.stopPropagation(); app.selectFastener(s.joint.id, s.index); }}>
				<T.SphereGeometry args={[prim.r, 16, 12]} />
				<T.MeshStandardMaterial color={colour} metalness={0.5} roughness={0.4} />
			</T.Mesh>
		{/if}
	{/each}
{/each}

{#if app.explode > 0}
	<T.LineSegments geometry={leaders}>
		<T.LineBasicMaterial color="#4b5563" transparent opacity={0.7} />
	</T.LineSegments>
{/if}
