import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { compile, libraries, type FurnitureSpec } from '../src/index.js';
import { applyOverrides, combine, duplicate, resetEntry, setValue } from '../../app/src/lib/workshop.js';

const fixture = (name: string) =>
  JSON.parse(readFileSync(fileURLToPath(new URL(`../../../fixtures/${name}/input.json`, import.meta.url)), 'utf8')) as FurnitureSpec;

// The workshop's changes to the standard library, as the app keeps them and
// hands them to the engine inside the spec.
describe('workshop library', () => {
  it('changes a standard value for every piece, and back', () => {
    let w = setValue({}, 'materials', 'hdf_3', ['sheetLength'], 2750);
    const spec = { ...fixture('basic_cabinet'), libraries: combine(w, undefined) };
    const back = compile(spec).bom.sheets.find((s) => s.material === 'hdf_3');
    expect(back?.sheetLength).toBe(2750);
    w = resetEntry(w, 'materials', 'hdf_3');
    expect(w).toEqual({});
    expect(compile(fixture('basic_cabinet')).bom.sheets.find((s) => s.material === 'hdf_3')?.sheetLength).toBe(2600);
  });

  it('adds an item copied from a standard one, which a spec can then name', () => {
    const base = libraries();
    const from = base.hardware.items['slide_ball_600'];
    let w = duplicate({}, 'hardware', from as unknown as { id: string }, 'slide_ball_650', 'Corredera 650');
    w = setValue(w, 'hardware', 'slide_ball_650', ['slide', 'length'], 650);
    const eff = applyOverrides(base, w);
    expect(eff.hardware.items['slide_ball_650'].slide?.length).toBe(650);
    const spec = fixture('drawer_unit');
    spec.parameters = { ...spec.parameters, depth: 700 };
    const drawers = spec.components.find((c) => c.type === 'drawers') as { slide: { hardware: string[] } };
    drawers.slide.hardware = ['slide_ball_650'];
    const plan = compile({ ...spec, libraries: w });
    expect(plan.derived['drawers.box_depth']).toBe(650);
    expect(plan.diagnostics.items.filter((d) => d.severity === 'FATAL')).toEqual([]);
  });

  it("lets the spec's own changes win over the workshop's", () => {
    const w = setValue({}, 'edgeMaterials', 'pvc_0_45mm', ['pricePerMetre'], 100);
    const own = setValue({}, 'edgeMaterials', 'pvc_0_45mm', ['pricePerMetre'], 250);
    const both = combine(w, own);
    expect(applyOverrides(libraries(), both).materials.edgeMaterials['pvc_0_45mm'].pricePerMetre).toBe(250);
    // An array (the glass hinge's door limit) is replaced whole, as the engine does.
    const g = setValue({}, 'hardware', 'hinge_glass_overlay', ['hinge', 'maxDoor'], [500, 700]);
    expect(applyOverrides(libraries(), g).hardware.items['hinge_glass_overlay'].hinge?.maxDoor).toEqual([500, 700]);
  });

  it("takes a spec's changes straight from the app's reactive state", () => {
    // Svelte's $state hands out Proxies all the way down, which
    // structuredClone refuses.
    const deep = <T extends object>(o: T): T =>
      new Proxy(o, { get: (t, k) => { const v = Reflect.get(t, k); return typeof v === 'object' && v !== null ? deep(v) : v; } });
    const own = deep(setValue({}, 'edgeMaterials', 'pvc_0_45mm', ['pricePerMetre'], 250));
    const w = setValue({}, 'materials', 'hdf_3', ['sheetLength'], 2750);
    const eff = applyOverrides(libraries(), combine(w, own));
    expect(eff.materials.edgeMaterials['pvc_0_45mm'].pricePerMetre).toBe(250);
    expect(applyOverrides(libraries(), combine(undefined, own)).materials.edgeMaterials['pvc_0_45mm'].pricePerMetre).toBe(250);
  });
});
