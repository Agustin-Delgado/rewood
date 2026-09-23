/**
 * Names a person reads instead of the spec's ids: "Cajones · módulo 2"
 * for `drawers_m2`, "630 × 560 × 18" for a part's size. The ids stay in
 * tooltips; the workshop labels still carry the part ids.
 */
import type { ComponentSpec, FurnitureSpec, ManufacturingPlan } from '@rewood/engine/browser';

const NOUN: Record<ComponentSpec['type'], string> = {
	carcass: 'Módulo',
	shelves: 'Estantes',
	doors: 'Puertas',
	drawers: 'Cajones',
	rail: 'Barral',
	worktop: 'Tapa de trabajo',
	panel: 'Panel',
	modesty: 'Faldón'
};

/**
 * The bays of every carcass (1-based x ranges between its sides and
 * dividers), and which of them a component's parts sit in. Read off the
 * plan, so a `bay` written as an expression names its bay too.
 */
function baysOf(plan: ManufacturingPlan | null | undefined, component: string, carcass: string | undefined): string | null {
	if (!plan || !carcass) return null;
	const walls = plan.parts
		.filter((p) => p.component === carcass && /^(side_left|side_right|divider_\d+)$/.test(p.role))
		.map((p) => [p.aabb.min[0], p.aabb.max[0]] as const)
		.sort((a, b) => a[0] - b[0]);
	if (walls.length < 3) return null;
	const bays = walls.slice(1).map((w, i) => [walls[i][1], w[0]] as const);
	const own = plan.parts.filter((p) => p.component === component);
	if (own.length === 0) return null;
	const hit = new Set<number>();
	for (const p of own) {
		const c = (p.aabb.min[0] + p.aabb.max[0]) / 2;
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
	const carcasses = spec.components.filter((c) => c.type === 'carcass').map((c) => c.id);
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
