/**
 * Application state: the spec being edited, the plan the engine compiled
 * from it, and what the user selected. The engine is deterministic and
 * fast, so every edit recompiles the whole plan; there is no partial state.
 */
import {
	loadEngine,
	type Diagnostic,
	type Engine,
	type Fastener,
	type Fix,
	type FurnitureSpec,
	type Joint,
	type LibrariesSnapshot,
	type LibraryOverrides,
	type ManufacturingPlan,
	type Part,
	type Severity
} from '@rewood/engine/browser';

import {
	ServerClient,
	defaultServerUrl,
	rememberServerUrl,
	type FurnitureSummary,
	type OrderSummary,
	type Project
} from './server';

import { VARIANTS, findVariant, variantSpec } from './catalog';
import { applyOverrides, combine, loadWorkshop, saveWorkshop } from './workshop';

export type ViewName = 'iso' | 'front' | 'back' | 'left' | 'right' | 'top';
export const VIEWS: { id: ViewName; name: string; title: string }[] = [
	{ id: 'iso', name: '3D', title: 'perspectiva' },
	{ id: 'front', name: 'frente', title: 'alzado frontal' },
	{ id: 'back', name: 'atrás', title: 'alzado posterior' },
	{ id: 'left', name: 'izq.', title: 'lateral izquierdo' },
	{ id: 'right', name: 'der.', title: 'lateral derecho' },
	{ id: 'top', name: 'planta', title: 'vista superior' }
];

class AppState {
	engine: Engine | null = $state(null);
	/** The engine's own libraries: standard sizes sold in Argentina. */
	defaults: LibrariesSnapshot | null = $state.raw(null);
	/** What the engine compiles with: the defaults, the workshop's changes, the spec's own. */
	libraries: LibrariesSnapshot | null = $state.raw(null);
	/** The workshop's changes to the defaults, for every piece; remembered by the browser. */
	workshop: LibraryOverrides = $state.raw(loadWorkshop());
	spec: FurnitureSpec = $state(variantSpec(VARIANTS[0]));
	/** The catalogue variant on screen; null once a spec comes from elsewhere. */
	variant: string | null = $state(VARIANTS[0].id);
	catalogOpen: boolean = $state(false);
	/** The raw JSON text when the user edits the spec by hand. */
	specText: string = $state('');
	specError: string | null = $state(null);
	// Raw: replaced whole on every compile and never edited in place; a deep
	// proxy over thousands of operations made every read in the viewer pay.
	plan: ManufacturingPlan | null = $state.raw(null);
	selectedPart: string | null = $state(null);
	/** A fastener clicked in the viewer: its joint and index within it. */
	selectedFastener: { joint: string; index: number } | null = $state(null);
	/** Component the editor should unfold and scroll to (from a finding). */
	focusComponent: string | null = $state(null);
	hiddenComponents: Set<string> = $state(new Set());
	showHoles: boolean = $state(true);
	/** Fasteners (dowels, minifix, hinges…) drawn where the joints put them. */
	showHardware: boolean = $state(true);
	/** Exploded view: 0 assembled, 1 the documentation's spread. */
	explode: number = $state(0);
	/** Every door and drawer open (the viewer animates to it). */
	openAll: boolean = $state(false);
	/** Doors and drawers opened one by one, by motion key (see `motion.ts`). */
	opened: Set<string> = $state(new Set());

	/** Open or shut one front; `all` are every front's keys, for when all were open. */
	toggleOpen(key: string, all: string[]) {
		const next = this.openAll ? new Set(all) : new Set(this.opened);
		this.openAll = false;
		if (next.has(key)) next.delete(key);
		else next.add(key);
		this.opened = next;
	}

	/** The "abrir" button: everything open, or everything shut. */
	setOpenAll(open: boolean) {
		this.openAll = open;
		this.opened = new Set();
	}

	/** Where the camera looks from: a perspective corner, or a flat elevation. */
	view: ViewName = $state('iso');
	/** Bumped on every view request so asking for the current view re-frames it. */
	viewTick: number = $state(0);

