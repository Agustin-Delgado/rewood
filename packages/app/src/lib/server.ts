/**
 * Client of `rewood-server` (§41). Optional: the UI works fully offline with
 * the WASM engine; the server adds persistence (projects, furniture with
 * versions) and immutable manufacturing orders.
 *
 * The base URL is, in order: what the user typed (kept in localStorage),
 * `VITE_REWOOD_SERVER`, and otherwise the page's own origin — which is the
 * right answer when `rewood-server --static` serves the built UI — or the
 * default local port in development.
 */
import type { FurnitureSpec, ManufacturingPlan } from '@rewood/engine/browser';

export interface Project {
	id: string;
	name: string;
	createdAt: string;
}

export interface FurnitureSummary {
	id: string;
	projectId: string;
	version: number;
	name: string;
	updatedAt: string;
}

export interface Furniture {
	id: string;
	projectId: string;
	version: number;
	spec: FurnitureSpec;
	versions: { version: number; spec: FurnitureSpec; savedAt: string }[];
	createdAt: string;
	updatedAt: string;
}

export interface OrderSummary {
	id: string;
	furnitureId: string;
	furnitureVersion: number;
	status: ManufacturingPlan['status'];
	manufacturingBlocked: boolean;
	createdAt: string;
	packageSha256: string;
}

export interface ManufacturingOrder extends Omit<OrderSummary, 'packageSha256'> {
	snapshot: {
		spec: FurnitureSpec;
		plan: ManufacturingPlan;
		packageSha256: string;
		packageFiles: string[];
	};
}

export type ProductionStatus = 'planned' | 'in_progress' | 'done' | 'cancelled';

export interface QcRecord {
	at: string;
	part: string;
	length: number;
	width: number;
	thickness: number;
	notes: string;
	nominal: [number, number, number];
	deviation: [number, number, number];
	tolerance: number;
	pass: boolean;
}

export interface Production {
	orderId: string;
	status: ProductionStatus;
	parts: Record<string, Record<string, boolean>>;
	orderSteps: Record<string, boolean>;
	events: { at: string; what: string }[];
	qc: QcRecord[];
	summary: {
		parts: number;
		cut: number;
		machined: number;
		edged: number;
		assembled: boolean;
		delivered: boolean;
		progress: number;
		qcRecords: number;
		qcFailed: number;
	};
}

export type ProviderRole = 'all' | 'cnc' | 'cutting' | 'assembly' | 'purchasing' | 'supplier';

const STORAGE_KEY = 'rewood.server';

export function defaultServerUrl(): string {
	try {
		const saved = localStorage.getItem(STORAGE_KEY);
		if (saved) return saved;
	} catch {
		/* private mode or blocked storage: fall through */
	}
	const env = import.meta.env.VITE_REWOOD_SERVER as string | undefined;
	if (env) return env;
	return import.meta.env.DEV ? 'http://127.0.0.1:8080' : location.origin;
}

export function rememberServerUrl(url: string) {
	try {
		localStorage.setItem(STORAGE_KEY, url);
	} catch {
		/* ignore */
	}
}

export class ServerError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export class ServerClient {
	base: string;

	constructor(base: string) {
		this.base = base.replace(/\/+$/, '');
	}

	private async call<T>(method: string, path: string, body?: unknown): Promise<T> {
		const res = await fetch(this.base + path, {
			method,
			headers: body === undefined ? {} : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
		if (!res.ok) {
			let message = `${res.status} ${res.statusText}`;
			try {
				const j = (await res.json()) as { error?: string };
				if (j.error) message = j.error;
			} catch {
				/* not json */
			}
			throw new ServerError(res.status, message);
		}
		return (await res.json()) as T;
	}

	health() {
		return this.call<{ engine: string; schema: string; assistant: boolean; model: string | null }>('GET', '/health');
	}
	projects() {
		return this.call<Project[]>('GET', '/projects');
	}
	createProject(name: string) {
		return this.call<Project>('POST', '/projects', { name });
	}
	furnitureList() {
		return this.call<FurnitureSummary[]>('GET', '/furniture');
	}
	furniture(id: string) {
		return this.call<Furniture>('GET', `/furniture/${id}`);
	}
	createFurniture(projectId: string, spec: FurnitureSpec) {
		return this.call<Furniture>('POST', '/furniture', { projectId, spec });
	}
	updateFurniture(id: string, spec: FurnitureSpec) {
		return this.call<Furniture>('PUT', `/furniture/${id}`, spec);
	}
	orders() {
		return this.call<OrderSummary[]>('GET', '/manufacturing-orders');
	}
	order(id: string) {
		return this.call<ManufacturingOrder>('GET', `/manufacturing-orders/${id}`);
	}
	createOrder(furnitureId: string) {
		return this.call<ManufacturingOrder>('POST', '/manufacturing-orders', { furnitureId });
	}
	packageUrl(orderId: string, role: ProviderRole = 'all') {
		return `${this.base}/manufacturing-orders/${orderId}/package${role === 'all' ? '' : `?role=${role}`}`;
	}
	production(orderId: string) {
		return this.call<Production>('GET', `/manufacturing-orders/${orderId}/production`);
	}
	setStep(orderId: string, step: string, done: boolean, part?: string) {
		return this.call<Production>('POST', `/manufacturing-orders/${orderId}/production/steps`, { part, step, done });
	}
	setStatus(orderId: string, status: ProductionStatus) {
		return this.call<Production>('POST', `/manufacturing-orders/${orderId}/production/status`, { status });
	}
	recordQc(orderId: string, part: string, length: number, width: number, thickness: number, notes = '') {
		return this.call<QcRecord>('POST', `/manufacturing-orders/${orderId}/qc`, { part, length, width, thickness, notes });
	}
	packageFileUrl(orderId: string, path: string) {
		return `${this.base}/manufacturing-orders/${orderId}/package/${path}`;
	}
}
