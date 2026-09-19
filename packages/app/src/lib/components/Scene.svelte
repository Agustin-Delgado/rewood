<script lang="ts">
	/**
	 * Every part as a box at its furniture-space AABB, every hole as a short
	 * cylinder into its face and every groove as a slot. Nothing here is
	 * geometry of its own: it is the plan, drawn.
	 */
	import { T } from '@threlte/core';
	import { OrbitControls, interactivity } from '@threlte/extras';
	import * as THREE from 'three';
	import type { Part } from '@rewood/engine/browser';
	import { app } from '$lib/state.svelte';
	import { explodeOffsets, faceNormalWorld, faceUvToWorld, toThree } from '$lib/geometry';

	interactivity();

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
	const target = $derived<[number, number, number]>([bounds[0] / 2, bounds[2] / 2, bounds[1] / 2]);
	const distance = $derived(Math.max(bounds[0], bounds[2]) * 1.6 + bounds[1]);

	function colourOf(part: Part): string {
		if (part.id === app.selectedPart) return '#ff8c42';
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
	function size(part: Part): [number, number, number] {
		return toThree([
			part.aabb.max[0] - part.aabb.min[0],
			part.aabb.max[1] - part.aabb.min[1],
			part.aabb.max[2] - part.aabb.min[2]
		]);
	}

	type Hole = { pos: [number, number, number]; quat: THREE.Quaternion; r: number; len: number };
	type Slot = { pos: [number, number, number]; size: [number, number, number] };

	const UP = new THREE.Vector3(0, 1, 0);
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
			const dir = new THREE.Vector3(...toThree([n[0], n[1], n[2]])).normalize();
			out.push({
				pos: toThree([c[0], c[1], c[2]]),
				quat: new THREE.Quaternion().setFromUnitVectors(UP, dir),
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

<T.PerspectiveCamera makeDefault position={[target[0] + distance * 0.7, target[1] + distance * 0.45, target[2] + distance]} fov={40} far={50000}>
	<OrbitControls {target} enableDamping />
</T.PerspectiveCamera>

<T.AmbientLight intensity={0.7} />
<T.DirectionalLight position={[2000, 4000, 3000]} intensity={1.4} />
<T.DirectionalLight position={[-2000, 1000, -1500]} intensity={0.5} />

<T.GridHelper args={[Math.max(bounds[0], bounds[1]) * 2, 20, '#bbb', '#ddd']} position={[bounds[0] / 2, floor - 1, bounds[1] / 2]} />

{#each parts as part (part.id)}
	<T.Group position={offsetOf(part)}>
	<T.Mesh position={centre(part)} onclick={(e: { stopPropagation: () => void }) => { e.stopPropagation(); app.selectedPart = part.id; }}>
		<T.BoxGeometry args={size(part)} />
		<T.MeshStandardMaterial color={colourOf(part)} roughness={0.8} />
	</T.Mesh>
	{#if app.showHoles}
		{#each holesOf(part) as hole, i (part.id + ':h' + i)}
			<T.Mesh position={hole.pos} quaternion={[hole.quat.x, hole.quat.y, hole.quat.z, hole.quat.w]}>
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
