# rewood

Compilador determinista de mobiliario (Rust + TS). La especificación de
referencia es el PDF *CAD paramétrico y motor de fabricación para mobiliario*
(v0.1); `README.md` dice qué parte está hecha y cómo correrla.

## Reglas duras

- **Determinismo.** La misma entrada produce exactamente la misma salida.
  Nada de `HashMap` iterado, reloj, azar, disco ni red dentro de
  `rewood-core`. Colecciones ordenadas (`BTreeMap`), ids secuenciales,
  diagnósticos ordenados.
- **El motor es el producto; la UI es una interfaz.** Todo lo que se pueda
  decidir sin geometría B-Rep se decide en `rewood-core`. No arrancar por el
  editor 3D.
- **Intención ≠ fabricación.** Un componente declara *qué* (una carcasa con
  fondo); las perforaciones salen de resolver uniones contra la biblioteca de
  herrajes. Nunca escribir una posición de agujero en un generador.
- **Cambios que mueven una perforación se ven en los fixtures.** Si
  `cargo test` falla en `fixtures.rs`, mirá el diff antes de regenerar con
  `UPDATE_FIXTURES=1`; regenerar sin mirar es esconder una regresión.
- **La IA escribe la spec, nunca geometría.** `rewood-server/src/assistant.rs`
  sólo expone `propose_spec`; lo que vuelve se compila y los hallazgos van de
  vuelta al modelo. No agregar herramientas que escriban piezas u operaciones.
- **Un campo nuevo del plan lleva `#[serde(default)]`.** Las órdenes congeladas
  (§32) se deserializan con el modelo actual; sin default dejan de abrir.
- **Los datos de herrajes son datos.** `data/hardware.json` trae defaults
  indicativos; ajustarlos es un cambio de datos, no de motor, y va con nota.
  Lo mismo `maxSpan` en `materials.json`.
- **Todo de medida estándar y a la venta en Argentina.** Placas, cantos,
  herrajes, bachas, vidrios: un ítem nuevo de la biblioteca es un producto
  que existe acá con esas medidas (Faplac/Egger, Häfele, Ducasse, Eurohard,
  Ferrum…), buscado antes de cargarlo, con la fuente en la `_note`. Nada
  inventado cuando existe un estándar.
- **El motor obliga; los avisos de diseño no bloquean.** Un `FATAL` deja el
  paquete sin DXF ni programas. Lo que "probablemente está mal" (luz de
  estantes, puertas anchas, bahías abiertas) es `DESIGN-*`/`SPEC-21x` con
  `WARNING` o `INFO`: se genera igual y se explica con el número que lo
  justifica. Un hallazgo nuevo lleva `entity` (componente o pieza) para que
  la UI lo ubique.
- **Lo que se ofrece se puede fabricar.** Toda opción de un fixture
  (`options`) la barre `every_option_value_is_manufacturable` en
  `tests/audit.rs`, extremo por extremo. Si uno da ERROR o FATAL se ajusta
  la cota (con una expresión si depende de otra medida), no el test.

- **La UI se arma con el sistema de diseño.** Toda pantalla usa los
  componentes de `packages/app/src/lib/ui` (primitivas headless de
  `@human-kit/ui` con las recetas de `recipes.ts`) y utilidades de Tailwind
  sobre los tokens de `src/app.css`. Nada de `<button>`/`<select>`/`<input>`
  sueltos ni colores escritos a mano: si falta algo, se agrega al sistema
  (receta + componente + ejemplo en `/sistema`), no a la pantalla.

## Idioma

Código, comentarios, nombres y mensajes de test en **inglés**. Textos que lee
una persona (mensajes de diagnóstico, nombres de piezas, salida del CLI,
README) en **español**. Conversación y commits en español.

## Comandos

```bash
cargo test --workspace                   # incluye rewood-server (flujo HTTP completo, producción, QC)
cargo clippy --workspace --all-targets   # sin warnings
cargo fmt --all
pnpm build:wasm && pnpm test             # WASM + @rewood/engine
pnpm build:wasm:web && pnpm dev          # UI (packages/app) en http://localhost:5173
cargo run -p rewood-server -- --data ./data --static packages/app/build   # API + UI en :8080
ANTHROPIC_API_KEY=... cargo run -p rewood-server                          # con asistente (§43)
pnpm check                               # svelte-check + tsc
pnpm build:app && (cd packages/app/build && npx vercel link --project rewood --yes --scope agustindelgados-projects && npx vercel deploy --prod --yes --scope agustindelgados-projects)  # demo rewood-mu.vercel.app (sólo UI). El `link` es obligatorio: el build borra `build/.vercel` y sin él la CLI deploya a un proyecto llamado `build`
```

La UI no calcula geometría: dibuja `placement`/`aabb`/operaciones del plan.
Si algo se ve mal en 3D, primero mirá el plan (`rewood compile`), después el
viewer. Los ejemplos de la UI son los fixtures del repo, importados directo.
