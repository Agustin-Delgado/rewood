/**
 * The catalogue: furniture by category, and the variants of each. A
 * variant is a template (a fixture of the repo, compiled by the engine
 * like any spec) plus the option values that make it that variant: the
 * desk "con cajonera" and the desk "simple" are one template with its
 * sides set differently. Nothing here computes geometry.
 */
import type { FurnitureSpec, ManufacturingPlan, Part } from '@rewood/engine/browser';

import basicCabinet from '../../../../fixtures/basic_cabinet/input.json';
import bookcase from '../../../../fixtures/bookcase_fixed/input.json';
import bookcaseAdjustable from '../../../../fixtures/bookcase_adjustable/input.json';
import desk from '../../../../fixtures/desk/input.json';
import drawerUnit from '../../../../fixtures/drawer_unit/input.json';
import kitchen from '../../../../fixtures/kitchen_run/input.json';
import nightstand from '../../../../fixtures/nightstand/input.json';
import sideboard from '../../../../fixtures/sideboard/input.json';
import tvUnit from '../../../../fixtures/tv_unit/input.json';
import wallCabinet from '../../../../fixtures/wall_cabinet/input.json';
import wardrobe from '../../../../fixtures/wardrobe_1800/input.json';
import wardrobeModules from '../../../../fixtures/wardrobe_modules/input.json';
import wardrobeRail from '../../../../fixtures/wardrobe_rail/input.json';

export type Preset = Record<string, number | boolean>;

export interface Variant {
	id: string;
	name: string;
	description: string;
	spec: FurnitureSpec;
	/** Option values on top of the template's defaults. */
	preset?: Preset;
}

export interface Category {
	id: string;
	name: string;
	description: string;
	variants: Variant[];
}

const t = (json: unknown) => json as FurnitureSpec;

