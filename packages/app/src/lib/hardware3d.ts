/**
 * Hardware, drawn: every fastener of the plan as a few primitives shaped
 * like the thing (a dowel is a peg, a hinge a cup with its arm and plate,
 * a slide two rails, a leg a column) sitting where its holes are. Nothing
 * here decides a position: the holes come from the plan's operations
 * (`source` ties each one to its joint and fastener) and the sizes are
 * symbols, not supplier drawings.
 *
 * Everything is laid out on the assembled furniture; each primitive names
 * the part it is screwed to (`owner`), so the viewer moves it with that
 * part when the view explodes or a door opens. What bridges two parts (a
 * dowel, a bolt, a screw) has no owner and floats between them, and a
 * leader joins the two holes it mates so the eye can follow it.
 */
import type { HardwareDef, Joint, ManufacturingPlan, Part, Vec3 } from '@rewood/engine/browser';
import { add, faceNormalWorld, faceUvToWorld, scale, zAxis } from './geometry';

export type Prim = {
	colour: string;
	/** The part it moves with; null = halfway between the joint's two parts. */
	owner: string | null;
} & (
	| { shape: 'cyl'; pos: Vec3; axis: Vec3; r: number; len: number }
	| { shape: 'box'; pos: Vec3; size: Vec3 }
	| { shape: 'sphere'; pos: Vec3; r: number }
);

export type HardwareSymbol = {
	key: string;
	joint: Joint;
	index: number;
	prims: Prim[];
	/** Mated holes, one segment each, every end with the part it sits on. */
	leaders: Leader[];
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
	if (id.startsWith('slide_spacer')) return 'spacer';
	if (id.startsWith('track_screw')) return 'track_screw';
	if (id.startsWith('sliding_roller')) return 'sliding_roller';
	if (id.startsWith('sliding_guide')) return 'sliding_guide';
	if (id.startsWith('rail_support')) return 'rail_support';
	if (id.startsWith('cabinet_hanger')) return 'hanger';
	if (id.startsWith('plinth_clip')) return 'clip';
	if (id.includes('strike') || id.endsWith('_plate')) return 'strike';
	if (id.startsWith('push_latch') || id.includes('catch')) return 'catch';
	return id.split('_')[0];
}

export type Leader = { a: Vec3; pa: string; b: Vec3; pb: string };

const AXIS_INDEX: Record<string, number> = { x: 0, y: 1, z: 2 };

/** Unit vector of an axis name (`pos_x`, `neg_y`…). */
function axisVec(a: string): Vec3 {
	const v: Vec3 = [0, 0, 0];
	v[AXIS_INDEX[a.slice(-1)]] = a.startsWith('neg') ? -1 : 1;
	return v;
}

/**
 * Which way a front faces out of the furniture: along its thin horizontal
 * axis, away from the part it hangs on or is fixed to (+Y unless its
 * carcass is turned).
 */
function outwardOf(plan: ManufacturingPlan, all: Map<string, Part>, part: Part): Vec3 {
	const fi = part.aabb.max[0] - part.aabb.min[0] < part.aabb.max[1] - part.aabb.min[1] ? 0 : 1;
	const centre = (p: Part) => (p.aabb.min[fi] + p.aabb.max[fi]) / 2;
	for (const j of plan.joints) {
		const other = j.edgePart === part.id ? j.facePart : j.facePart === part.id ? j.edgePart : null;
		if (!other || other === part.id) continue;
		const o = all.get(other);
		if (!o || Math.abs(centre(o) - centre(part)) < 1e-6) continue;
		const v: Vec3 = [0, 0, 0];
		v[fi] = centre(part) > centre(o) ? 1 : -1;
		return v;
	}
	return [0, 1, 0];
}

