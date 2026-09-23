import { describe, expect, it } from 'vitest';

import { compile } from '../src/index.js';
import { VARIANTS, variantSpec } from '../../app/src/lib/catalog.js';

// What the catalogue offers is manufacturable: every variant, preset
// applied, compiles without an ERROR or FATAL finding. The options of each
// template are swept in `tests/audit.rs`; the presets live only here.
describe('catalogue', () => {
  it('has unique variant ids', () => {
    const ids = VARIANTS.map((v) => v.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  for (const v of VARIANTS) {
    it(`${v.id} compiles`, () => {
      const plan = compile(variantSpec(v));
      const bad = plan.diagnostics.items.filter((d) => d.severity === 'ERROR' || d.severity === 'FATAL');
      expect(bad.map((d) => `${d.code} ${d.message}`)).toEqual([]);
      expect(plan.parts.length).toBeGreaterThan(0);
    });
  }
});