export const CATALOG: Category[] = [
	{
		id: 'wardrobes',
		name: 'Placares',
		description: 'Módulos con puertas, estantes, cajoneras y barral.',
		variants: [
			{
				id: 'wardrobe_modules',
				name: 'Con cajonera',
				description: 'Tres módulos; la cajonera va en el del medio. Elegí cuántos cajones y qué alto ocupa.',
				spec: t(wardrobeModules)
			},
			{
				id: 'wardrobe_modules_double',
				name: 'Con cajonera doble',
				description: 'Dos módulos de cajones lado a lado, a la izquierda; el tercero de estantes.',
				spec: t(wardrobeModules),
				preset: { drawer_modules: 2, drawer_position: 0 }
			},
			{
				id: 'wardrobe_modules_triple',
				name: 'Con cajonera triple',
				description: 'Cajones a todo el ancho, estantes y puertas encima.',
				spec: t(wardrobeModules),
				preset: { drawer_modules: 3, drawers: 2, drawer_zone_top: 600 }
			},
			{
				id: 'wardrobe_modules_plain',
				name: 'Sin cajonera',
				description: 'Sólo estantes detrás de las puertas.',
				spec: t(wardrobeModules),
				preset: { drawer_modules: 0, shelves: 4 }
			},
			{
				id: 'wardrobe_rail',
				name: 'Con barral',
				description: 'Barral para colgar, cajones interiores y puerta embutida.',
				spec: t(wardrobeRail)
			},
			{
				id: 'wardrobe_1800',
				name: 'Clásico 1800',
				description: 'Tres módulos armados a mano: el ejemplo de referencia del motor.',
				spec: t(wardrobe)
			}
		]
	},
	{
		id: 'desks',
		name: 'Escritorios',
		description: 'Tapa sobre cajoneras, gabinetes o laterales.',
		variants: [
			{
				id: 'desk',
				name: 'Con cajonera y gabinete',
				description: 'Cajonera de tres a la izquierda, gabinete con puerta a la derecha.',
				spec: t(desk)
			},
			{
				id: 'desk_pedestal',
				name: 'Con cajonera',
				description: 'Cajonera a un lado, lateral del otro y faldón entre ambos.',
				spec: t(desk),
				preset: { left: 1, right: 0 }
			},
			{
				id: 'desk_two_pedestals',
				name: 'Con dos cajoneras',
				description: 'Una cajonera a cada lado.',
				spec: t(desk),
				preset: { left: 1, right: 1 }
			},
			{
				id: 'desk_simple',
				name: 'Simple',
				description: 'Tapa sobre dos laterales con faldón, sin cajones.',
				spec: t(desk),
				preset: { left: 0, right: 0, width: 1200 }
			},
			{
				id: 'desk_cabinet',
				name: 'Con gabinete',
				description: 'Gabinete con puerta y estante a la izquierda, lateral a la derecha.',
				spec: t(desk),
				preset: { left: 2, right: 0 }
			},
			{
				id: 'desk_compact',
				name: 'Compacto',
				description: 'Un metro de ancho, cajonera angosta de tres.',
				spec: t(desk),
				preset: { width: 1000, left: 1, right: 0, pedestal_width: 300 }
			}
		]
	},
	{
		id: 'kitchen',
		name: 'Cocina',
		description: 'Bajo mesadas, alacenas y módulos sueltos.',
		variants: [
			{ id: 'kitchen_run', name: 'Bajo mesada', description: 'De dos a cuatro módulos con patas y zócalo; cajonera en el segundo.', spec: t(kitchen) },
			{
				id: 'kitchen_drawer_base',
				name: 'Cajonero bajo mesada',
				description: 'Un módulo de 600 con tres cajones, a la altura de la mesada.',
				spec: t(drawerUnit),
				preset: { width: 600, height: 720, depth: 560, drawer_count: 3 }
			},
			{ id: 'wall_cabinet', name: 'Alacena colgante', description: 'Colgada de la pared, una o dos puertas, estantes regulables.', spec: t(wallCabinet) },
			{
				id: 'wall_cabinet_single',
				name: 'Alacena de una puerta',
				description: 'Angosta, de 450, para completar una hilera.',
				spec: t(wallCabinet),
				preset: { width: 450, door_count: 1 }
			},
			{
				id: 'wall_cabinet_hood',
				name: 'Alacena sobre campana',
				description: 'Baja y sin estantes, para ir sobre el extractor.',
				spec: t(wallCabinet),
				preset: { width: 600, height: 360, shelf_count: 0 }
			},
			{ id: 'basic_cabinet', name: 'Módulo básico', description: 'Carcasa con estantes y puertas.', spec: t(basicCabinet) }
		]
	},
	{
		id: 'bedroom',
		name: 'Dormitorio',
		description: 'Mesas de luz y cajoneras.',
		variants: [
			{ id: 'nightstand', name: 'Mesa de luz', description: 'Cajón arriba, puerta y estante abajo.', spec: t(nightstand) },
			{
				id: 'nightstand_two',
				name: 'Mesa de luz de dos cajones',
				description: 'Dos cajones iguales, sin puerta.',
				spec: t(nightstand),
				preset: { drawers: 2 }
			},
			{
				id: 'nightstand_three',
				name: 'Mesa de luz de tres cajones',
				description: 'Más alta, tres cajones de arriba abajo.',
				spec: t(nightstand),
				preset: { drawers: 3, height: 600 }
			},
			{ id: 'drawer_unit', name: 'Cajonera', description: 'Cajones de arriba abajo, sobre patas con zócalo.', spec: t(drawerUnit) },
			{
				id: 'dresser',
				name: 'Cómoda',
				description: 'Cinco cajones anchos, de 900.',
				spec: t(drawerUnit),
				preset: { width: 900, height: 900, depth: 500, drawer_count: 5 }
			}
		]
	},
	{
		id: 'living',
		name: 'Living y comedor',
		description: 'Muebles bajos, de TV y aparadores.',
		variants: [
			{ id: 'tv_unit', name: 'Mueble de TV', description: 'Cajones a los lados y hueco al medio.', spec: t(tvUnit) },
			{
				id: 'tv_unit_doors',
				name: 'Mueble de TV con puertas',
				description: 'Puertas push-open a los lados, sin tiradores.',
				spec: t(tvUnit),
				preset: { sides: 1 }
			},
			{
				id: 'tv_unit_open',
				name: 'Mueble de TV abierto',
				description: 'Tres huecos con estante, sin frentes.',
				spec: t(tvUnit),
				preset: { sides: 2 }
			},
			{ id: 'sideboard', name: 'Aparador', description: 'Push-open, cajones con cierre suave y patas.', spec: t(sideboard) }
		]
	},
	{
		id: 'bookcases',
		name: 'Bibliotecas',
		description: 'Estantes fijos y regulables.',
		variants: [
			{ id: 'bookcase_fixed', name: 'Con estante fijo', description: 'Un estante estructural al medio y estantes fijos.', spec: t(bookcase) },
			{
				id: 'bookcase_doors',
				name: 'Con puertas abajo',
				description: 'Estantes a la vista arriba, cerrado con puertas bajo el estante fijo.',
				spec: t(bookcase),
				preset: { doors: true }
			},
			{ id: 'bookcase_adjustable', name: 'Regulable', description: 'Estantes sobre soportes Sistema 32, que se mueven.', spec: t(bookcaseAdjustable) }
		]
	}
];

