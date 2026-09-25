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
	import { motions, partMatrix, type Motion } from '$lib/motion';
	import Instances from './Instances.svelte';

	interactivity();
	const { size, dom, scene: threeScene, camera: threeCamera } = useThrelte();
	// The realistic photo reads the scene as it is drawn: open doors,
	// colours, hardware.
	$effect(() => {
		app.photoSource = () => ({ scene: threeScene, camera: threeCamera.current });
		return () => (app.photoSource = null);
	});

	// Selection: a click picks a part, a click on it again drops it (not
	// the second click of a double click, which opens the front), and a
	// click on empty space drops whatever was picked. A drag orbits the
	// view and changes nothing.
	let hit = false;
	function pickPart(id: string, e: MouseEvent) {
		if (e.detail > 1) return;
		app.selectPart(app.selectedPart === id ? null : id);
	}
	$effect(() => {
		const el = dom;
		let down: [number, number] | null = null;
		const onDown = (e: PointerEvent) => (down = [e.clientX, e.clientY]);
		const onClick = (e: MouseEvent) => {
			const still = down && Math.hypot(e.clientX - down[0], e.clientY - down[1]) < 4;
			// After the scene's own click handlers, whichever listener ran first.
			setTimeout(() => {
				if (!hit && still) app.selectPart(null);
				hit = false;
			});
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape' && !app.catalogOpen) app.selectPart(null);
		};
		el.addEventListener('pointerdown', onDown);
		el.addEventListener('click', onClick);
		window.addEventListener('keydown', onKey);
		return () => {
			el.removeEventListener('pointerdown', onDown);
			el.removeEventListener('click', onClick);
			window.removeEventListener('keydown', onKey);
		};
	});

	const parts = $derived(app.plan?.parts.filter((p) => !app.hiddenComponents.has(p.component)) ?? []);
	// Offsets from every part, hidden ones included, so hiding a component
	// does not move the rest.
	const offsets = $derived(explodeOffsets(app.plan?.parts ?? [], app.explode));

	const bounds = $derived.by(() => {
		let max = [1, 1, 1];
		for (const p of app.plan?.parts ?? []) {
			max = [Math.max(max[0], p.aabb.max[0]), Math.max(max[1], p.aabb.max[1]), Math.max(max[2], p.aabb.max[2])];
		}
		return max;
	});
	/** Lowest Z: the floor sits under the legs, not under the carcass. */
	const floor = $derived((app.plan?.parts ?? []).reduce((m, p) => Math.min(m, p.aabb.min[2]), 0));
	/** Under the lowest part as drawn: exploded legs go below the assembled floor. */
	const gridFloor = $derived(
		(app.plan?.parts ?? []).reduce((m, p) => Math.min(m, p.aabb.min[2] + (offsets.get(p.id)?.[2] ?? 0)), floor)
	);
	// The camera reads plain numbers: a recompile that leaves the size alone
	// (a new colour) makes a new `bounds` array with the same values, and an
	// array would move the camera back to where it started.
	const bx = $derived(bounds[0]);
	const by = $derived(bounds[1]);
	const bz = $derived(bounds[2]);
	const target = $derived<[number, number, number]>([bx / 2, (bz + floor) / 2, by / 2]);
	const distance = $derived(Math.max(bx, bz) * 1.6 + by);

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
		const out: { id: string; a: Vec3; pa: string; b: Vec3; pb: string }[] = [];
		const joints = (app.plan?.joints ?? []).filter((j) => j.kind === 'fixture' && j.hardware.some((h) => h.startsWith('rail_support')));
		const byComponent = new Map<string, typeof joints>();
		for (const j of joints) byComponent.set(j.component, [...(byComponent.get(j.component) ?? []), j]);
		for (const [component, list] of byComponent) {
			if (app.hiddenComponents.has(component)) continue;
			for (let i = 0; i + 1 < list.length; i += 2) {
				const a = list[i].fasteners[0]?.position;
				const b = list[i + 1].fasteners[0]?.position;
				if (!a || !b) continue;
				// Hangs from the two panels its supports are screwed to.
				const [pa, pb] = [list[i].facePart, list[i + 1].facePart];
				if (!parts.some((p) => p.id === pa) || !parts.some((p) => p.id === pb)) continue;
				out.push({ id: `${component}:${i}`, a, pa, b, pb });
			}
		}
		return out;
	});
	/** A rail bar between its two supports, wherever their panels are drawn. */
	function railPose(r: { a: Vec3; pa: string; b: Vec3; pb: string }) {
		const a = new THREE.Vector3(...toThree(r.a)).applyMatrix4(matrixOf(r.pa));
		const b = new THREE.Vector3(...toThree(r.b)).applyMatrix4(matrixOf(r.pb));
		const q = new THREE.Quaternion().setFromUnitVectors(UP, b.clone().sub(a).normalize());
		return { pos: a.clone().add(b).multiplyScalar(0.5).toArray() as [number, number, number], quat: [q.x, q.y, q.z, q.w] as [number, number, number, number], len: a.distanceTo(b) };
	}

	// --- where every part is drawn ----------------------------------------------
	// Exploded offset, then the opening (a door turning on its hinge line,
	// a drawer on its slides). Holes and hardware ride on their part's
	// matrix, so nothing below recomputes geometry when the view moves.
	const moves = $derived(app.plan ? motions(app.plan) : new Map<string, Motion>());

	// Doors and drawers ease to where they were asked to go.
	const opening = $state<{ progress: Record<string, number> }>({ progress: {} });
	$effect(() => {
		const want = new Map<string, number>();
		for (const m of moves.values()) want.set(m.key, app.openAll || app.opened.has(m.key) ? 1 : 0);
		let frame = 0;
		let last = performance.now();
		const step = (now: number) => {
			const dt = Math.min(0.05, Math.max(0, now - last) / 1000);
			last = now;
			let moving = false;
			const next: Record<string, number> = {};
			for (const [key, target] of want) {
				const cur = untrack(() => opening.progress[key]) ?? 0;
				const v = cur < target ? Math.min(target, cur + dt * 2.2) : Math.max(target, cur - dt * 2.2);
				next[key] = v;
				if (v !== target) moving = true;
			}
			opening.progress = next;
			if (moving) frame = requestAnimationFrame(step);
		};
		frame = requestAnimationFrame(step);
		return () => cancelAnimationFrame(frame);
	});

	const matrices = $derived.by(() => {
		const out = new Map<string, THREE.Matrix4>();
		for (const p of app.plan?.parts ?? []) {
			const m = moves.get(p.id);
			out.set(p.id, partMatrix(m, m ? (opening.progress[m.key] ?? 0) : 0, offsets.get(p.id)));
		}
		return out;
	});
	const IDENTITY = new THREE.Matrix4();
	const matrixOf = (id: string) => matrices.get(id) ?? IDENTITY;
	function pose(id: string): { position: [number, number, number]; quaternion: [number, number, number, number] } {
		const p = new THREE.Vector3();
		const q = new THREE.Quaternion();
		matrixOf(id).decompose(p, q, new THREE.Vector3());
		return { position: [p.x, p.y, p.z], quaternion: [q.x, q.y, q.z, q.w] };
	}

	// --- hardware -------------------------------------------------------------
	// Laid out once per plan on the assembled furniture; the matrices above
	// place it.
	const symbols = $derived.by(() => {
		if (!app.showHardware || !app.plan) return [] as HardwareSymbol[];
		return hardwareSymbols(app.plan, parts, (id) => app.libraries?.hardware.items[id]);
	});
	type Placed = { sym: HardwareSymbol; prim: Prim; local: THREE.Matrix4 };
	const UP = new THREE.Vector3(0, 1, 0);
	const OUT = new THREE.Vector3(0, 0, 1);
	/** Turns a flat shape's normal (+Z) to face along `axis`. */
	function facing(axis: Vec3): THREE.Quaternion {
		return new THREE.Quaternion().setFromUnitVectors(OUT, new THREE.Vector3(...toThree(axis)).normalize());
	}
	function along(axis: Vec3): THREE.Quaternion {
		return new THREE.Quaternion().setFromUnitVectors(UP, new THREE.Vector3(...toThree(axis)).normalize());
	}
	const placed = $derived.by(() => {
		const out: Record<Prim['shape'], Placed[]> = { cyl: [], box: [], sphere: [] };
		for (const sym of symbols) {
			for (const prim of sym.prims) {
				const pos = new THREE.Vector3(...toThree(prim.pos));
				const local =
					prim.shape === 'cyl'
						? new THREE.Matrix4().compose(pos, along(prim.axis), new THREE.Vector3(prim.r, prim.len, prim.r))
						: prim.shape === 'box'
							? new THREE.Matrix4().compose(pos, new THREE.Quaternion(), new THREE.Vector3(...toThree(prim.size)))
							: new THREE.Matrix4().compose(pos, new THREE.Quaternion(), new THREE.Vector3(prim.r, prim.r, prim.r));
				out[prim.shape].push({ sym, prim, local });
			}
		}
		return out;
	});
	/** A bridging body (a dowel) sits halfway between its two parts. */
	function ownerMatrix(sym: HardwareSymbol, owner: string | null): THREE.Matrix4 {
		if (owner) return matrixOf(owner);
		const a = new THREE.Vector3().setFromMatrixPosition(matrixOf(sym.joint.edgePart));
		const b = new THREE.Vector3().setFromMatrixPosition(matrixOf(sym.joint.facePart));
		return new THREE.Matrix4().makeTranslation(a.add(b).multiplyScalar(0.5));
	}
	function symbolColour(s: HardwareSymbol, prim: Prim): string {
		const sel = app.selectedFastener;
		if (sel && sel.joint === s.joint.id && sel.index === s.index) return '#ff3b1f';
		// The selected part's hardware lights up with it.
		if (app.selectedPart && (s.joint.edgePart === app.selectedPart || s.joint.facePart === app.selectedPart)) return '#ff8c42';
		return prim.colour;
	}
	const scratch = new THREE.Matrix4();
	const tint = new THREE.Color();
	function fillPrims(list: Placed[]) {
		return (mesh: THREE.InstancedMesh) => {
			list.forEach((x, i) => {
				mesh.setMatrixAt(i, scratch.multiplyMatrices(ownerMatrix(x.sym, x.prim.owner), x.local));
				mesh.setColorAt(i, tint.set(symbolColour(x.sym, x.prim)));
			});
		};
	}
	function pickPrim(list: Placed[]) {
		return (i: number) => {
			const x = list[i];
			hit = true;
			if (x) app.selectFastener(x.sym.joint.id, x.sym.index);
		};
	}
	const unitCyl = new THREE.CylinderGeometry(1, 1, 1, 20);
	const unitBox = new THREE.BoxGeometry(1, 1, 1);
	const unitSphere = new THREE.SphereGeometry(1, 16, 12);
	const unitEdges = new THREE.EdgesGeometry(unitBox);
	// A hole reads as its mouth: a flat dark disc on the face, unlit so it
	// is the same black from any angle, lifted a hair off the face and
	// pushed a fixed step towards the camera in depth so the face never
	// shows through it. No slope factor: a disc seen exactly edge-on has an
	// infinite depth slope, and a slope-scaled offset pulled such discs in
	// front of the doors as thin dashes.
	const holeDisc = new THREE.CircleGeometry(1, 24);
	const railCyl = new THREE.CylinderGeometry(1, 1, 1, 12);
	const metal = new THREE.MeshStandardMaterial({ metalness: 0.5, roughness: 0.4 });
	const holeMaterial = new THREE.MeshBasicMaterial({
		color: '#ffffff',
		polygonOffset: true,
		polygonOffsetFactor: 0,
		polygonOffsetUnits: -2
	});
	// A groove the same way: a flat dark strip on its face.
	const grooveMaterial = new THREE.MeshBasicMaterial({
		color: '#ffffff',
		polygonOffset: true,
		polygonOffsetFactor: 0,
		polygonOffsetUnits: -2
	});
	// A cutout goes through the panel: a solid body, lit like one.
	const cutoutMaterial = new THREE.MeshStandardMaterial({ color: '#3a3a3a' });

	/** Every leader of the exploded view in one geometry. */
	const leaders = $derived.by(() => {
		const pts: number[] = [];
		if (app.explode > 0) {
			const v = new THREE.Vector3();
			for (const s of symbols) {
				for (const l of s.leaders) {
					v.set(...toThree(l.a)).applyMatrix4(matrixOf(l.pa));
					pts.push(v.x, v.y, v.z);
					v.set(...toThree(l.b)).applyMatrix4(matrixOf(l.pb));
					pts.push(v.x, v.y, v.z);
				}
			}
		}
		const g = new THREE.BufferGeometry();
		g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
		return g;
	});
	// Every slider step makes a new one: the last one's GPU buffer goes.
	$effect(() => {
		const g = leaders;
		return () => g.dispose();
	});
	/** The floor grid, remade only when the furniture's footprint changes. */
	const gridSize = $derived(Math.round(Math.max(bounds[0], bounds[1]) * 2));
	const grid = $derived(new THREE.GridHelper(gridSize, 20, '#bbb', '#ddd'));
	$effect(() => {
		const g = grid;
		return () => {
			g.geometry.dispose();
			(g.material as THREE.Material).dispose();
		};
	});

	// --- parts ------------------------------------------------------------------
	function colourOf(part: Part): string {
		if (part.id === app.selectedPart) return '#ff8c42';
		// A part with a finding shows it: red for what blocks or should be
		// fixed, amber for what is probably wrong.
		const level = app.severityOfPart(part.id);
		if (level === 'ERROR' || level === 'FATAL') return '#d95f4b';
		if (level === 'WARNING') return '#e3c25c';
		// The board's colour, when the person chose one (or the default).
		const decor = part.decor ? app.libraries?.materials.decors[part.decor] : undefined;
		if (decor) return decor.hex;
		// The 3 mm back is white fibreboard (fibrofácil blanco), not brown.
		if (part.material.startsWith('hdf')) return '#e8e5de';
		if (part.material.startsWith('mirror')) return '#c9d6dc';
		if (part.material.startsWith('glass')) return '#bfe3ea';
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

	/** A double click on a door or a drawer opens or shuts it. */
	function toggleFront(part: Part, e: { stopPropagation: () => void }) {
		const m = moves.get(part.id);
		if (!m) return;
		e.stopPropagation();
		app.toggleOpen(m.key, [...new Set([...moves.values()].map((x) => x.key))]);
	}

	// --- holes and grooves --------------------------------------------------------
	// One matrix per mark on its part's assembled frame, computed per plan.
	type Mark = { part: string; local: THREE.Matrix4; colour?: THREE.Color };
	// A hole or groove mark stands out from its panel: dark on a light
	// board, the raw chipboard core's tan on a dark one (which is what a
	// drilled hole in black melamine shows).
	const DARK_MARK = new THREE.Color('#1f1f1f');
	const LIGHT_MARK = new THREE.Color('#c9ad85');
	function markColour(part: Part): THREE.Color {
		const c = new THREE.Color(colourOf(part));
		return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b > 0.25 ? DARK_MARK : LIGHT_MARK;
	}
	const holes = $derived.by(() => {
		const out: Mark[] = [];
		if (!app.showHoles) return out;
		for (const part of parts) {
			for (const op of part.operations) {
				if (op.type !== 'DRILL') continue;
				const n = faceNormalWorld(part.placement, op.face);
				const r = op.diameter / 2;
				const disc = (at: Vec3, normal: Vec3) => {
					const lift: Vec3 = [at[0] + normal[0] * 0.15, at[1] + normal[1] * 0.15, at[2] + normal[2] * 0.15];
					out.push({
						part: part.id,
						local: new THREE.Matrix4().compose(new THREE.Vector3(...toThree(lift)), facing(normal), new THREE.Vector3(r, r, 1)),
						colour: markColour(part)
					});
				};
				disc(faceUvToWorld(part, op.face, op.u, op.v), n);
				// A through hole shows on the other face as well.
				if (op.through && (op.face === 'front' || op.face === 'back')) {
					const other = op.face === 'front' ? 'back' : 'front';
					disc(faceUvToWorld(part, other, op.u, op.v), [-n[0], -n[1], -n[2]]);
				}
			}
		}
		return out;
	});
	const grooves = $derived.by(() => {
		const out: Mark[] = [];
		if (!app.showHoles) return out;
		for (const part of parts) {
			for (const op of part.operations) {
				if (op.type !== 'GROOVE') continue;
				const a = faceUvToWorld(part, op.face, op.from[0], op.from[1]);
				const b = faceUvToWorld(part, op.face, op.to[0], op.to[1]);
				const n = faceNormalWorld(part.placement, op.face);
				const mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
				const d = [Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]), Math.abs(b[2] - a[2])];
				const s: Vec3 = [0, 0, 0];
				for (let i = 0; i < 3; i++) {
					if (n[i] !== 0) s[i] = 0.1;
					else if (d[i] > 0.001) s[i] = d[i];
					else s[i] = op.width;
				}
				const c: Vec3 = [mid[0] + n[0] * 0.15, mid[1] + n[1] * 0.15, mid[2] + n[2] * 0.15];
				out.push({
					part: part.id,
					local: new THREE.Matrix4().compose(new THREE.Vector3(...toThree(c)), new THREE.Quaternion(), new THREE.Vector3(...toThree(s))),
					colour: markColour(part)
				});
			}
		}
		return out;
	});
	const cutouts = $derived.by(() => {
		const out: Mark[] = [];
		if (!app.showHoles) return out;
		for (const part of parts) {
			// A cutout: a box through the panel, the size of the opening.
			for (const op of part.operations) {
				if (op.type !== 'CUTOUT') continue;
				const c = faceUvToWorld(part, op.face, op.u, op.v);
				const n = faceNormalWorld(part.placement, op.face);
				const e = faceUvToWorld(part, op.face, op.u + 1, op.v);
				const du = [e[0] - c[0], e[1] - c[1], e[2] - c[2]].map(Math.abs);
				const t = part.dims.thickness;
				const s: Vec3 = [0, 0, 0];
				for (let i = 0; i < 3; i++) s[i] = n[i] !== 0 ? t + 0.6 : du[i] > 0.5 ? op.width : op.height;
				const m: Vec3 = [c[0] - (n[0] * t) / 2, c[1] - (n[1] * t) / 2, c[2] - (n[2] * t) / 2];
				out.push({
					part: part.id,
					local: new THREE.Matrix4().compose(new THREE.Vector3(...toThree(m)), new THREE.Quaternion(), new THREE.Vector3(...toThree(s)))
				});
			}
		}
		return out;
	});
	function fillMarks(list: Mark[]) {
		return (mesh: THREE.InstancedMesh) => {
			list.forEach((x, i) => {
				mesh.setMatrixAt(i, scratch.multiplyMatrices(matrixOf(x.part), x.local));
				if (x.colour) mesh.setColorAt(i, x.colour);
			});
		};
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
	{@const at = railPose(r)}
	<T.Mesh position={at.pos} quaternion={at.quat} scale={[12, at.len, 12]} geometry={railCyl} userData={{ tag: 'rail' }}>
		<T.MeshStandardMaterial color="#9aa0a6" metalness={0.6} roughness={0.35} />
	</T.Mesh>
{/each}

<T is={grid} position={[bounds[0] / 2, gridFloor - 1, bounds[1] / 2]} />

{#each parts as part (part.id)}
	{@const at = pose(part.id)}
	<T.Group position={at.position} quaternion={at.quaternion}>
		<T.Mesh position={centre(part)} scale={size3(part)} geometry={unitBox} userData={{ part: part.id }} onclick={(e: { stopPropagation: () => void; nativeEvent: MouseEvent }) => { e.stopPropagation(); hit = true; pickPart(part.id, e.nativeEvent); }} ondblclick={(e: { stopPropagation: () => void }) => toggleFront(part, e)}>
			{#if part.outsourced && part.material.startsWith('glass')}
				<!-- Glass: seen through, so the shelves behind the doors read. -->
				<T.MeshStandardMaterial color={colourOf(part)} roughness={0.1} metalness={0.1} transparent opacity={0.35} depthWrite={false} />
			{:else if part.outsourced}
				<T.MeshStandardMaterial color={colourOf(part)} roughness={0.05} metalness={0.8} />
			{:else}
				<T.MeshStandardMaterial color={colourOf(part)} roughness={0.8} />
			{/if}
		</T.Mesh>
		<!-- The outline makes the elevations readable: face against face
		     of the same colour has no edge otherwise. -->
		<T.LineSegments position={centre(part)} scale={size3(part)} geometry={unitEdges}>
			<T.LineBasicMaterial color="#3a3025" transparent opacity={0.35} />
		</T.LineSegments>
	</T.Group>
{/each}

<Instances geometry={holeDisc} material={holeMaterial} count={holes.length} fill={fillMarks(holes)} />
<Instances geometry={unitBox} material={grooveMaterial} count={grooves.length} fill={fillMarks(grooves)} />
<Instances geometry={unitBox} material={cutoutMaterial} count={cutouts.length} fill={fillMarks(cutouts)} />
<Instances tag="hardware" geometry={unitCyl} material={metal} count={placed.cyl.length} fill={fillPrims(placed.cyl)} onclick={pickPrim(placed.cyl)} />
<Instances tag="hardware" geometry={unitBox} material={metal} count={placed.box.length} fill={fillPrims(placed.box)} onclick={pickPrim(placed.box)} />
<Instances tag="hardware" geometry={unitSphere} material={metal} count={placed.sphere.length} fill={fillPrims(placed.sphere)} onclick={pickPrim(placed.sphere)} />

{#if app.explode > 0}
	<T.LineSegments geometry={leaders}>
		<T.LineBasicMaterial color="#4b5563" transparent opacity={0.7} />
	</T.LineSegments>
{/if}
