<script lang="ts">
	/**
	 * Many copies of one shape in a single draw call. `fill` writes every
	 * instance's matrix (and colour); it runs again whenever what it reads
	 * changes, so moving a thousand holes is a loop over a typed array, not
	 * a thousand scene objects.
	 *
	 * The mesh is made here and only remade when the count changes; the old
	 * one gives its GPU buffers back. (Letting Threlte rebuild it from
	 * `args` made a new one on every recompile and never freed the last.)
	 */
	import { T } from '@threlte/core';
	import * as THREE from 'three';

	let {
		geometry,
		material,
		count,
		fill,
		onclick
	}: {
		geometry: THREE.BufferGeometry;
		material: THREE.Material;
		count: number;
		fill: (mesh: THREE.InstancedMesh) => void;
		onclick?: (instance: number) => void;
	} = $props();

	const mesh = $derived.by(() => {
		const m = new THREE.InstancedMesh(geometry, material, Math.max(count, 1));
		m.count = count;
		m.frustumCulled = false;
		return m;
	});
	// InstancedMesh.dispose frees the instance buffers, not the shared
	// geometry and material.
	$effect(() => {
		const m = mesh;
		return () => m.dispose();
	});

	$effect(() => {
		const m = mesh;
		fill(m);
		m.instanceMatrix.needsUpdate = true;
		if (m.instanceColor) m.instanceColor.needsUpdate = true;
		m.computeBoundingSphere();
		m.computeBoundingBox();
	});

	function click(e: { stopPropagation: () => void; instanceId?: number }) {
		if (!onclick || e.instanceId === undefined) return;
		e.stopPropagation();
		onclick(e.instanceId);
	}
</script>

{#if count > 0}
	<T is={mesh} onclick={onclick ? click : undefined} />
{/if}