export const VARIANTS: Variant[] = CATALOG.flatMap((c) => c.variants);

export function findVariant(id: string): { category: Category; variant: Variant } | null {
	for (const category of CATALOG) {
		const variant = category.variants.find((v) => v.id === id);
		if (variant) return { category, variant };
	}
	return null;
}

/** The variant's spec: the template with its preset written into the parameters. */
export function variantSpec(v: Variant): FurnitureSpec {
	const spec = structuredClone(v.spec);
	if (v.preset) spec.parameters = { ...(spec.parameters ?? {}), ...v.preset };
	return spec;
}

/** Overall size of what the plan builds, legs included: width × height × depth. */
export function overall(plan: ManufacturingPlan): [number, number, number] | null {
	if (plan.parts.length === 0) return null;
	const min = [Infinity, Infinity, Infinity];
	const max = [-Infinity, -Infinity, -Infinity];
	for (const p of plan.parts) {
		for (let i = 0; i < 3; i++) {
			min[i] = Math.min(min[i], p.aabb.min[i]);
			max[i] = Math.max(max[i], p.aabb.max[i]);
		}
	}
	const r = (v: number) => Math.round(v);
	return [r(max[0] - min[0]), r(max[2] - min[2]), r(max[1] - min[1])];
}

export interface Elevation {
	width: number;
	height: number;
	rects: { x: number; y: number; w: number; h: number; front: boolean }[];
}

const isFront = (p: Part) =>
	(p.role.endsWith('_front') && !p.role.endsWith('box_front')) || p.role.includes('door');

/**
 * The front elevation of a plan: every part's box seen from the front
 * (X across, Z up), back to front so the fronts cover what is behind.
 * SVG space: y grows downwards.
 */
export function elevation(plan: ManufacturingPlan): Elevation | null {
	const parts = plan.parts.filter((p) => p.role !== 'back');
	if (parts.length === 0) return null;
	const x0 = Math.min(...parts.map((p) => p.aabb.min[0]));
	const x1 = Math.max(...parts.map((p) => p.aabb.max[0]));
	const z0 = Math.min(...parts.map((p) => p.aabb.min[2]));
	const z1 = Math.max(...parts.map((p) => p.aabb.max[2]));
	const rects = [...parts]
		.sort((a, b) => a.aabb.max[1] - b.aabb.max[1])
		.map((p) => ({
			x: p.aabb.min[0] - x0,
			y: z1 - p.aabb.max[2],
			w: p.aabb.max[0] - p.aabb.min[0],
			h: p.aabb.max[2] - p.aabb.min[2],
			front: isFront(p)
		}));
	return { width: x1 - x0, height: z1 - z0, rects };
}
