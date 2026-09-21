/**
 * Hardware, drawn: every fastener of the plan as a few primitives shaped
 * like the thing (a dowel is a peg, a hinge a cup with its arm and plate,
 * a slide two rails, a leg a column) sitting where its holes are. Nothing
 * here decides a position: the holes come from the plan's operations
 * (`source` ties each one to its joint and fastener) and the sizes are
 * symbols, not supplier drawings.
 *
 * Bodies screwed to one part move with that part in the exploded view;
 * what bridges two parts (a dowel, a bolt, a screw) floats between them,
 * and a leader joins the two holes it mates so the eye can follow it.
 */
import type { HardwareDef, Joint, ManufacturingPlan, Part, Vec3 } from '@rewood/engine/browser';
import { add, faceNormalWorld, faceUvToWorld, scale, zAxis } from './geometry';

export type Prim = { colour: string } & (
	| { shape: 'cyl'; pos: Vec3; axis: Vec3; r: number; len: number }
	| { shape: 'box'; pos: Vec3; size: Vec3 }
	| { shape: 'sphere'; pos: Vec3; r: number }
);

export type HardwareSymbol = {
	key: string;
	joint: Joint;
	index: number;
	prims: Prim[];
	/** Mated holes pulled apart by the exploded view, one segment each. */
	leaders: [Vec3, Vec3][];
};

/** One drilled hole of a fastener: its mouth and outward normal, in furniture space. */
type Hole = {
	part: Part;
	label: string;
	p: Vec3;
	n: Vec3;
	d: number;
	depth: number;
	through: boolean;
};

// Palette: metal and plastic, never the colour of the wood.
const ZINC = '#b4bcc4';
const STEEL = '#7d868f';
const BEECH = '#d9b877';
const BLACK = '#33383d';

const ZERO: Vec3 = [0, 0, 0];

function sub(a: Vec3, b: Vec3): Vec3 {
	return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}
function len(a: Vec3): number {
	return Math.hypot(a[0], a[1], a[2]);
}
function norm(a: Vec3): Vec3 {
	const l = len(a);
	return l < 1e-9 ? [0, 0, 1] : scale(a, 1 / l);
}
function mid(a: Vec3, b: Vec3): Vec3 {
	return scale(add(a, b), 0.5);
}
function centroid(hs: Hole[]): Vec3 {
	return scale(
		hs.reduce((s, h) => add(s, h.p), ZERO),
		1 / hs.length
	);
}
/** The axis (unit, axis-aligned) along which a set of holes spreads most. */
function spread(hs: Hole[]): { axis: Vec3; span: number } {
	if (hs.length < 2) return { axis: [0, 0, 1], span: 0 };
	let best: Vec3 = [0, 0, 1];
	let span = 0;
	for (let i = 0; i < 3; i++) {
		const vals = hs.map((h) => h.p[i]);
		const s = Math.max(...vals) - Math.min(...vals);
		if (s > span) {
			span = s;
			best = [0, 0, 0];
			best[i] = 1;
		}
	}
	return { axis: best, span };
}
/** Sizes given as (along a, along b, along the remaining axis) laid out on the world axes. */
function sized(a: Vec3, b: Vec3, along: number, across: number, rest: number): Vec3 {
	const out: Vec3 = [rest, rest, rest];
	for (let i = 0; i < 3; i++) {
		if (Math.abs(a[i]) > 0.5) out[i] = along;
		else if (Math.abs(b[i]) > 0.5) out[i] = across;
	}
	return out;
}

/** A part's thickness axis pointed at another part: the face that looks at it. */
function facing(part: Part, other: Part): Vec3 {
	const z = zAxis(part.placement);
	const pc = scale(add(part.aabb.min, part.aabb.max), 0.5);
	const oc = scale(add(other.aabb.min, other.aabb.max), 0.5);
	const d = sub(oc, pc);
	return d[0] * z[0] + d[1] * z[1] + d[2] * z[2] < 0 ? scale(z, -1) : z;
}