	setView(view: ViewName) {
		this.view = view;
		this.viewTick += 1;
	}

	// --- server (optional) -------------------------------------------------
	server: ServerClient = $state(new ServerClient(''));
	serverStatus: 'off' | 'ok' | 'error' = $state('off');
	serverEngine: string | null = $state(null);
	serverError: string | null = $state(null);
	projects: Project[] = $state([]);
	furnitureList: FurnitureSummary[] = $state([]);
	orders: OrderSummary[] = $state([]);
	/** The server record the spec on screen came from, if any. */
	current: { id: string; projectId: string; version: number } | null = $state(null);
	/** Edited since it was last saved to (or loaded from) the server. */
	dirty: boolean = $state(false);

	async init() {
		this.engine = await loadEngine();
		this.defaults = this.engine.libraries();
		this.libraries = applyOverrides(this.defaults, combine(this.workshop, this.spec.libraries));
		this.specText = JSON.stringify(this.spec, null, 2);
		this.recompile();
		this.server = new ServerClient(defaultServerUrl());
		await this.connect();
	}

	/** Probe the server; a failure is not an error for the UI, just "offline". */
	async connect(url?: string) {
		if (url !== undefined) {
			this.server = new ServerClient(url);
			rememberServerUrl(url);
		}
		try {
			const h = await this.server.health();
			this.serverEngine = h.engine;
			this.serverStatus = 'ok';
			this.serverError = null;
		} catch (e) {
			this.serverStatus = 'error';
			this.serverError = (e as Error).message;
			return;
		}
		await this.refreshServer();
	}

	/** Reload the lists; a failure here is reported but the server stays connected. */
	async refreshServer() {
		if (this.serverStatus !== 'ok') return;
		try {
			[this.projects, this.furnitureList, this.orders] = await Promise.all([
				this.server.projects(),
				this.server.furnitureList(),
				this.server.orders()
			]);
			this.serverError = null;
		} catch (e) {
			this.serverError = (e as Error).message;
		}
	}

	async createProject(name: string) {
		const p = await this.server.createProject(name);
		await this.refreshServer();
		return p;
	}

	/** Save the spec on screen: a new version of `current`, or a new record in `projectId`. */
	async save(projectId?: string) {
		if (!this.current && !projectId) throw new Error('elegí un proyecto');
		const f = this.current
			? await this.server.updateFurniture(this.current.id, this.effectiveSpec())
			: await this.server.createFurniture(projectId as string, this.effectiveSpec());
		this.current = { id: f.id, projectId: f.projectId, version: f.version };
		this.dirty = false;
		await this.refreshServer();
		return f;
	}

	async open(id: string) {
		const f = await this.server.furniture(id);
		this.spec = f.spec;
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.resetView();
		this.current = { id: f.id, projectId: f.projectId, version: f.version };
		this.dirty = false;
		this.variant = null;
		this.recompile();
		this.viewTick += 1;
	}

	/** Freeze the saved version as an order. Unsaved edits are saved first. */
	async emitOrder(projectId?: string) {
		if (!this.current || this.dirty) await this.save(projectId);
		const o = await this.server.createOrder((this.current as { id: string }).id);
		await this.refreshServer();
		return o;
	}

	/**
	 * The spec as the engine gets it: with the workshop's library changes
	 * inside (the spec's own on top), so what is compiled, packaged or saved
	 * carries them and gives the same plan anywhere.
	 */
	effectiveSpec(): FurnitureSpec {
		const libraries = combine(this.workshop, this.spec.libraries);
		if (!libraries) return this.spec;
		return { ...$state.snapshot(this.spec), libraries } as FurnitureSpec;
	}

	/** New workshop changes: remembered and applied to the piece on screen. */
	setWorkshop(next: LibraryOverrides) {
		this.workshop = next;
		saveWorkshop(next);
		this.recompile();
	}

