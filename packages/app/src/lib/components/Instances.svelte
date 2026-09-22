<script lang="ts">
	/**
	 * Many copies of one shape in a single draw call. `fill` writes every
	 * instance's matrix (and colour); it runs again whenever what it reads
	 * changes, so moving a thousand holes is a loop over a typed array, not
	 * a thousand scene objects.
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

	let mesh = $state<THREE.InstancedMesh>();

	$effect(() => {
		if (!mesh) return;
		fill(mesh);
		mesh.instanceMatrix.needsUpdate = true;
		if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
		mesh.computeBoundingSphere();
		mesh.computeBoundingBox();
	});

	function click(e: { stopPropagation: () => void; instanceId?: number }) {
		if (!onclick || e.instanceId === undefined) return;
		e.stopPropagation();
		onclick(e.instanceId);
	}
</script>

{#if count > 0}
	{#key count}
		<T.InstancedMesh args={[geometry, material, count]} bind:ref={mesh} onclick={onclick ? click : undefined} frustumCulled={false} />
	{/key}
{/if}
