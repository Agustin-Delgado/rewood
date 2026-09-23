/**
 * Openings, animated: a door swings on the line its hinges give it, a
 * drawer runs out on its slides, and everything screwed to them (handle,
 * hinge cup, the drawer's rail) goes along. Nothing here is fabrication:
 * the plan says which part hangs on which hinge or slide, and the viewer
 * only moves what is already there.
 */
import * as THREE from 'three';
import type { ManufacturingPlan, Part, Vec3 } from '@rewood/engine/browser';
import { toThree } from './geometry';

/** How far a door opens and how much of its box a drawer shows. */
const DOOR_ANGLE = (95 * Math.PI) / 180;
const DRAWER_OUT = 0.75;

export type Motion =
	| { key: string; kind: 'door'; pivot: Vec3; axis: Vec3; angle: number }
	| { key: string; kind: 'drawer'; travel: Vec3 };

function centre(p: Part): Vec3 {
	return [(p.aabb.min[0] + p.aabb.max[0]) / 2, (p.aabb.min[1] + p.aabb.max[1]) / 2, (p.aabb.min[2] + p.aabb.max[2]) / 2];
}

/** The part's thinnest horizontal axis: the way a front faces. */
function facingAxis(p: Part): 0 | 1 {
	return p.aabb.max[0] - p.aabb.min[0] < p.aabb.max[1] - p.aabb.min[1] ? 0 : 1;
}

const AXIS_INDEX: Record<string, number> = { x: 0, y: 1, z: 2 };