/** The kind of a hardware id when the library is not loaded: its prefix. */
function guessKind(id: string): string {
	if (id.startsWith('minifix')) return 'cam_lock';
	if (id.startsWith('knob')) return 'handle';
	if (id.startsWith('shelf_pin_row')) return 'pin_row';
	if (id.startsWith('shelf_pin')) return 'pin';
	if (id.startsWith('rail_support')) return 'rail_support';
	if (id.startsWith('cabinet_hanger')) return 'hanger';
	if (id.startsWith('plinth_clip')) return 'clip';
	if (id.includes('strike') || id.endsWith('_plate')) return 'strike';
	if (id.startsWith('push_latch') || id.includes('catch')) return 'catch';
	return id.split('_')[0];
}

export function hardwareSymbols(
	plan: ManufacturingPlan,
	parts: Part[],
	offsets: Map<string, Vec3>,
	def: (id: string) => HardwareDef | undefined,
	exploded: boolean
): HardwareSymbol[] {
	const visible = new Map(parts.map((p) => [p.id, p]));
	const all = new Map(plan.parts.map((p) => [p.id, p]));
	const off = (part: Part): Vec3 => offsets.get(part.id) ?? ZERO;

	// Every sourced hole, by the fastener that asked for it.
	const holes = new Map<string, Hole[]>();
	for (const part of parts) {
		for (const op of part.operations) {
			if (op.type !== 'DRILL' || !op.source) continue;
			const key = `${op.source.joint}:${op.source.hardware}:${op.source.fastener}`;
			const list = holes.get(key) ?? [];
			list.push({
				part,
				label: op.source.label,
				p: faceUvToWorld(part, op.face, op.u, op.v),
				n: faceNormalWorld(part.placement, op.face),
				d: op.diameter,
				depth: op.depth ?? part.dims.thickness,
				through: op.through
			});
			holes.set(key, list);
		}
	}

	const out: HardwareSymbol[] = [];
	for (const joint of plan.joints) {
		if (joint.kind === 'row') continue;
		const edge = all.get(joint.edgePart);
		const face = all.get(joint.facePart);
		if (!edge || !face) continue;
		// A body screwed to a hidden part hides with it; what bridges the
		// two needs both.
		const both = visible.has(edge.id) && visible.has(face.id);
		if (!visible.has(edge.id) && !visible.has(face.id)) continue;
		joint.fasteners.forEach((f, index) => {
			const hs = holes.get(`${joint.id}:${f.hardware}:${f.index}`) ?? [];
			const hw = def(f.hardware);
			const kind = hw?.kind ?? guessKind(f.hardware);
			// `index` is the position in the joint (what the selection uses);
			// `f.index` counts within one hardware and is what the holes cite.
			const sym: HardwareSymbol = { key: `${joint.id}:${index}`, joint, index, prims: [], leaders: [] };
			const prims = sym.prims;
			const shown = (part: Part | null) => (part ? visible.has(part.id) : both);
			const at = (part: Part | null): Vec3 => (part ? off(part) : mid(off(edge), off(face)));
			const cyl = (pos: Vec3, axis: Vec3, r: number, l: number, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'cyl', pos: add(pos, at(part)), axis, r, len: l, colour });
			};
			const box = (pos: Vec3, size: Vec3, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'box', pos: add(pos, at(part)), size, colour });
			};
			const sphere = (pos: Vec3, r: number, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'sphere', pos: add(pos, at(part)), r, colour });
			};
			const leader = (a: Vec3, b: Vec3) => {
				if (exploded && both) sym.leaders.push([a, b]);
			};
			// A body that fills a hole and shows its head at the mouth.
			const plug = (h: Hole, colour: string, extra = 0.3) =>
				cyl(sub(h.p, scale(h.n, h.depth / 2 - 0.6)), h.n, h.d / 2 + extra, h.depth, colour, h.part);
			const byLabel = (s: string) => hs.find((h) => h.label.includes(s));

			switch (kind) {
				case 'dowel': {
					const he = byLabel('edge') ?? hs[0];
					const hf = byLabel('face') ?? hs[1];
					if (he && hf) {
						// Its middle on the contact plane, half in each part.
						const centre = add(hf.p, scale(hf.n, (he.depth - hf.depth) / 2));
						cyl(centre, hf.n, hf.d / 2, he.depth + hf.depth, BEECH, null);
						leader(add(hf.p, off(hf.part)), add(he.p, off(he.part)));
					} else if (hs[0]) plug(hs[0], BEECH);
					break;
				}
				case 'cam_lock': {
					const cam = byLabel('cam');
					const thread = byLabel('thread');
					const bolt = hs.find((h) => h.label === 'bolt') ?? byLabel('bolt');
					if (cam) plug(cam, ZINC);
					if (thread) {
						// Screwed into the face part, its shank reaching the cam.
						const reach = bolt?.depth ?? 34;
						cyl(add(thread.p, scale(thread.n, (reach - thread.depth) / 2)), thread.n, 3.5, reach + thread.depth, ZINC, thread.part);
						cyl(add(thread.p, scale(thread.n, reach - 1.5)), thread.n, 4.5, 3, ZINC, thread.part);
						if (bolt) leader(add(thread.p, off(thread.part)), add(bolt.p, off(bolt.part)));
					}
					break;
				}
				case 'confirmat':
				case 'screw': {
					const through = hs.find((h) => h.through) ?? hs.find((h) => h.depth >= h.part.dims.thickness);
					const pilot = hs.find((h) => h !== through);
					if (through) {
						const l = through.part.dims.thickness + (pilot?.depth ?? 20);
						cyl(sub(through.p, scale(through.n, l / 2)), through.n, (pilot?.d ?? 4) / 2 + 0.6, l, ZINC, through.part);
						cyl(add(through.p, scale(through.n, 0.6)), through.n, through.d / 2 + 1.5, 1.4, ZINC, through.part);
						if (pilot) leader(add(sub(through.p, scale(through.n, through.part.dims.thickness)), off(through.part)), add(pilot.p, off(pilot.part)));
					} else if (hs[0]) plug(hs[0], ZINC);
					break;
				}
				case 'hinge': {
					const cup = hs.find((h) => h.label === 'cup');
					if (!cup) {
						sphere(f.position, 8, STEEL, null);
						break;
					}
					const n = cup.n;
					const pilots = hs.filter((h) => h.label.includes('pilot'));
					const hingeAxis: Vec3 = joint.axis.endsWith('z') ? [0, 0, 1] : joint.axis.endsWith('x') ? [1, 0, 0] : [0, 1, 0];
					plug(cup, STEEL);
					// The cup's flange with its two pilot screws.
					const flange = pilots.length === 2 ? mid(pilots[0].p, pilots[1].p) : cup.p;
					box(add(flange, scale(n, 1)), sized(hingeAxis, n, 50, 2, 20), STEEL, edge);
					// The mounting plate, on the side's face that looks at the door,
					// 37 mm from the side's front edge; the arm reaches it from the cup.
					const s = facing(face, edge);
					const i = s.findIndex((c) => Math.abs(c) > 0.5);
					const plate = add(cup.p, scale(n, 37));
					plate[i] = (s[i] > 0 ? face.aabb.max[i] : face.aabb.min[i]) + s[i] * 2;
					plate[1] = face.aabb.max[1] - 37;
					const d = sub(plate, cup.p);
					const reach = Math.abs(d[0] * n[0] + d[1] * n[1] + d[2] * n[2]) + 8;
					box(add(cup.p, scale(n, 1 + reach / 2)), sized(hingeAxis, n, 12, reach, 12), STEEL, edge);
					box(plate, sized(hingeAxis, s, 44, 4, 30), ZINC, face);
					leader(add(add(cup.p, scale(n, reach)), off(edge)), add(plate, off(face)));
					break;
				}
				case 'handle': {
					if (hs.length === 0) break;
					// On the face that looks out of the furniture (+Y), 28 mm off it.
					const outer = hs.map((h) => (h.n[1] > 0 ? h.p : sub(h.p, scale(h.n, h.part.dims.thickness))));
					const o: Vec3 = [0, 1, 0];
					if (hs.length >= 2) {
						const axis = norm(sub(outer[0], outer[1]));
						const span = len(sub(outer[0], outer[1]));
						cyl(add(mid(outer[0], outer[1]), scale(o, 28)), axis, 5, span + 24, BLACK, edge);
						for (const p of outer) cyl(add(p, scale(o, 14)), o, 4, 28, BLACK, edge);
					} else {
						cyl(add(outer[0], scale(o, 5)), o, 5, 10, BLACK, edge);
						sphere(add(outer[0], scale(o, 22)), 13, BLACK, edge);
					}
					break;
				}
				case 'slide': {
					// Two rails, one on the cabinet side and one on the drawer box.
					const style = hw?.slide?.style ?? 'ball';
					const height = style === 'roller' ? 17 : 45;
					// The rail starts a few mm behind the front edge (the first screw
					// is 37 in) and runs past the last screw.
					const rail = (group: Hole[], colour: string): Vec3 | null => {
						if (group.length === 0) return null;
						const c = centroid(group);
						const { axis, span } = spread(group);
						const n = group[0].n;
						const pos = add(c, scale(n, 3));
						const i = axis.findIndex((v) => v > 0.5);
						if (i >= 0) {
							const front = Math.max(...group.map((h) => h.p[i]));
							pos[i] = front + 32 - (span + 75) / 2;
						}
						box(pos, sized(axis, n, span + 75, 6, height), colour, group[0].part);
						return add(pos, off(group[0].part));
					};
					const a = rail(hs.filter((h) => h.part === face), ZINC);
					const b = rail(hs.filter((h) => h.part === edge), STEEL);
					if (a && b) leader(a, b);
					break;
				}
				case 'leg': {
					if (hs.length === 0) break;
					const c = centroid(hs);
					const n = hs[0].n;
					const height = hw?.leg?.height ?? 100;
					const base = hw?.leg?.baseDiameter ?? 50;
					cyl(add(c, scale(n, 1.5)), n, base / 2, 3, BLACK, face);
					cyl(add(c, scale(n, 3 + (height - 9) / 2)), n, 14, height - 9, BLACK, face);
					cyl(add(c, scale(n, height - 3)), n, 20, 6, BLACK, face);
					break;
				}
				case 'catch':
				case 'strike':
				case 'clip':
				case 'rail_support':
				case 'hanger': {
					if (hs.length === 0) break;
					const c = centroid(hs);
					const n = hs[0].n;
					const { axis, span } = spread(hs);
					const [along, across, thick, colour] =
						kind === 'catch'
							? [Math.max(46, span + 16), 16, 12, BLACK]
							: kind === 'strike'
								? [30, 14, 2, ZINC]
								: kind === 'clip'
									? [Math.max(24, span + 8), 22, 10, BLACK]
									: kind === 'rail_support'
										? [Math.max(30, span + 10), 26, 8, ZINC]
										: [Math.max(60, span + 20), 44, 12, ZINC];
					const pos = add(c, scale(n, thick / 2));
					// A catch body reaches the door it holds: its front end at
					// the panel's front edge.
					if (kind === 'catch' && Math.abs(axis[1]) > 0.5) pos[1] = face.aabb.max[1] - 1 - along / 2;
					box(pos, sized(axis, n, along, thick, across), colour, face);
					break;
				}
				case 'pin': {
					// A shelf support: half in the side's row hole, half under the shelf.
					cyl(f.position, facing(face, edge), 2.5, 24, ZINC, face);
					break;
				}
				case 'rail':
				case 'pin_row':
					break;
				default: {
					if (hs.length > 0) for (const h of hs) plug(h, ZINC);
					else sphere(f.position, 8, STEEL, null);
				}
			}
			if (sym.prims.length > 0) out.push(sym);
		});
	}
	return out;
}
