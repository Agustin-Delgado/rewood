/**
 * Browser entry: the same engine, loaded from the `web` wasm-pack build
 * (`pnpm build:wasm:web` → `pkg-web/`). The module must be initialised
 * once, asynchronously, before anything can be compiled.
 */
import init, * as wasm from '../pkg-web/rewood.js';
import wasmUrl from '../pkg-web/rewood_bg.wasm?url';
import type { Fix, FurnitureSpec, LibrariesSnapshot, ManufacturingPlan, PackageFile } from './types.js';

export * from './types.js';

export interface Engine {
  compile(spec: FurnitureSpec): ManufacturingPlan;
  compileJson(specJson: string): ManufacturingPlan;
  packageFiles(spec: FurnitureSpec): PackageFile[];
  /** The spec with a diagnostic's fix applied (unchanged if it does not apply). */
  applyFix(spec: FurnitureSpec, fix: Fix): FurnitureSpec;
  engineVersion(): string;
  schemaVersion(): string;
  libraries(): LibrariesSnapshot;
}

let ready: Promise<Engine> | null = null;

/** Loads the wasm once; later calls return the same engine. */
export function loadEngine(): Promise<Engine> {
  if (!ready) {
    ready = init({ module_or_path: wasmUrl }).then(() => ({
      compile: (spec) => JSON.parse(wasm.compile(JSON.stringify(spec))) as ManufacturingPlan,
      compileJson: (json) => JSON.parse(wasm.compile(json)) as ManufacturingPlan,
      packageFiles: (spec) => JSON.parse(wasm.package_files(JSON.stringify(spec))) as PackageFile[],
      applyFix: (spec, fix) => JSON.parse(wasm.apply_fix(JSON.stringify(spec), JSON.stringify(fix))) as FurnitureSpec,
      engineVersion: () => wasm.engine_version(),
      schemaVersion: () => wasm.schema_version(),
      libraries: () => JSON.parse(wasm.libraries()) as LibrariesSnapshot,
    }));
  }
  return ready;
}
