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
 * A carcass and what hangs in it, see `modules()` in `explode.rs`. `run`
 * goes left to right seen from the front (+X unless the carcass is
 * turned); the front looks that way turned a quarter left.
 */
type Module = { centre: Vec3; spread: number; run: Vec3 };

const frontOf = (m: Module): Vec3 => [-m.run[1], m.run[0], 0];

function bbox(parts: Part[]): [Vec3, Vec3] {
	const lo: Vec3 = [Infinity, Infinity, Infinity];
	const hi: Vec3 = [-Infinity, -Infinity, -Infinity];
	for (const p of parts) {
		for (let i = 0; i < 3; i++) {
			lo[i] = Math.min(lo[i], p.aabb.min[i]);
			hi[i] = Math.max(hi[i], p.aabb.max[i]);
		}
	}
	return [lo, hi];
}

function modules(parts: Part[]): Module[] {
	const byCarcass = new Map<string, Part[]>();
	for (const p of parts) {
		if (!['side_left', 'side_right', 'top', 'bottom'].includes(p.role)) continue;
		byCarcass.set(p.component, [...(byCarcass.get(p.component) ?? []), p]);
	}
	const centreOf = (p: Part): Vec3 => [0, 1, 2].map((i) => (p.aabb.min[i] + p.aabb.max[i]) / 2) as Vec3;
	const runOf = (g: Part[]): Vec3 => {
		const l = g.find((p) => p.role === 'side_left');
		const r = g.find((p) => p.role === 'side_right');
		if (!l || !r) return [1, 0, 0];
		const d = [0, 1].map((i) => centreOf(r)[i] - centreOf(l)[i]);
		return Math.abs(d[0]) >= Math.abs(d[1]) ? [Math.sign(d[0]), 0, 0] : [0, Math.sign(d[1]), 0];
	};
	const groups = byCarcass.size ? [...byCarcass.values()] : [parts];
	const out: Module[] = groups.map((g) => {
		const [lo, hi] = bbox(g);
		return { centre: [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2], spread: 0, run: byCarcass.size ? runOf(g) : [1, 0, 0] };
	});
	out.sort((a, b) => a.centre[0] - b.centre[0]);
	// Modules of one run (the same direction) spread along it, in order.
	const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
	const runs = new Map<string, Module[]>();
	for (const m of out) runs.set(m.run.join(','), [...(runs.get(m.run.join(',')) ?? []), m]);
	for (const group of runs.values()) {
		group.sort((a, b) => dot(a.centre, a.run) - dot(b.centre, b.run));
		group.forEach((m, i) => (m.spread = i - (group.length - 1) / 2));
	}
	return out;
}

/**
 * Exploded-view offsets, the same rule as `export/explode.rs`: each part
 * moves a fixed step along its thickness axis away from the centre of its
 * module (a part on the centre plane stays); doors and drawers come
 * forward first, and the modules of a run drift apart. `factor` 0 =
 * assembled, 1 = the documentation's spread.
 */
export function explodeOffsets(parts: Part[], factor: number): Map<string, Vec3> {
	const out = new Map<string, Vec3>();
	if (parts.length === 0 || factor === 0) return out;
	const mods = modules(parts);
	const step = 120 * factor;
	for (const p of parts) {
		const c: Vec3 = [
			(p.aabb.min[0] + p.aabb.max[0]) / 2,
			(p.aabb.min[1] + p.aabb.max[1]) / 2,
			(p.aabb.min[2] + p.aabb.max[2]) / 2
		];
		const dist = (x: Module) => (x.centre[0] - c[0]) ** 2 + (x.centre[1] - c[1]) ** 2;
		let m = mods[0];
		for (const x of mods) if (dist(x) < dist(m)) m = x;
		// A part over several modules (a worktop) explodes from the
		// furniture's centre and does not drift with any one module.
		const covered = mods.filter(
			(x) => p.aabb.min[0] <= x.centre[0] && x.centre[0] <= p.aabb.max[0] && p.aabb.min[1] <= x.centre[1] && x.centre[1] <= p.aabb.max[1]
		).length;
		if (covered > 1) {
			const n = mods.length;
			m = { centre: [0, 1, 2].map((i) => mods.reduce((s, x) => s + x.centre[i], 0) / n) as Vec3, spread: 0, run: [1, 0, 0] };
		}
		const centre = m.centre;
		const normal = zAxis(p.placement);
		const d = (c[0] - centre[0]) * normal[0] + (c[1] - centre[1]) * normal[1] + (c[2] - centre[2]) * normal[2];
		const sign = Math.abs(d) < 1e-6 ? 0 : Math.sign(d);
		const off = scale(normal, sign * step);
		const forward =
			p.role.includes('door') || p.role.includes('fixed_front')
				? 1.5 * step
				: p.role.includes('drawer')
					? p.role.endsWith('_front') && !p.role.includes('box_front')
						? 2 * step
						: 1.2 * step
					: 0;
		const front = frontOf(m);
		for (let i = 0; i < 3; i++) off[i] += front[i] * forward + m.run[i] * m.spread * 2 * step;
		out.set(p.id, off);
	}
	return out;
}