	recompile() {
		if (!this.engine) return;
		// Part and joint ids renumber when parts come and go: the selection
		// follows what was picked (component and role), or clears.
		const before = this.plan;
		const part = before?.parts.find((p) => p.id === this.selectedPart);
		const sel = this.selectedFastener;
		const joint = sel ? before?.joints.find((j) => j.id === sel.joint) : undefined;
		const whatIs = (plan: ManufacturingPlan | null | undefined, id: string) => {
			const p = plan?.parts.find((x) => x.id === id);
			return p ? `${p.component}:${p.role}` : '';
		};
		const jointKey = joint && `${joint.component}:${joint.kind}:${whatIs(before, joint.edgePart)}:${whatIs(before, joint.facePart)}`;

		if (this.defaults) this.libraries = applyOverrides(this.defaults, combine(this.workshop, this.spec.libraries));
		this.plan = this.engine.compile(this.effectiveSpec());
		const plan = this.plan;
		this.selectedPart = part ? (plan.parts.find((p) => p.component === part.component && p.role === part.role)?.id ?? null) : null;
		if (sel && jointKey) {
			const again = plan.joints.find(
				(j) => `${j.component}:${j.kind}:${whatIs(plan, j.edgePart)}:${whatIs(plan, j.facePart)}` === jointKey
			);
			this.selectedFastener = again && again.fasteners[sel.index] ? { joint: again.id, index: sel.index } : null;
		} else this.selectedFastener = null;
	}

	/** A spec from elsewhere (a file, pasted JSON of another piece): nothing of the last one carries over. */
	private resetView() {
		this.selectedPart = null;
		this.selectedFastener = null;
		this.hiddenComponents = new Set();
		this.opened = new Set();
		this.openAll = false;
		this.explode = 0;
	}

	/** Start from a catalogue variant: its template with its option values. */
	loadVariant(id: string) {
		const found = findVariant(id);
		if (!found) return;
		this.spec = variantSpec(found.variant);
		this.variant = id;
		this.catalogOpen = false;
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.resetView();
		this.current = null;
		this.dirty = false;
		this.recompile();
		this.viewTick += 1;
	}

	/** Set a top-level parameter from the properties panel. */
	setParameter(name: string, value: number | string | boolean) {
		this.spec.parameters = { ...(this.spec.parameters ?? {}), [name]: value };
		this.specText = JSON.stringify(this.spec, null, 2);
		this.dirty = true;
		this.recompile();
	}

	/** After a form edited `spec` in place: refresh the JSON view and recompile. */
	touch() {
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.dirty = true;
		this.recompile();
	}

	/** A finding's one-click fix: the engine edits the spec, then recompiles. */
	applyFix(fix: Fix) {
		if (!this.engine) return;
		const before = JSON.stringify(this.spec);
		const next = this.engine.applyFix(this.spec, fix);
		// The engine hands the spec back untouched when the fix no longer
		// applies (the field it points at changed shape): say so.
		if (JSON.stringify(next) === before) {
			this.specError = `no se pudo aplicar «${fix.label}»: la spec cambió desde que se propuso`;
			return;
		}
		this.applySpec(next);
	}

	/** A whole new spec (a fix applied by the engine): replaces what is on screen. */
	applySpec(spec: FurnitureSpec) {
		this.spec = spec;
		this.variant = null;
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.dirty = true;
		this.recompile();
	}

	/** A spec file opened from disk: never a new version of the server record on screen. */
	openSpecFile(text: string) {
		this.current = null;
		this.variant = null;
		this.resetView();
		this.applySpecText(text);
	}