/** What moves when the furniture opens, by part id. */
export function motions(plan: ManufacturingPlan): Map<string, Motion> {
	const out = new Map<string, Motion>();
	const byId = new Map(plan.parts.map((p) => [p.id, p]));

	// Doors: the hinge side is where the panel it hangs on is; the door
	// turns on its outer edge there (front face, the way a full overlay
	// hinge swings it clear of the side). Fronts face +Y unless their
	// carcass is turned; the door's own thin axis says which way.
	for (const joint of plan.joints) {
		if (joint.kind !== 'hinge' || out.has(joint.edgePart)) continue;
		const door = byId.get(joint.edgePart);
		const side = byId.get(joint.facePart);
		if (!door || !side) continue;
		const along = AXIS_INDEX[joint.axis.slice(-1)];
		const fi = along === 2 ? facingAxis(door) : 1;
		const i = [0, 1, 2].find((k) => k !== along && k !== fi) ?? 0;
		const outwards = centre(door)[fi] >= centre(side)[fi] ? 1 : -1;
		const pivot: Vec3 = [0, 0, 0];
		pivot[fi] = outwards > 0 ? door.aabb.max[fi] : door.aabb.min[fi];
		pivot[i] = centre(side)[i] < centre(door)[i] ? door.aabb.min[i] : door.aabb.max[i];
		pivot[along] = centre(door)[along];
		const axis: Vec3 = [0, 0, 0];
		axis[along] = 1;
		// Whichever way brings the door's middle out to the front, tried in
		// three.js space, where it is applied (Y and Z swapped mirror it).
		const probe = (angle: number) => {
			const m = rotation(pivot, axis, angle);
			return new THREE.Vector3(...toThree(centre(door))).applyMatrix4(m).getComponent(toThree([0, 1, 2])[fi]) * outwards;
		};
		const angle = probe(DOOR_ANGLE) > probe(-DOOR_ANGLE) ? DOOR_ANGLE : -DOOR_ANGLE;
		// Keyed by what it is, not by its id: ids renumber when a recompile
		// adds or drops parts, and an open door must stay that door.
		out.set(door.id, { key: `${door.component}:${door.role}`, kind: 'door', pivot, axis, angle });
	}

	// Sliding doors: the ones on the front lane (every second one) run over
	// the door before them, as far as their width less the overlap.
	const sliding = new Map<string, Map<number, Part>>();
	for (const p of plan.parts) {
		const m = p.role.match(/^sliding_door_(\d+)$/);
		if (!m) continue;
		const set = sliding.get(p.component) ?? new Map<number, Part>();
		set.set(Number(m[1]), p);
		sliding.set(p.component, set);
	}
	for (const set of sliding.values()) {
		for (const [n, door] of set) {
			const before = set.get(n - 1);
			if (n % 2 !== 0 || !before) continue;
			const wi = facingAxis(door) === 0 ? 1 : 0;
			const width = door.aabb.max[wi] - door.aabb.min[wi];
			const overlap = Math.max(0, Math.min(door.aabb.max[wi], before.aabb.max[wi]) - Math.max(door.aabb.min[wi], before.aabb.min[wi]));
			const travel: Vec3 = [0, 0, 0];
			travel[wi] = Math.sign(centre(before)[wi] - centre(door)[wi]) * (width - overlap);
			out.set(door.id, { key: `${door.component}:${door.role}`, kind: 'drawer', travel });
		}
	}

	// Drawers: every part of one drawer (front, box, bottom) runs out
	// together, as far as a good part of its box, the way its front faces.
	const drawers = new Map<string, Part[]>();
	for (const p of plan.parts) {
		const m = p.role.match(/^(.*drawer_\d+)_/);
		if (!m) continue;
		const key = `${p.component}:${m[1]}`;
		drawers.set(key, [...(drawers.get(key) ?? []), p]);
	}
	for (const [key, group] of drawers) {
		const front = group.find((p) => p.role.endsWith('_front') && !p.role.endsWith('box_front'));
		const box = group.filter((p) => p !== front);
		if (!front || box.length === 0) continue;
		const fi = facingAxis(front);
		const boxCentre = box.map(centre).reduce((a, c) => a + c[fi], 0) / box.length;
		const outwards = centre(front)[fi] >= boxCentre ? 1 : -1;
		const depth = Math.max(0, ...box.map((p) => p.aabb.max[fi] - p.aabb.min[fi]));
		const travel: Vec3 = [0, 0, 0];
		travel[fi] = outwards * depth * DRAWER_OUT;
		// Behind a sliding door that stays where it is, a drawer does not
		// come out: it would run through the door.
		const wi = fi === 0 ? 1 : 0;
		const blocked = [...sliding.values()].some((set) =>
			[...set.values()].some((d) => {
				const m = out.get(d.id);
				const shift = m?.kind === 'drawer' ? m.travel[wi] : 0;
				const cover = Math.min(d.aabb.max[wi] + shift, front.aabb.max[wi]) - Math.max(d.aabb.min[wi] + shift, front.aabb.min[wi]);
				const levels = d.aabb.min[2] < front.aabb.max[2] && d.aabb.max[2] > front.aabb.min[2];
				return levels && cover > (front.aabb.max[wi] - front.aabb.min[wi]) / 4;
			})
		);
		if (blocked) continue;
		for (const p of group) out.set(p.id, { key, kind: 'drawer', travel });
	}
	return out;
}

/** Three.js rotation of `angle` about a furniture-space line. */
function rotation(pivot: Vec3, axis: Vec3, angle: number): THREE.Matrix4 {
	const p = new THREE.Vector3(...toThree(pivot));
	const a = new THREE.Vector3(...toThree(axis)).normalize();
	return new THREE.Matrix4()
		.makeTranslation(p.x, p.y, p.z)
		.multiply(new THREE.Matrix4().makeRotationAxis(a, angle))
		.multiply(new THREE.Matrix4().makeTranslation(-p.x, -p.y, -p.z));
}

/** Where a part is drawn: opened by `t` (0 shut, 1 open), then exploded by `offset`. */
export function partMatrix(motion: Motion | undefined, t: number, offset: Vec3 | undefined): THREE.Matrix4 {
	const m = new THREE.Matrix4();
	if (offset) m.makeTranslation(...toThree(offset));
	if (!motion || t <= 0) return m;
	if (motion.kind === 'door') return m.multiply(rotation(motion.pivot, motion.axis, motion.angle * t));
	const [x, y, z] = motion.travel;
	return m.multiply(new THREE.Matrix4().makeTranslation(...toThree([x * t, y * t, z * t])));
}
