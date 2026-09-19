/**
 * The part of the engine's geometry the viewer needs: how a part's frame
 * maps its (u, v) face coordinates to furniture space, and how furniture
 * space maps to three.js space. Mirrors `geometry.rs`; the engine stays the
 * source of truth — this only draws what it computed.
 *
 * Furniture: X = width, Y = depth (back → front), Z = height.
 * three.js:  x = X, y = Z (up), z = Y (front towards the viewer).
 */
import type { Axis, Dims, Face, Part, Placement, Vec3 } from '@rewood/engine/browser';

export const AXIS_VEC: Record<Axis, Vec3> = {
	pos_x: [1, 0, 0],
	neg_x: [-1, 0, 0],
	pos_y: [0, 1, 0],
	neg_y: [0, -1, 0],
	pos_z: [0, 0, 1],
	neg_z: [0, 0, -1]
};

export function cross(a: Vec3, b: Vec3): Vec3 {
	return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

export function add(a: Vec3, b: Vec3): Vec3 {
	return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

export function scale(a: Vec3, s: number): Vec3 {
	return [a[0] * s, a[1] * s, a[2] * s];
}

/** Local +Z of a placement, as a world vector. */
export function zAxis(p: Placement): Vec3 {
	return cross(AXIS_VEC[p.x], AXIS_VEC[p.y]);
}

export function toWorld(p: Placement, local: Vec3): Vec3 {
	const x = scale(AXIS_VEC[p.x], local[0]);
	const y = scale(AXIS_VEC[p.y], local[1]);
	const z = scale(zAxis(p), local[2]);
	return add(add(add(p.origin, x), y), z);
}

/** Outward normal of a face in local space. */
export function faceNormalLocal(face: Face): Vec3 {
	switch (face) {
		case 'front':
			return [0, 0, 1];
		case 'back':
			return [0, 0, -1];
		case 'left':
			return [-1, 0, 0];
		case 'right':
			return [1, 0, 0];
		case 'bottom':
			return [0, -1, 0];
		case 'top':
			return [0, 1, 0];
	}
}

export function faceNormalWorld(p: Placement, face: Face): Vec3 {
	const n = faceNormalLocal(face);
	return add(add(scale(AXIS_VEC[p.x], n[0]), scale(AXIS_VEC[p.y], n[1])), scale(zAxis(p), n[2]));
}

/** Same convention as `Dims::uv_to_local` in the engine. */
export function uvToLocal(dims: Dims, face: Face, u: number, v: number): Vec3 {
	const along = [dims.length, dims.width, dims.thickness];
	const n = faceNormalLocal(face);
	const [ua, va] = uvAxes(face);
	const p: Vec3 = [0, 0, 0];
	p[ua] = u;
	p[va] = v;
	const ni = n.findIndex((c) => c !== 0);
	p[ni] = n[ni] > 0 ? along[ni] : 0;
	return p;
}

function uvAxes(face: Face): [number, number] {
	switch (face) {
		case 'front':
		case 'back':
			return [0, 1];
		case 'left':
		case 'right':
			return [1, 2];
		case 'bottom':
		case 'top':
			return [0, 2];
	}
}

/** World point of a (u, v) position on a part's face. */
export function faceUvToWorld(part: Part, face: Face, u: number, v: number): Vec3 {
	return toWorld(part.placement, uvToLocal(part.dims, face, u, v));
}

/** Furniture → three.js coordinates. */
export function toThree(v: Vec3): [number, number, number] {
	return [v[0], v[2], v[1]];
}

/**
 * Exploded-view offsets, the same rule as `export/explode.rs`: each part
 * moves a fixed step along its thickness axis away from the furniture
 * centre (a part on the centre plane goes up); doors and drawers come
 * forward first. `factor` 0 = assembled, 1 = the documentation's spread.
 */
export function explodeOffsets(parts: Part[], factor: number): Map<string, Vec3> {
	const out = new Map<string, Vec3>();
	if (parts.length === 0 || factor === 0) return out;
	const lo: Vec3 = [Infinity, Infinity, Infinity];
	const hi: Vec3 = [-Infinity, -Infinity, -Infinity];
	for (const p of parts) {
		for (let i = 0; i < 3; i++) {
			lo[i] = Math.min(lo[i], p.aabb.min[i]);
			hi[i] = Math.max(hi[i], p.aabb.max[i]);
		}
	}
	const centre: Vec3 = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
	const step = 120 * factor;
	for (const p of parts) {
		const c: Vec3 = [
			(p.aabb.min[0] + p.aabb.max[0]) / 2,
			(p.aabb.min[1] + p.aabb.max[1]) / 2,
			(p.aabb.min[2] + p.aabb.max[2]) / 2
		];
		const normal = zAxis(p.placement);
		const d = (c[0] - centre[0]) * normal[0] + (c[1] - centre[1]) * normal[1] + (c[2] - centre[2]) * normal[2];
		const sign = Math.abs(d) < 1e-6 ? 1 : Math.sign(d);
		const off = scale(normal, sign * step);
		if (p.role.includes('door')) off[1] += 1.5 * step;
		else if (p.role.includes('drawer')) {
			off[1] += p.role.endsWith('_front') && !p.role.includes('box_front') ? 2 * step : 1.2 * step;
		}
		out.set(p.id, off);
	}
	return out;
}
