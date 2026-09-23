/**
 * Names a person reads instead of the spec's ids: "Cajones · módulo 2"
 * for `drawers_m2`, "630 × 560 × 18" for a part's size. The ids stay in
 * tooltips; the workshop labels still carry the part ids.
 */
import type { ComponentSpec, FurnitureSpec, ManufacturingPlan, Part } from '@rewood/engine/browser';

const NOUN: Record<ComponentSpec['type'], string> = {
	carcass: 'Módulo',
	shelves: 'Estantes',
	doors: 'Puertas',
	drawers: 'Cajones',
	rail: 'Barral',
	worktop: 'Tapa de trabajo',
	panel: 'Panel',
	modesty: 'Faldón',
	sliding_doors: 'Puertas corredizas',
	sink: 'Bacha'
};

/**
 * The bays of every carcass (1-based x ranges between its sides and
 * dividers), and which of them a component's parts sit in. Read off the
 * plan, so a `bay` written as an expression names its bay too.
 */
function baysOf(plan: ManufacturingPlan | null | undefined, component: string, carcass: string | undefined): string | null {
	if (!plan || !carcass) return null;
	const wallParts = plan.parts.filter((p) => p.component === carcass && /^(side_left|side_right|divider_\d+)$/.test(p.role));
	const left = wallParts.find((p) => p.role === 'side_left');
	const right = wallParts.find((p) => p.role === 'side_right');
	if (!left || !right) return null;
	// Across the walls (X unless the carcass is turned), numbered from the
	// left side as seen from the front.
	const k = left.aabb.max[0] - left.aabb.min[0] < left.aabb.max[1] - left.aabb.min[1] ? 0 : 1;
	const mid = (p: Part) => (p.aabb.min[k] + p.aabb.max[k]) / 2;
	const dir = mid(right) > mid(left) ? 1 : -1;
	const walls = wallParts.map((p) => [p.aabb.min[k], p.aabb.max[k]] as const).sort((a, b) => dir * (a[0] - b[0]));
	if (walls.length < 3) return null;
	const bays = walls.slice(1).map((w, i) => (dir > 0 ? [walls[i][1], w[0]] : [w[1], walls[i][0]]) as readonly [number, number]);
	const own = plan.parts.filter((p) => p.component === component);
	if (own.length === 0) return null;
	const hit = new Set<number>();
	for (const p of own) {
		const c = mid(p);
		bays.forEach(([a, b], i) => {
			if (c > a && c < b) hit.add(i + 1);
		});
	}
	const list = [...hit].sort((a, b) => a - b);
	if (list.length === 0 || list.length === bays.length) return null;
	return list.length === 1 ? `hueco ${list[0]}` : `huecos ${list[0]}–${list[list.length - 1]}`;
}

/** Label of every component of the spec, by id. */
export function componentLabels(spec: FurnitureSpec, plan?: ManufacturingPlan | null): Map<string, string> {
	// Modules numbered as built: one left out by its `when` takes no number.
	const off = new Set(plan?.inactive ?? []);
	const carcasses = spec.components.filter((c) => c.type === 'carcass' && !off.has(c.id)).map((c) => c.id);
	const many = carcasses.length > 1;
	const moduleName = (id: string) => (many ? `módulo ${carcasses.indexOf(id) + 1}` : '');
	const out = new Map<string, string>();
	for (const c of spec.components) {
		if (c.type === 'carcass') {
			out.set(c.id, many ? `Módulo ${carcasses.indexOf(c.id) + 1}` : 'Estructura');
			continue;
		}
		let noun = NOUN[c.type] ?? c.type;
		if (c.type === 'drawers' && c.mount === 'inset') noun = 'Cajones interiores';
		if (c.type === 'doors' && c.fixed) noun = 'Frente fijo';
		if (c.type === 'doors' && c.facing) noun = 'Puertas con espejo';
		if (c.type === 'doors' && c.opening === 'up') noun = 'Puerta basculante';
		if (c.type === 'doors' && c.opening === 'down') noun = 'Tapa rebatible';
		const where: string[] = [];
		const carcass = (c as { carcass?: string }).carcass ?? (many ? undefined : carcasses[0]);
		if (carcass && many) where.push(moduleName(carcass));
		const bay = baysOf(plan, c.id, carcass ?? carcasses[0]);
		if (bay) where.push(bay);
		out.set(c.id, where.length ? `${noun} · ${where.join(', ')}` : noun);
	}
	return out;
}

/** A part's own name without the component id the engine appends. */
export function partName(name: string): string {
	return name.replace(/\s*\([^)]*\)\s*$/, '');
}

const NUM = new Intl.NumberFormat('es-AR', { maximumFractionDigits: 1 });

/** A millimetre value for reading: at most one decimal. */
export function mm(v: number): string {
	return NUM.format(v);
}

export function size(dims: { length: number; width: number; thickness: number }): string {
	return `${mm(dims.length)} × ${mm(dims.width)} × ${mm(dims.thickness)}`;
}
