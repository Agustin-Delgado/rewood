import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { compile, compileJson, engineVersion, libraries, packageFiles, schemaVersion, type FurnitureSpec } from '../src/index.js';

const fixture = (name: string, file: string) =>
  readFileSync(fileURLToPath(new URL(`../../../fixtures/${name}/${file}`, import.meta.url)), 'utf8');

describe('@rewood/engine', () => {
  it('reports versions and libraries', () => {
    expect(engineVersion()).toBe('0.1.0');
    expect(schemaVersion()).toBe('1.0');
    const libs = libraries();
    expect(Object.keys(libs.hardware.items)).toContain('minifix_15');
    expect(libs.materials.materials['melamine_18'].nominalThickness).toBe(18);
    expect(libs.profile.tools.length).toBeGreaterThan(10);
  });

  it('compiles the basic cabinet to the same plan as the Rust fixture', () => {
    const plan = compileJson(fixture('basic_cabinet', 'input.json'));
    const expected = JSON.parse(fixture('basic_cabinet', 'expected.json'));
    expect(plan).toEqual(expected);
    expect(plan.status).toBe('ok');
    expect(plan.parts).toHaveLength(9);
  });

  it('accepts a typed spec and exposes typed operations', () => {
    const spec: FurnitureSpec = {
      schemaVersion: '1.0',
      id: 'ts_cabinet',
      name: 'Módulo desde TS',
      parameters: { width: 800, height: 700, depth: 350 },
      material: 'melamine_18',
      edgeMaterial: 'abs_1mm',
      components: [
        { type: 'carcass', id: 'carcass', joint: { hardware: ['confirmat_7x50'] } },
        { type: 'shelves', id: 'shelves', count: 1, joint: { hardware: ['dowel_8x30'] } },
      ],
    };
    const plan = compile(spec);
    expect(plan.manufacturingBlocked).toBe(false);
    const side = plan.parts.find((p) => p.role === 'side_left')!;
    const through = side.operations.filter((op) => op.type === 'DRILL' && op.through);
    // Confirmat: a through hole on the side for each fastener of the two joints.
    expect(through.length).toBeGreaterThan(0);
    expect(plan.bom.hardware.map((h) => h.hardware)).toEqual(['confirmat_7x50', 'dowel_8x30']);
  });

  it('compiles drawers and produces a package with one DXF per part', () => {
    const spec = JSON.parse(fixture('drawer_unit', 'input.json')) as FurnitureSpec;
    const plan = compile(spec);
    expect(plan).toEqual(JSON.parse(fixture('drawer_unit', 'expected.json')));
    expect(plan.joints.filter((j) => j.kind === 'slide')).toHaveLength(6);
    const files = packageFiles(spec);
    expect(files.filter((f) => f.path.endsWith('.dxf'))).toHaveLength(plan.parts.length);
    expect(files.find((f) => f.path === 'manifest.json')).toBeDefined();
    expect(files.find((f) => f.path === 'documentation/report.html')?.contents).toContain('<h2>Despiece</h2>');
    expect(files.filter((f) => f.path.startsWith('documentation/parts/'))).toHaveLength(plan.parts.length);
    const nc = files.filter((f) => f.path.endsWith('.nc'));
    expect(nc.length).toBeGreaterThanOrEqual(plan.parts.length);
    expect(nc[0].contents.startsWith('%')).toBe(true);
    expect(nc[0].contents).toContain('(REWOOD');
  });

  it('compiles the wardrobe success criterion (§55)', () => {
    const plan = compileJson(fixture('wardrobe_1800', 'input.json'));
    expect(plan).toEqual(JSON.parse(fixture('wardrobe_1800', 'expected.json')));
    expect(plan.manufacturingBlocked).toBe(false);
    expect(plan.derived['carcass.bays']).toBe(3);
    expect(plan.joints.map((j) => j.kind)).toContain('handle');
  });

  it('compiles fixed shelves at explicit heights', () => {
    const plan = compileJson(fixture('bookcase_fixed', 'input.json'));
    expect(plan).toEqual(JSON.parse(fixture('bookcase_fixed', 'expected.json')));
    const fixed = plan.parts.find((p) => p.role === 'fixed_shelf_1');
    expect(fixed?.placement.origin[2]).toBe(1000);
  });

  it('turns a bad spec into a blocked plan instead of throwing', () => {
    const plan = compileJson('{ nope');
    expect(plan.manufacturingBlocked).toBe(true);
    expect(plan.diagnostics.items[0].code).toBe('SPEC-000');
  });
});
