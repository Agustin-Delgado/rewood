/**
 * Names a person reads instead of the spec's ids: "Cajones · módulo 2"
 * for `drawers_m2`, "630 × 560 × 18" for a part's size. The ids stay in
 * tooltips; the workshop labels still carry the part ids.
 */
import type { ComponentSpec, FurnitureSpec } from '@rewood/engine/browser';

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

/** Label of every component of the spec, by id. */
export function componentLabels(spec: FurnitureSpec): Map<string, string> {
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
		const bay = (c as { bay?: unknown }).bay;
		if (typeof bay === 'number') where.push(`hueco ${bay}`);
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
