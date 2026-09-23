/**
 * The workshop's library: the engine's defaults (standard sizes sold in
 * Argentina) with the changes a person makes to them — prices, a board
 * size, a hinge that takes other doors, a new slide the shop stocks. The
 * changes are `libraries` overrides, the same the engine reads from a spec:
 * an existing id is patched key by key, a new id comes complete. They apply
 * to every piece and travel inside the spec that is compiled, packaged or
 * saved, so a plan stays reproducible from its spec alone.
 */
import type { LibrariesSnapshot, LibraryOverrides } from '@rewood/engine/browser';

type Entry = { id: string; [k: string]: unknown };
type ListKey = 'materials' | 'edgeMaterials' | 'hardware' | 'suppliers';
export const LISTS: ListKey[] = ['materials', 'edgeMaterials', 'hardware', 'suppliers'];

const STORAGE_KEY = 'rewood.workshop.v1';

function isObject(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Deep merge, the engine's rule: objects merge key by key, anything else (arrays too) replaces. */
export function merge<T>(base: T, patch: unknown): T {
	if (!isObject(base) || !isObject(patch)) return structuredClone(patch) as T;
	const out: Record<string, unknown> = { ...base };
	for (const [k, v] of Object.entries(patch)) out[k] = k in out ? merge(out[k], v) : structuredClone(v);
	return out as T;
}

/** `b` over `a`: entries by id, the later one merged over the earlier. */
export function combine(a: LibraryOverrides | undefined, b: LibraryOverrides | undefined): LibraryOverrides | undefined {
	if (!a || isEmpty(a)) return b && !isEmpty(b) ? b : undefined;
	if (!b || isEmpty(b)) return a;
	const out: LibraryOverrides = {};
	for (const key of LISTS) {
		const list: Entry[] = [];
		for (const e of [...((a[key] ?? []) as Entry[]), ...((b[key] ?? []) as Entry[])]) {
			const i = list.findIndex((x) => x.id === e.id);
			if (i >= 0) list[i] = merge(list[i], e);
			else list.push(structuredClone(e));
		}
		if (list.length) (out as Record<string, unknown>)[key] = list;
	}
	if (a.profile || b.profile) out.profile = merge(a.profile ?? {}, b.profile ?? {});
	return out;
}

export function isEmpty(o: LibraryOverrides): boolean {
	return LISTS.every((k) => !(o[k]?.length ?? 0)) && !(o.profile && Object.keys(o.profile).length);
}

/** The libraries the engine will use: the defaults with the overrides applied. */
export function applyOverrides(base: LibrariesSnapshot, o: LibraryOverrides | undefined): LibrariesSnapshot {
	if (!o || isEmpty(o)) return base;
	const out = structuredClone(base);
	const into = (map: Record<string, unknown>, list: Entry[] | undefined) => {
		for (const e of list ?? []) map[e.id] = e.id in map ? merge(map[e.id], e) : structuredClone(e);
	};
	into(out.materials.materials as Record<string, unknown>, o.materials as Entry[]);
	into(out.materials.edgeMaterials as Record<string, unknown>, o.edgeMaterials as Entry[]);
	into(out.hardware.items as Record<string, unknown>, o.hardware as Entry[]);
	into(out.suppliers.suppliers as Record<string, unknown>, o.suppliers as Entry[]);
	if (o.profile) out.profile = merge(out.profile, o.profile);
	return out;
}

/** What the browser remembers; empty when it cannot (private window, blocked storage). */
export function loadWorkshop(): LibraryOverrides {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return {};
		const v = JSON.parse(raw);
		return isObject(v) ? (v as LibraryOverrides) : {};
	} catch {
		return {};
	}
}

export function saveWorkshop(o: LibraryOverrides) {
	try {
		if (isEmpty(o)) localStorage.removeItem(STORAGE_KEY);
		else localStorage.setItem(STORAGE_KEY, JSON.stringify(o));
	} catch {
		// Not remembered; the changes still apply for this session.
	}
}

/** Set one value (a path of keys) of one entry of a list, keeping the rest of its override. */
export function setValue(o: LibraryOverrides, list: ListKey, id: string, path: string[], value: unknown): LibraryOverrides {
	const next = structuredClone(o);
	const entries = ((next[list] ?? []) as Entry[]).slice();
	let entry = entries.find((e) => e.id === id);
	if (!entry) {
		entry = { id };
		entries.push(entry);
	}
	let node: Record<string, unknown> = entry;
	for (const k of path.slice(0, -1)) {
		if (!isObject(node[k])) node[k] = {};
		node = node[k] as Record<string, unknown>;
	}
	node[path[path.length - 1]] = value;
	(next as Record<string, unknown>)[list] = entries;
	return next;
}

/** Back to the standard: the entry's override goes (a new item goes altogether). */
export function resetEntry(o: LibraryOverrides, list: ListKey, id: string): LibraryOverrides {
	const next = structuredClone(o);
	const entries = ((next[list] ?? []) as Entry[]).filter((e) => e.id !== id);
	if (entries.length) (next as Record<string, unknown>)[list] = entries;
	else delete next[list];
	return next;
}

/** A new item copied whole from an existing one (a new id has to be complete). */
export function duplicate(o: LibraryOverrides, list: ListKey, from: Entry, id: string, name: string): LibraryOverrides {
	const next = structuredClone(o);
	const copy = { ...structuredClone(from), id, name };
	(next as Record<string, unknown>)[list] = [...((next[list] ?? []) as Entry[]), copy];
	return next;
}

/** Whether the workshop changed (or added) this entry. */
export function hasOverride(o: LibraryOverrides, list: ListKey, id: string): boolean {
	return ((o[list] ?? []) as Entry[]).some((x) => x.id === id);
}