	/** Apply hand-edited JSON. A parse error leaves the last good spec. */
	applySpecText(text: string) {
		this.specText = text;
		try {
			const parsed = JSON.parse(text) as FurnitureSpec;
			if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
				this.specError = 'la spec tiene que ser un objeto JSON';
				return;
			}
			this.specError = null;
			// Another piece of furniture: it is not the server record on
			// screen, and nothing selected or hidden applies to it.
			if (parsed.id !== this.spec.id) {
				this.variant = null;
				this.current = null;
				this.resetView();
			}
			this.spec = parsed;
			this.dirty = true;
			this.recompile();
		} catch (e) {
			this.specError = (e as Error).message;
		}
	}

	toggleComponent(id: string) {
		const next = new Set(this.hiddenComponents);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		this.hiddenComponents = next;
	}

	get selected() {
		return this.plan?.parts.find((p) => p.id === this.selectedPart) ?? null;
	}

	selectPart(id: string | null) {
		this.selectedPart = id;
		this.selectedFastener = null;
	}

	selectFastener(joint: string, index: number) {
		this.selectedFastener = { joint, index };
		this.selectedPart = null;
	}

	/** The joint and fastener the user clicked, resolved against the plan. */
	get fastener(): { joint: Joint; fastener: Fastener } | null {
		const sel = this.selectedFastener;
		if (!sel || !this.plan) return null;
		const joint = this.plan.joints.find((j) => j.id === sel.joint);
		const fastener = joint?.fasteners[sel.index];
		return joint && fastener ? { joint, fastener } : null;
	}

	/** Library name of a hardware id, or the id when the library is not loaded. */
	hardwareName(id: string): string {
		return this.libraries?.hardware.items[id]?.name ?? id;
	}

	partById(id: string): Part | null {
		return this.plan?.parts.find((p) => p.id === id) ?? null;
	}

	/**
	 * What holds a part: every joint touching it, one row per hardware,
	 * with the part on the other side. A dowel row on a shelf reads
	 * "8 × Tarugo 8×30 con P001 Lateral izquierdo".
	 */
	hardwareOf(part: string): HardwareRow[] {
		if (!this.plan) return [];
		const rows: HardwareRow[] = [];
		for (const joint of this.plan.joints) {
			if (joint.edgePart !== part && joint.facePart !== part) continue;
			const otherId = joint.edgePart === part ? joint.facePart : joint.edgePart;
			const other = otherId === part ? null : this.partById(otherId);
			const counts = new Map<string, number>();
			for (const f of joint.fasteners) counts.set(f.hardware, (counts.get(f.hardware) ?? 0) + 1);
			for (const [hardware, count] of counts) {
				rows.push({ joint, hardware, name: this.hardwareName(hardware), count, other });
			}
		}
		return rows;
	}

	/**
	 * Findings about a component: those naming it, and those naming one of
	 * its parts (a hole too close to an edge belongs to the panel, and the
	 * panel to the component that generated it).
	 */
	findingsFor(component: string): Diagnostic[] {
		if (!this.plan) return [];
		const parts = new Set(this.plan.parts.filter((p) => p.component === component).map((p) => p.id));
		return this.plan.diagnostics.items.filter(
			(d) => d.entity === component || (d.entity !== undefined && parts.has(d.entity))
		);
	}

	/** The worst finding touching a part, for the viewer's tint. */
	severityOfPart(part: string): Severity | null {
		if (!this.plan) return null;
		const p = this.plan.parts.find((x) => x.id === part);
		if (!p) return null;
		let worst: Severity | null = null;
		for (const d of this.plan.diagnostics.items) {
			if (d.entity !== part && d.location !== part && d.entity !== p.component) continue;
			if (worst === null || RANK[d.severity] > RANK[worst]) worst = d.severity;
		}
		return worst;
	}
}

const RANK: Record<Severity, number> = { INFO: 0, WARNING: 1, ERROR: 2, FATAL: 3 };

export interface HardwareRow {
	joint: Joint;
	hardware: string;
	name: string;
	count: number;
	/** The part on the other side of the joint; null for a fixture on the part itself. */
	other: Part | null;
}

/** Joint kinds as the workshop calls them. */
export const JOINT_KIND_ES: Record<Joint['kind'], string> = {
	butt: 'unión a tope',
	row: 'hilera de soportes',
	hinge: 'bisagra',
	slide: 'corredera',
	face_to_face: 'cara contra cara',
	handle: 'tirador',
	fixture: 'fijación'
};

export const app = new AppState();