export function hardwareSymbols(
	plan: ManufacturingPlan,
	parts: Part[],
	def: (id: string) => HardwareDef | undefined
): HardwareSymbol[] {
	const visible = new Map(parts.map((p) => [p.id, p]));
	const all = new Map(plan.parts.map((p) => [p.id, p]));

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
	// How deep a sliding door track is, for drawing it.
	const trackDepth = plan.bom.hardware.map((h) => def(h.hardware)?.sliding?.depth).find((d) => d) ?? 55;
	for (const joint of plan.joints) {
		// A System 32 row is holes, nothing to draw; a track's screw row
		// carries the track itself.
		if (joint.kind === 'row' && !joint.hardware.some((h) => (def(h)?.kind ?? guessKind(h)) === 'track_screw')) continue;
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
			const cyl = (pos: Vec3, axis: Vec3, r: number, l: number, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'cyl', pos, axis, r, len: l, colour, owner: part?.id ?? null });
			};
			const box = (pos: Vec3, size: Vec3, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'box', pos, size, colour, owner: part?.id ?? null });
			};
			const sphere = (pos: Vec3, r: number, colour: string, part: Part | null) => {
				if (shown(part)) prims.push({ shape: 'sphere', pos, r, colour, owner: part?.id ?? null });
			};
			const leader = (a: Vec3, pa: Part, b: Vec3, pb: Part) => {
				if (both) sym.leaders.push({ a, pa: pa.id, b, pb: pb.id });
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
						leader(hf.p, hf.part, he.p, he.part);
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
						if (bolt) leader(thread.p, thread.part, bolt.p, bolt.part);
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
						if (pilot) leader(sub(through.p, scale(through.n, through.part.dims.thickness)), through.part, pilot.p, pilot.part);
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
					// 37 mm back from the side's front edge, where the door is.
					const o = outwardOf(plan, all, edge);
					const k = o.findIndex((c) => Math.abs(c) > 0.5);
					plate[k] = (o[k] > 0 ? face.aabb.max[k] : face.aabb.min[k]) - o[k] * 37;
					const d = sub(plate, cup.p);
					const reach = Math.abs(d[0] * n[0] + d[1] * n[1] + d[2] * n[2]) + 8;
					box(add(cup.p, scale(n, 1 + reach / 2)), sized(hingeAxis, n, 12, reach, 12), STEEL, edge);
					box(plate, sized(hingeAxis, s, 44, 4, 30), ZINC, face);
					leader(add(cup.p, scale(n, reach)), edge, plate, face);
					break;
				}
				case 'handle': {
					if (hs.length === 0) break;
					// On the face that looks out of the furniture, 28 mm off it.
					const o = outwardOf(plan, all, edge);
					const outer = hs.map((h) => (h.n[0] * o[0] + h.n[1] * o[1] + h.n[2] * o[2] > 0 ? h.p : sub(h.p, scale(h.n, h.part.dims.thickness))));
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
							// The joint runs from the front backwards.
							const s = axisVec(joint.axis)[i] < 0 ? 1 : -1;
							const front = s > 0 ? Math.max(...group.map((h) => h.p[i])) : Math.min(...group.map((h) => h.p[i]));
							pos[i] = front + s * (32 - (span + 75) / 2);
						}
						box(pos, sized(axis, n, span + 75, 6, height), colour, group[0].part);
						return pos;
					};
					// On a spacer, the cabinet rail sits on the spacer's face.
					const spacer = joint.hardware.map((id) => def(id)?.spacer?.thickness ?? (guessKind(id) === 'spacer' ? Number(id.split('_').pop()) : 0)).reduce((a, b) => a + b, 0);
					const onFace = hs.filter((h) => h.part === face).map((h) => ({ ...h, p: add(h.p, scale(h.n, spacer)) }));
					const a = rail(onFace, ZINC);
					const b = rail(hs.filter((h) => h.part === edge), STEEL);
					if (a && b) leader(a, face, b, edge);
					break;
				}
				case 'spacer': {
					// A block on the cabinet panel, under the slide it carries.
					const thick = hw?.spacer?.thickness ?? Number(f.hardware.split('_').pop());
					const slide = joint.hardware.map((id) => def(id)?.slide).find((s) => s);
					const length = slide?.length ?? 450;
					const n = facing(face, edge);
					const i = n.findIndex((c) => Math.abs(c) > 0.5);
					// From 5 mm behind the panel's front edge, backwards.
					const back = axisVec(joint.axis);
					const k = back.findIndex((c) => Math.abs(c) > 0.5);
					const pos: Vec3 = [...f.position];
					pos[k] = back[k] < 0 ? face.aabb.max[k] - 5 - length / 2 : face.aabb.min[k] + 5 + length / 2;
					pos[i] = (n[i] > 0 ? face.aabb.max[i] : face.aabb.min[i]) + n[i] * (thick / 2);
					box(pos, sized(back.map(Math.abs) as Vec3, n, length, thick, 50), BLACK, face);
					break;
				}
				case 'leg': {
					if (hs.length === 0) break;
					const c = centroid(hs);
					const n = hs[0].n;
					const height = hw?.leg?.height ?? 100;
					const base = hw?.leg?.baseDiameter ?? 50;
					if (hw?.leg?.caster) {
						// A plate, the swivel and the wheel under it, its axle across X.
						const wheel = height - 18;
						box(add(c, scale(n, 2)), sized([1, 0, 0], n, base, 4, base), ZINC, face);
						box(add(c, scale(n, 4 + (height - wheel - 4) / 2)), sized([1, 0, 0], n, 20, height - wheel - 4, 30), ZINC, face);
						cyl(add(c, scale(n, height - wheel / 2)), [1, 0, 0], wheel / 2, 22, BLACK, face);
						break;
					}
					cyl(add(c, scale(n, 1.5)), n, base / 2, 3, BLACK, face);
					cyl(add(c, scale(n, 3 + (height - 9) / 2)), n, 14, height - 9, BLACK, face);
					cyl(add(c, scale(n, height - 3)), n, 20, 6, BLACK, face);
					break;
				}
				case 'track_screw': {
					// The track along the row, drawn once, from its first screw.
					if (index !== 0 || joint.fasteners.length === 0) break;
					const ps = joint.fasteners.map((x) => x.position);
					const n = hs[0]?.n ?? [0, 0, 1];
					const lo: Vec3 = [0, 1, 2].map((k) => Math.min(...ps.map((p) => p[k]))) as Vec3;
					const hi: Vec3 = [0, 1, 2].map((k) => Math.max(...ps.map((p) => p[k]))) as Vec3;
					const along: Vec3 = hi[0] - lo[0] >= hi[1] - lo[1] ? [1, 0, 0] : [0, 1, 0];
					const pos = add(mid(lo, hi), scale(n, 6));
					const length = len(sub(hi, lo)) + 100;
					// On a vertical face it is a file rail along a drawer side,
					// not a sliding door track under a top.
					const rest = Math.abs(n[2]) < 0.5 ? 20 : trackDepth;
					box(pos, sized(along, n, length, 12, rest), ZINC, face);
					break;
				}
				case 'sliding_roller':
				case 'sliding_guide': {
					if (hs.length === 0) break;
					const c = centroid(hs);
					const n = hs[0].n;
					const { axis } = spread(hs);
					box(add(c, scale(n, 6)), sized(axis, n, 50, 12, 24), kind === 'sliding_roller' ? BLACK : ZINC, edge);
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
					const back = axisVec(joint.axis);
					const k = back.findIndex((c) => Math.abs(c) > 0.5);
					if (kind === 'catch' && k !== 2 && Math.abs(axis[k]) > 0.5)
						pos[k] = back[k] < 0 ? face.aabb.max[k] - 1 - along / 2 : face.aabb.min[k] + 1 + along / 2;
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
