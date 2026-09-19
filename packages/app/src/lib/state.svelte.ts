/**
 * Application state: the spec being edited, the plan the engine compiled
 * from it, and what the user selected. The engine is deterministic and
 * fast, so every edit recompiles the whole plan; there is no partial state.
 */
import {
	loadEngine,
	type Diagnostic,
	type Engine,
	type FurnitureSpec,
	type LibrariesSnapshot,
	type ManufacturingPlan,
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

import basicCabinet from '../../../../fixtures/basic_cabinet/input.json';
import bookcase from '../../../../fixtures/bookcase_fixed/input.json';
import kitchen from '../../../../fixtures/kitchen_run/input.json';
import drawerUnit from '../../../../fixtures/drawer_unit/input.json';
import wardrobe from '../../../../fixtures/wardrobe_1800/input.json';

export const EXAMPLES: { id: string; name: string; spec: FurnitureSpec }[] = [
	{ id: 'wardrobe_1800', name: 'Placard 1800 (3 módulos)', spec: wardrobe as FurnitureSpec },
	{ id: 'basic_cabinet', name: 'Módulo básico 900×800', spec: basicCabinet as FurnitureSpec },
	{ id: 'drawer_unit', name: 'Cajonera 600×700', spec: drawerUnit as FurnitureSpec },
	{ id: 'bookcase_fixed', name: 'Biblioteca 800×2000 con estante fijo', spec: bookcase as FurnitureSpec },
	{ id: 'kitchen_run', name: 'Bajo mesada 1800: 3 módulos', spec: kitchen as unknown as FurnitureSpec }
];

class AppState {
	engine: Engine | null = $state(null);
	libraries: LibrariesSnapshot | null = $state(null);
	spec: FurnitureSpec = $state(structuredClone(EXAMPLES[0].spec));
	/** The raw JSON text when the user edits the spec by hand. */
	specText: string = $state('');
	specError: string | null = $state(null);
	plan: ManufacturingPlan | null = $state(null);
	selectedPart: string | null = $state(null);
	/** Component the editor should unfold and scroll to (from a finding). */
	focusComponent: string | null = $state(null);
	hiddenComponents: Set<string> = $state(new Set());
	showHoles: boolean = $state(true);
	/** Exploded view: 0 assembled, 1 the documentation's spread. */
	explode: number = $state(0);

	// --- server (optional) -------------------------------------------------
	server: ServerClient = $state(new ServerClient(''));
	serverStatus: 'off' | 'ok' | 'error' = $state('off');
	serverEngine: string | null = $state(null);
	/** Model name when the server has the assistant on. */
	assistantModel: string | null = $state(null);
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
		this.libraries = this.engine.libraries();
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
			this.assistantModel = h.assistant ? h.model : null;
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
			? await this.server.updateFurniture(this.current.id, this.spec)
			: await this.server.createFurniture(projectId as string, this.spec);
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
		this.selectedPart = null;
		this.hiddenComponents = new Set();
		this.current = { id: f.id, projectId: f.projectId, version: f.version };
		this.dirty = false;
		this.recompile();
	}

	/** Freeze the saved version as an order. Unsaved edits are saved first. */
	async emitOrder(projectId?: string) {
		if (!this.current || this.dirty) await this.save(projectId);
		const o = await this.server.createOrder((this.current as { id: string }).id);
		await this.refreshServer();
		return o;
	}

	recompile() {
		if (!this.engine) return;
		this.plan = this.engine.compile(this.spec);
		if (this.selectedPart && !this.plan.parts.some((p) => p.id === this.selectedPart)) {
			this.selectedPart = null;
		}
	}

	loadExample(id: string) {
		const ex = EXAMPLES.find((e) => e.id === id);
		if (!ex) return;
		this.spec = structuredClone(ex.spec);
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.selectedPart = null;
		this.hiddenComponents = new Set();
		this.current = null;
		this.dirty = false;
		this.recompile();
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

	/** A spec proposed by the assistant: replaces what is on screen. */
	applySpec(spec: FurnitureSpec) {
		this.spec = spec;
		this.specText = JSON.stringify(this.spec, null, 2);
		this.specError = null;
		this.dirty = true;
		this.recompile();
	}

	/** Apply hand-edited JSON. A parse error leaves the last good spec. */
	applySpecText(text: string) {
		this.specText = text;
		try {
			const parsed = JSON.parse(text) as FurnitureSpec;
			this.specError = null;
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

export const app = new AppState();
