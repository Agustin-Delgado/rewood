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
	| { key: string; kind: 'drawer'; travel: number };

function centre(p: Part): Vec3 {
	return [(p.aabb.min[0] + p.aabb.max[0]) / 2, (p.aabb.min[1] + p.aabb.max[1]) / 2, (p.aabb.min[2] + p.aabb.max[2]) / 2];
}

/** What moves when the furniture opens, by part id. */
export function motions(plan: ManufacturingPlan): Map<string, Motion> {
	const out = new Map<string, Motion>();
	const byId = new Map(plan.parts.map((p) => [p.id, p]));

	// Doors: the hinge side is where the panel it hangs on is; the door
	// turns on its outer edge there (front face, the way a full overlay
	// hinge swings it clear of the side).
	for (const joint of plan.joints) {
		if (joint.kind !== 'hinge' || out.has(joint.edgePart)) continue;
		const door = byId.get(joint.edgePart);
		const side = byId.get(joint.facePart);
		if (!door || !side) continue;
		const vertical = joint.axis.endsWith('z');
		const i = vertical ? 0 : 2;
		const onMin = centre(side)[i] < centre(door)[i];
		const pivot: Vec3 = [0, door.aabb.max[1], 0];
		pivot[i] = onMin ? door.aabb.min[i] : door.aabb.max[i];
		pivot[vertical ? 2 : 0] = centre(door)[vertical ? 2 : 0];
		const axis: Vec3 = vertical ? [0, 0, 1] : [1, 0, 0];
		// Whichever way brings the door's middle out to the front.
		const probe = (angle: number) => {
			const m = rotation(pivot, axis, angle);
			return new THREE.Vector3(...toThree(centre(door))).applyMatrix4(m).z;
		};
		const angle = probe(DOOR_ANGLE) > probe(-DOOR_ANGLE) ? DOOR_ANGLE : -DOOR_ANGLE;
		out.set(door.id, { key: door.id, kind: 'door', pivot, axis, angle });
	}

	// Drawers: every part of one drawer (front, box, bottom) runs out
	// together, as far as a good part of its box.
	const drawers = new Map<string, Part[]>();
	for (const p of plan.parts) {
		const m = p.role.match(/^(.*drawer_\d+)_/);
		if (!m) continue;
		const key = `${p.component}:${m[1]}`;
		drawers.set(key, [...(drawers.get(key) ?? []), p]);
	}
	for (const [key, group] of drawers) {
		const box = group.filter((p) => !p.role.endsWith('_front') || p.role.endsWith('box_front'));
		const depth = Math.max(0, ...box.map((p) => p.aabb.max[1] - p.aabb.min[1]));
		const travel = depth * DRAWER_OUT;
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
	return m.multiply(new THREE.Matrix4().makeTranslation(...toThree([0, motion.travel * t, 0])));
}
