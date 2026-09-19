# rewood

[![CI](https://github.com/Agustin-Delgado/rewood/actions/workflows/ci.yml/badge.svg)](https://github.com/Agustin-Delgado/rewood/actions/workflows/ci.yml)
Licencia MIT.

Compilador determinista de mobiliario: de una especificación paramétrica a un
paquete de fabricación (piezas, uniones, perforaciones, ranuras, cantos,
despiece, BOM, DXF por pieza y validación). Implementa los pasos 1–8 del orden
de desarrollo de la especificación técnica (*CAD paramétrico y motor de
fabricación para mobiliario*, §54), la segunda etapa (§50): bisagras,
correderas y cajones, patas y zócalo, tiradores, divisores/bahías, estantes
fijos, cantos configurables y exportación DXF; documentación automática
(§29–30: planos, vistas incluida la explotada, etiquetas con QR, manual de
armado, informe imprimible); nesting (§25, paso 11); CAM con postprocesador
ISO genérico y simulación del NC (§26–28, pasos 12–13); una UI SvelteKit +
Threlte (§37–38) que corre el motor en WASM en el navegador; un servicio HTTP
con órdenes inmutables (§39–41), proveedores, compras, seguimiento de
producción y control de calidad (§52, etapa 4); y el asistente de lenguaje
natural a spec (§43, etapa 5). El criterio de éxito técnico (§55, placard
1800×2100×500 con 3 módulos, 2 puertas, 4 estantes y 3 cajones) compila sin
intervención en `fixtures/wardrobe_1800`. Sin kernel B-Rep todavía (no hace
falta mientras todo sea panel rectangular).

```text
F(FurnitureSpec, MaterialLibrary, HardwareLibrary, ManufacturingRules, Profile)
  -> ManufacturingPlan
```

La misma entrada produce exactamente la misma salida. El motor no toca reloj,
azar, disco ni red.

## Estructura

```text
crates/rewood-core   motor (Rust puro, sin I/O)
crates/rewood-cli    `rewood compile|validate|cutlist|package`
crates/rewood-wasm   wrapper wasm-bindgen: JSON entra, JSON sale
crates/rewood-server servicio HTTP (axum): proyectos, muebles versionados, recálculo, órdenes con snapshot inmutable
packages/engine      @rewood/engine: tipos TS + wrapper sobre el WASM (node y browser)
packages/app         UI SvelteKit + Threlte: árbol, 3D, parámetros, hallazgos, despiece, paquete
fixtures/<nombre>/   input.json + expected.json (regresión byte a byte):
                     basic_cabinet, drawer_unit, wardrobe_1800, bookcase_fixed, kitchen_run, invalid_cabinet
```

Dentro de `rewood-core`:

| módulo | qué hace |
|---|---|
| `expr` | lenguaje de expresiones (`width - 2 * thickness`, `if(...)`, `max(...)`, comparaciones, `and/or/not`) |
| `params` | grafo de parámetros: orden topológico, detección de ciclos, recálculo de dependientes |
| `spec` | formato de entrada propio, versionado (`schemaVersion: "1.0"`), `deny_unknown_fields` |
| `geometry` | paneles rectangulares en marcos alineados a ejes, caras semánticas, mapeo cara↔(u,v) |
| `library` | materiales, cantos, herrajes y perfil de fabricación (JSON embebido en `data/`, sobreescribible por id desde la spec) |
| `components` | generadores: `carcass`, `shelves`, `doors`, `drawers` → piezas + pedidos de unión |
| `joints` | seis tipos de unión (`butt`, `hinge`, `slide`, `face_to_face`, `handle`, `fixture` para patas y clips), reparto de herrajes a lo largo (y en filas) de la unión, perforaciones por herraje |
| `rules` | reglas de fabricación (`FAB-*`), puras y en orden fijo |
| `cam` | toolpaths independientes de máquina (taladro, ranura, contorno, taladro horizontal) por pieza y puesta, giro a 90° cuando sólo entra así en la mesa, y `PostProcessor` con `GenericIso` |
| `simulation` | intérprete ISO del NC emitido: límites de máquina, rápidos en material, material removido cruzado con el `Program` (`CAM-30x`), tiempo estimado |
| `nesting` | MaxRects (best short side fit) por material, con veta, kerf y margen, o **guillotina** para seccionadora (`profile.nesting.mode`) con la secuencia de cortes en tres etapas; determinista; módulo aparte del núcleo |
| `plan` | `ManufacturingPlan`: despiece agrupado, BOM con costos (placas del nesting), compras por proveedor, nesting, mecanizado simulado, versiones, diagnósticos |
| `export` | paquete de fabricación: DXF R12 por pieza, planos SVG acotados, vistas de ensamblaje, etiquetas con QR, informe HTML imprimible, CSV, manifest |

## Uso

```bash
cargo run -p rewood-cli -- cutlist  fixtures/basic_cabinet/input.json
cargo run -p rewood-cli -- validate fixtures/invalid_cabinet/input.json   # exit 1 si queda bloqueado
cargo run -p rewood-cli -- compile  fixtures/basic_cabinet/input.json > plan.json
cargo run -p rewood-cli -- package  fixtures/drawer_unit/input.json  out/drawer_unit
```

`package` escribe `parts/P001.dxf…` (para importar en un CAM), `cnc/P001_A.nc…`
(G-code ISO por pieza y puesta, `cnc/toolpaths.json` con la versión
independiente de máquina, `cnc/README.txt` con la convención de puestas),
`documentation/report.html`
(despiece, BOM, hallazgos, vistas frontal/lateral/superior, un plano acotado por
pieza con tabla de operaciones, y las etiquetas: se imprime a PDF desde el
navegador), `documentation/parts/P001.svg…`, `documentation/assembly.svg` (frontal,
lateral, superior y explotada isométrica), `documentation/exploded.svg`,
`documentation/nesting.svg` (una placa por figura, piezas giradas en otro tono),
`documentation/assembly.txt` (manual de armado paso a paso generado desde el
grafo de uniones: carcasa abierta, estantes con tarugos, fondo, tapa, cada
cajón con sus correderas y frente, puertas con bisagras; herrajes por paso),
`labels/labels.svg` (QR `rewood://<id>-v<versión>/<pieza>` por etiqueta,
verificado decodificando con OpenCV), `bom/*.csv`, `documentation/cutlist.txt`,
`documentation/operations.csv`, `plan.json` y `manifest.json` (ver
`parts/README.txt` para las convenciones de capas). Los DXF se verificaron
abriéndolos con ezdxf y renderizándolos; las capas nombran cara, diámetro y
profundidad (`DRILL_FRONT_D35_L12.5`, `DRILL_EDGE_LEFT_D8_L34`, `GROOVE_FRONT_W3.2_L8`).

Servicio (§39–41):

```bash
cargo run -p rewood-server -- --data ./data --listen 127.0.0.1:8080
cargo run -p rewood-server -- --data ./data --static packages/app/build   # sirve también la UI
```

| ruta | qué hace |
|---|---|
| `POST /projects`, `GET /projects` | proyectos |
| `POST /furniture` `{ projectId, spec }`, `GET /furniture`, `GET /furniture/:id`, `PUT /furniture/:id` (spec) | muebles; cada PUT guarda una versión nueva y conserva las anteriores |
| `POST /furniture/:id/recalculate` (alias `manufacturing-plan`), `GET …/parts`, `GET …/bom`, `GET …/operations`, `POST …/validate` | el plan del mueble, recompilado siempre desde la spec: el servicio no guarda planes salvo dentro de una orden |
| `POST /manufacturing-orders` `{ furnitureId }` | **snapshot inmutable** (§32): spec, plan completo (con perfil y versiones de bibliotecas), todos los archivos del paquete congelados en disco y su SHA-256. Un plan bloqueado da 422 |
| `GET /manufacturing-orders`, `GET /manufacturing-orders/:id`, `GET …/:id/package[?role=cnc\|cutting\|assembly\|purchasing]` (zip), `GET …/:id/package/<ruta>` | consultar y bajar lo congelado; con `role`, sólo los archivos de ese proveedor (§52 multi-proveedor: el CNC recibe programas y DXF, la seccionadora despiece y nesting, el armador la documentación, compras las órdenes) |
| `GET …/:id/production`, `POST …/:id/production/steps` `{ part?, step, done }`, `POST …/:id/production/status` `{ status }` | seguimiento de producción (§52): pasos `cut`/`machined`/`edged` por pieza y `assembled`/`delivered` por orden, estado `planned`→`in_progress`→`done` (o `cancelled`), eventos; es el lado mutable de la orden, el snapshot no se toca |
| `GET …/:id/qc`, `POST …/:id/qc` `{ part, length, width, thickness, notes? }` | control de calidad (§52): cada medición se juzga contra las dimensiones terminadas del plan con `profile.tolerances.length` (largo y ancho; el espesor es de la placa) y queda registrada, pase o no |
| `GET /libraries`, `GET /health` | bibliotecas por defecto, versiones, si el asistente está activo |
| `POST /assistant` `{ message, spec?, history? }` | lenguaje natural → spec (§43/§53), ver abajo; 503 sin `ANTHROPIC_API_KEY` |

**Etapa 4 (§52), lo que cabe en un taller.** Proveedores (`data/suppliers.json`,
parcheables por `libraries.suppliers`; cada material y herraje dice el suyo
con `supplier`) y el plan trae `purchasing`: la BOM partida por proveedor,
con cantidades, precios y plazo, ítems iguales fusionados (los tornillos
4×16 de patas y clips son una línea); en el paquete va `purchasing/<proveedor>.csv`
y en el informe la sección "Compras por proveedor". El servicio agrega
seguimiento de producción, control de calidad y paquetes por rol de
proveedor (tabla de arriba); la UI los opera desde la pestaña Servidor
("producción" en cada orden). No hay compra automática contra un proveedor
real ni API de proveedores: eso es integración con cada uno.

**Asistente (etapa 5, §43/§53).** `POST /assistant` manda el pedido al modelo
(`claude-sonnet-5` por defecto, `REWOOD_MODEL` para cambiarlo) con una sola
herramienta, `propose_spec`, y un prompt que describe el formato, los ids de
las bibliotecas (sacados de `Libraries::default()`, así un herraje nuevo se
ofrece solo) y el placard de ejemplo. El modelo escribe la **spec**; el
servidor la compila con el motor y, si hay FATAL o ERROR, le devuelve los
hallazgos como resultado de la herramienta hasta cuatro vueltas; la respuesta
trae la spec que compiló, su estado, los hallazgos y cuántas correcciones
hicieron falta. El modelo nunca ve ni decide una perforación: la arquitectura
del §43 tal cual. Está detrás del trait `Model`, y el bucle se prueba con un
modelo guionado (`FakeModel`) que primero propone correderas de 450 en un
mueble de 400 de fondo y después lo corrige. Con el servidor sin clave la UI
lo dice; con clave, la pestaña **Asistente** conversa, aplica la spec que
vuelve y muestra el estado. No se probó contra la API real en este entorno
(no había clave): el cliente HTTP sigue el formato Messages con tool use.

CORS abierto (no hay autenticación ni nada por usuario todavía). Con
`--static <dir>` el mismo proceso sirve la UI compilada desde `/` (rutas
desconocidas caen en `index.html`; la API siempre gana): un solo binario para
una estación de taller.

Monolito modular: el motor corre en el proceso, el almacenamiento es un
directorio de JSON (`projects/`, `furniture/`, `orders/`, `packages/<orden>/`)
con la misma forma que tendrían las tablas de PostgreSQL; cambiar de backend es
un adaptador. Los tests recorren el flujo entero (proyecto → mueble → plan →
versión nueva → orden → zip) y verifican que editar el mueble después no toca
la orden.

UI:

```bash
pnpm build:wasm:web && pnpm dev     # http://localhost:5173
pnpm build:app                      # estático en packages/app/build
```

Árbol de componentes y piezas (mostrar/ocultar por componente), vista 3D con
cada pieza como caja en su `aabb`, perforaciones como cilindros y ranuras como
huecos (calculados desde el mismo (u, v) del plan, no geometría propia), un
deslizador de vista explotada (misma regla que `export/explode.rs`), panel
de parámetros que recompila en vivo, valores derivados, detalle y operaciones de
la pieza seleccionada, editor de componentes por formulario (agregar, quitar,
reordenar; bahía, zona, cantidades, materiales, cantos, herrajes, bisagras,
correderas y tiradores elegidos de las bibliotecas que expone el motor con
`libraries()`; restricciones con su hallazgo al lado; sobreescrituras de
biblioteca como JSON acotado), pestañas de hallazgos / despiece / BOM / placas (nesting clickeable) / spec JSON
editable, y descarga del paquete completo como `.zip` o del informe en una
pestaña nueva. El motor corre en WASM en el navegador; el plan que muestra es
byte a byte el mismo que produce el CLI (los fixtures lo comprueban en vitest).

La pestaña **Servidor** conecta con `rewood-server` (URL editable; por defecto
el propio origen de la página, `VITE_REWOOD_SERVER`, o `127.0.0.1:8080` en
desarrollo): crear proyectos, guardar el mueble (cada guardado es una versión
nueva; el encabezado muestra `fur-000001 · v2 · sin guardar`), abrir uno
guardado, emitir la orden de fabricación (guarda antes si hay cambios; con
hallazgos ERROR pregunta; con FATAL no deja) y bajar el zip o el informe
congelados de cada orden. Sin servidor la UI sigue funcionando entera: el
motor está en el navegador.

Tests:

```bash
cargo test --workspace                       # unitarios + end-to-end + fixtures
UPDATE_FIXTURES=1 cargo test -p rewood-core  # regenerar expected.json a propósito
pnpm build:wasm && pnpm test                 # WASM + wrapper TS (vitest)
```

El test de fixtures compara el plan completo con `expected.json` después de
redondear a 0,001 mm: si un cambio del motor mueve una perforación, falla y
dice dónde (`plan.parts[2].operations[1].u: 36 vs 34`).

## La especificación de entrada

```json
{
  "schemaVersion": "1.0",
  "id": "basic_cabinet",
  "name": "Módulo básico",
  "parameters": { "width": 900, "height": 800, "depth": 400, "door_count": "if(width > 600, 2, 1)" },
  "material": "melamine_18",
  "edgeMaterial": "abs_1mm",
  "components": [
    { "type": "carcass", "id": "carcass", "bays": 2, "joint": { "hardware": ["minifix_15", "dowel_8x30"] },
      "back": { "material": "hdf_3", "groove": { "inset": 10, "depth": 8 } } },
    { "type": "shelves", "id": "shelves", "bay": 1, "count": 2, "joint": { "hardware": ["dowel_8x30"] } },
    { "type": "doors", "id": "doors", "bay": 1, "count": "door_count", "handle": { "hardware": ["handle_bar_128"] } },
    { "type": "drawers", "id": "drawers", "bay": 2, "zone": { "from": 0, "to": 800 }, "count": 3,
      "joint": { "hardware": ["dowel_8x30"], "placement": { "endOffset": 40, "maxSpacing": 150 } },
      "slide": { "hardware": ["slide_ball_450"] }, "handle": { "hardware": ["handle_bar_128"] } }
  ],
  "constraints": [
    { "id": "shelf_span", "expr": "carcass.inner_width <= 1000", "message": "..." }
  ]
}
```

- Todo campo numérico de un componente acepta un número o una expresión sobre
  los parámetros. La intención es que ninguna dimensión relevante sea un
  literal suelto.
- Los generadores publican valores derivados (`carcass.inner_width`,
  `doors.door_width`, `shelves.bay_height`) que las restricciones pueden usar.
- `libraries` agrega o **parchea por id** materiales, cantos, herrajes y el
  perfil, sin tocar el motor: un id existente cambia sólo las claves dadas
  (`{ "id": "melamine_18", "pricePerSheet": 52000 }`), uno nuevo tiene que
  venir completo; `profile` es un parche del perfil por defecto (`{ "nesting":
  { "mode": "guillotine" } }`). Lo que no describe un objeto válido es
  `LIB-105`.
- Costos: `pricePerSheet` (material), `pricePerMetre` (canto), `unitPrice`
  (cada `bomItem` de un herraje) y `machine.hourlyRate` × tiempo simulado,
  en `profile.currency`. La BOM trae el costo por línea, `materialsCost`,
  `machiningCost`, `totalCost` y la lista `unpriced` de lo que se usó sin
  precio: un 0 es "sin precio", nunca "gratis", y el total es un piso. Las
  bibliotecas por defecto no traen precios (son del taller);
  `fixtures/kitchen_run` los pone por `libraries`.
- Cada componente acepta `edges`: `default` (lo que el generador considera
  razonable: el frente en carcasa y estantes, los cuatro cantos en puertas y
  frentes), `none`, `front` o `all`.
- `joint.placement` sobreescribe la regla de reparto de los herrajes para las
  uniones de ese componente.
- Bahías: `bays: N` en la carcasa genera N−1 divisores verticales (largo
  interior, hasta la ranura del fondo) unidos a tapa y base; `bayWidths:
  [500, "auto", 400]` fija anchos interiores explícitos (un `"auto"` absorbe
  el resto; si no hay `auto` la suma tiene que cerrar: `SPEC-309`). Estantes, puertas
  y cajones eligen bahía con `bay` (1-based; omitido = todas) y una franja de
  altura con `zone: { from, to }` (mm desde la base de la carcasa). Un frente
  sobre un lateral lo cubre entero; sobre un divisor cubre la mitad, así dos
  frentes vecinos se encuentran en el centro del divisor con una luz.
- Varias carcasas: cada una acepta `origin: { x, y, z }` (expresiones; por
  defecto 0) y todo lo que la refiere (`carcass: "m2"`) se mueve con ella.
  Una línea de cocina son tres carcasas con `origin.x` = 0, `module`,
  `2 * module` (`fixtures/kitchen_run`); si se pisan lo dice `FAB-101`. Los
  generadores trabajan en el espacio de su carcasa y la traslación se aplica
  al final, incluida la de los puntos de anclaje de tiradores y patas.
- Patas y zócalo: `legs: { hardware: ["leg_adjustable_100"], inset: 50,
  maxSpacing: 600, plinth?: { setback: 40, material?, clips: ["plinth_clip"] } }`
  en la carcasa. Dos filas de patas (a `inset` del frente y del fondo)
  repartidas a lo ancho por la regla de la pata; cada una es una unión
  `fixture` sobre la cara exterior de la base con el patrón de tornillos del
  herraje (`offsetAlong`/`offsetAcross`). El zócalo es un panel entre los
  laterales, retirado `setback` del frente, con clips (`fixture` sobre su
  cara interior) en cada pata delantera; el manual lo presenta después de
  las patas. El origen del mueble sigue en la cara inferior de la base:
  las patas quedan en z < 0 (los planos y el 3D lo contemplan). `SPEC-312/313`
  si el retiro no deja la base de la pata bajo el panel o el zócalo no toca
  las patas.
- Estantes: `count: N` los reparte parejos en la zona (retranqueo 20 por
  defecto). `positions: [1000, "divider_z"]` en cambio pone **estantes fijos**
  a esas alturas (cara inferior, mm desde la base): toman toda la profundidad
  interior (retranqueo 0), llevan el herraje que diga `joint` (minifix +
  tarugo para un divisor horizontal estructural) y el manual de armado los
  fija antes que los regulables. Las zonas de lo que va arriba y abajo se
  declaran igual que siempre (`"from": "divider_z + carcass.thickness"`);
  `SPEC-310` si se dan `count` y `positions` a la vez, `SPEC-311` si una
  posición no cabe o van desordenadas.
- Puertas: hasta 2 por bahía, colgadas del panel que la limita (`hinge`, por
  defecto `hinge_35_overlay`; `null` para no colgarlas). Con 1 puerta, cuelga
  a la izquierda. Una puerta que apoya sobre un divisor lo superpone a medias:
  el herraje correcto ahí es una bisagra de media superposición, y elegirlo es
  cosa del `hinge.hardware` de ese componente. `handle` agrega un tirador
  vertical a `fromEdge` del canto de apertura.
- Cajones: apilados desde el piso de su zona; el frente superpuesto, la caja
  (dos laterales, frente interior, trasera, fondo ranurado) y las correderas.
  La corredera define la profundidad de la caja (`slide.length`) y la holgura
  lateral (`slide.sideClearance`). `frontFixing` (por defecto
  `screw_4x30_face`) atornilla el frente desde adentro de la caja en una
  grilla de columnas × 2 filas; `handle` centra un tirador horizontal.

## Convenciones geométricas

- Espacio del mueble: X = ancho (izquierda→derecha), Y = profundidad
  (atrás→adelante), Z = alto. Unidad interna: mm.
- Espacio de la pieza: X = largo, Y = ancho, Z = espesor. `front` es la cara
  local +Z: **la cara donde se prefiere mecanizar** (el interior de una
  carcasa, el dorso de una puerta). Las otras caras son `back`, `left` (−X),
  `right` (+X), `bottom` (−Y), `top` (+Y).
- Cada pieza lleva un marco derecho (`placement`: origen + ejes X e Y como
  ejes del mundo con signo). No hay rotaciones arbitrarias ni trigonometría.
- Las operaciones se expresan en (u, v) de su cara, siempre medidos a lo largo
  de los ejes positivos de la pieza: un agujero en (u, v) de `back` queda
  exactamente enfrente del mismo (u, v) en `front`.

## Uniones y herrajes

Una unión es intención ("unir la tapa con el lateral izquierdo con minifix y
tarugo"). El motor:

1. encuentra el rectángulo de contacto entre las dos piezas (cara grande de
   una contra canto de la otra: la primera es la *face part*, la segunda la
   *edge part*);
2. reparte los puntos de fijación a lo largo de la línea de contacto según la
   regla de cada herraje (`endOffset`, `maxSpacing`): más largo, más herrajes;
3. por cada punto, genera las perforaciones que el herraje declara
   (`contact_face` en la face part, `edge` en el canto de la edge part,
   `face_offset` en la cara de la edge part — la excéntrica del minifix a
   34 mm del canto).

Uniones cara contra cara o canto contra canto todavía no existen: dan
`JOINT-102`/`JOINT-103` (fatal).

Además del tope (`butt`) hay dos uniones declaradas por los generadores:

- `hinge`: la puerta es la *edge part* (su canto de bisagra es la línea de
  unión) y el lateral la *face part*. La cazoleta Ø35 va en el dorso de la
  puerta a 22,5 mm del canto (`face_offset`), los pilotos a ±22,5 mm a lo
  largo (`offsetAlong`), y la base en el lateral a 37 mm del frente
  (`face_inset`). La cantidad sale de una tabla por altura
  (`countByLength`: 2 hasta 900, 3 hasta 1500, 4 hasta 2000).
- `slide`: el lateral de la caja es la *edge part* (con su cara `front`
  hacia afuera, donde va la corredera) y el lateral de la carcasa la *face
  part*. Un solo punto de fijación en el frente (`fixed: [0]`) y los huecos
  colgados de él a 37/165/293 mm hacia atrás.
- `face_to_face`: dos caras grandes apoyadas (el frente del cajón sobre el
  frente interior). Se detecta por geometría; los puntos van en una grilla:
  la regla de reparto a lo largo del lado largo y dos filas a `endOffset` de
  los bordes del lado corto (una fila si no entra). La *edge part* se
  perfora pasante (`contact_face`), la *face part* recibe el piloto.
- `handle`: una sola pieza; el generador decide el centro y la dirección, y
  los huecos cuelgan de ese punto con `offsetAlong` (±64 para 128 mm).

**Los valores de `data/hardware.json` (diámetros, profundidades, offsets) son
defaults indicativos para paneles de 18 mm.** Hay que validarlos contra el
catálogo del proveedor antes de fabricar; el motor los trata como datos.

## CAM

`cam::programs(part, profile)` produce hasta dos programas por pieza: puesta
**A** (cara `front` arriba: perforaciones y ranuras del frente, perforaciones
de canto y el contorno al final, con la fresa por fuera del rectángulo
terminado y 0,5 mm más profundo que el espesor) y **B** sólo si el dorso tiene
mecanizados (pieza girada sobre su eje X: `(u, v)` del dorso → `(u, ancho − v)`).
Origen en la esquina inferior izquierda, Z0 en la cara superior. La
herramienta sale de `profile.tools` (mecha por diámetro exacto, fresa ≤ ancho
de ranura, la compresión más ancha para el contorno); lo que falta es
`CAM-201`, y una perforación de canto sin taladro horizontal en el perfil es
`CAM-202`. Las operaciones se agrupan por herramienta (un cambio por
herramienta y puesta).

`GenericIso` emite G21/G90/G17, G81/G83 (picoteo cuando la profundidad
supera la pasada de la mecha), G01 en pasadas para ranuras (con desplazamiento
lateral si la fresa es más angosta) y contorno (concordante, exterior). La
perforación horizontal sale como `(HDRILL …)` en comentario: el ISO genérico no
tiene agregado horizontal; un post por máquina la traduce. Un test recorre
todos los programas del placard y verifica que la herramienta nunca sale de la
pieza más de un radio, nunca baja más que el espesor + 0,5 y sólo avanza con
G01 bajo la superficie.

## Simulación (§28)

Cada programa NC que emite el post se **interpreta como lo haría el control**
(`simulation.rs`: G00/G01, ciclos G81/G83 con R y Q, G80, T/M06, S/M03, M05,
M30 y los bloques `(HDRILL …)` del agregado horizontal) y se verifica en dos
niveles:

- **Máquina (nivel 3)**: rápidos dentro del material o cambio de herramienta
  con el husillo bajo (`CAM-301`), avance sin herramienta, sin husillo o sin F
  (`CAM-302`), códigos fuera del subconjunto, herramienta inexistente, sin M30
  (`CAM-303`), fuera de la mesa o de la carrera Z (`CAM-304`), Z por debajo
  del piso permitido (`profile.machine.maxCutBelowBlank`) (`CAM-305`);
  herramienta dentro de una fijación declarada en `profile.machine.fixtures`
  (prensas, topes, ventosas: rectángulo en XY y altura de su cara superior
  respecto de la placa; una ventosa bajo un pasante se detecta igual que
  una prensa en el camino del contorno) (`CAM-308`).
- **Material removido (nivel 2)**: lo que el NC corta se cruza con lo que el
  `Program` pide. Una perforación, ranura, contorno o taladro horizontal que
  no aparece en el NC a su posición y profundidad es `CAM-306`; un corte que
  ninguna operación pidió, `CAM-307`. Es la verificación del **post**: uno
  que pierda un agujero o doble una profundidad falla en todos los planes
  antes de cargar una placa (los tests lo prueban mutilando el G-code).

El nivel 1 (piezas, uniones, colisiones) son las reglas `FAB-1xx/2xx` y
`JOINT-*`. La simulación deja en el plan `machining`: por programa, cortes,
rápidos, cambios de herramienta y **tiempo estimado** con los avances del
perfil (`machine.rapidFeed`, `toolChangeSeconds`); el CLI, el informe y la
pestaña CNC de la UI lo muestran. En el paquete va `cnc/simulation.json` con
los cortes reconstruidos. Una pieza que sólo entra en la mesa de costado se
programa girada 90° (`ROTATED 90` en la cabecera; `FAB-302` ya aceptaba las
dos orientaciones, el CAM tenía que hacerlo también).

## Diagnósticos

Cada hallazgo tiene `code`, `severity` (`INFO`/`WARNING`/`ERROR`/`FATAL`),
`entity`, `location`, `message` y, cuando se puede, `suggestion`. Un `ERROR`
deja generar el plan (`status: errors`); un `FATAL` lo bloquea
(`manufacturingBlocked: true`).

| familia | qué cubre |
|---|---|
| `SPEC-*` | la spec no se puede leer o no tiene sentido (versión, campos, componentes); `SPEC-21x` es la distribución dentro de la carcasa una vez expandidos todos los componentes: dos frentes (o cajones y estantes) sobre la misma altura de una bahía (`210`, error), una bahía vacía (`211`, info), una bahía con frentes que la dejan abierta en un tramo (`212`), módulos de una hilera que se pisan o dejan una rendija ≤ 50 mm (`213`) |
| `PARAM-*` | ciclos o expresiones inválidas en parámetros |
| `LIB-*` | material, canto o herraje desconocido |
| `CON-001` | una restricción declarada no se cumple |
| `JOINT-*` | las piezas no se tocan, el tipo de contacto no está soportado, o el herraje no es del tipo de la unión (`JOINT-104`) |
| `FAB-1xx` | geometría: piezas superpuestas |
| `FAB-2xx` | mecanizado: perforación fuera de cara, distancia al borde, profundidad, cruces de perforaciones, ranura, mecha inexistente, operación no admitida, perforación que cae dentro de una ranura (`208`: el tarugo iría donde corre el fondo) |
| `FAB-3xx` | material/máquina: no sale de la placa, excede el área de trabajo, espesor incompatible con el herraje |
| `DESIGN-1xx` | lo que un carpintero diría antes de cortar; nunca bloquea. `101` luz de estantes, tapa y base mayor que `maxSpan` de la placa (pandeo; la tapa y la base miden la bahía más ancha, no la carcasa); `102` puerta de más de 600 o menos de 200 mm; `103` luz entre frentes menor a 1,5 mm; `104` frentes de cajón de menos de 100 mm o caja sin altura para la corredera; `105` cajón de más de 900 mm; `106` menos de 150 mm libres entre estantes; `107` carcasa sin fondo; `108` medidas que no parecen milímetros o profundidad de más de 1000; `109` sin canto (info a nivel mueble, aviso en frentes con `edges: none`); `110` manija pasada la mitad de la puerta, del lado de la bisagra; `111` carga: puerta más pesada que lo que aguantan sus bisagras, o cajón cuya caja más 10 kg de contenido supera la corredera (`maxLoadKg` del herraje; el peso sale de la densidad del material); `113` (info) medida escrita como número donde va un parámetro |
| `CAM-2xx` | sin herramienta para una ranura o el contorno; perforación de canto sin taladro horizontal |
| `STAGE-*` | aviso de algo que la etapa actual no cubre (ninguno activo hoy) |

**Un `FATAL` bloquea de verdad.** El paquete de un plan bloqueado trae sólo
`BLOQUEADO.txt` (los hallazgos fatales), el informe, `plan.json` y el
manifiesto: nada de DXF, programas ni despiece; el CLI sale con 1, el
servidor no emite la orden (422) y la UI deshabilita la descarga. Un `ERROR`
deja exportar y avisa; la UI pide confirmación antes de emitir una orden con
errores.

En la UI cada tarjeta de componente muestra sus hallazgos (los propios y los
de las piezas que generó) con un contador por gravedad; el 3D tiñe de rojo
las piezas con errores y de ámbar las que tienen avisos; y tocar un hallazgo
en la pestaña Hallazgos selecciona la pieza y despliega el componente.

## Límites conocidos de esta etapa

- Los componentes de una misma bahía no se reparten el espacio solos: si un
  cajón y un estante comparten zona, `SPEC-210` lo dice a nivel componente y
  `FAB-101` a nivel pieza; el que escribe la spec ajusta las zonas.
- Un estante fijo (`positions`) cruza una bahía; no hay tapa intermedia que
  cruce varias bahías atravesando los divisores verticales (serían divisores
  partidos, que es otro componente).
- El manual de armado ordena por tipo de componente, no por simulación
  física: alcanza para carcasas con tarugos y excéntricas, no detecta una
  pieza que quede inaccesible.
- El cruce ranura↔perforación no se verifica (sólo perforación↔perforación).
- El nesting es MaxRects con una sola heurística y sin búsqueda; da
  layouts válidos y reproducibles, no óptimos. El modo guillotina es por
  niveles (tiras a lo ancho de la placa, la más alta primero): cada corte va
  de borde a borde y sale numerado en tres etapas (`cuts` del layout, dibujadas
  en `nesting.svg` y en la pestaña Placas), a costa de aprovechamiento. Sin
  piezas no rectangulares.
- Varias carcasas se posicionan por `origin` absoluto; no hay restricciones
  relativas ("m2 pegada a la derecha de m1") ni piezas compartidas entre
  módulos (un lateral común), y las patas son por módulo.
- Una orden congelada antes de que existiera un campo del plan tiene que
  seguir cargando: los campos nuevos del plan van con `#[serde(default)]`
  (`machining` lo aprendió a la fuerza con órdenes de la sesión anterior).
- Los tipos de `packages/engine/src/types.ts` reflejan a mano el modelo serde.
  El test de vitest compara el plan del WASM con el `expected.json` de Rust,
  así que un desfase en la *forma* del JSON se nota; un desfase sólo en tipos
  no. Candidato a `ts-rs` cuando la superficie se estabilice.
- Los planos son SVG/HTML (imprimibles a PDF desde el navegador), no PDF
  nativo; las vistas de ensamblaje (incluida la explotada) son proyecciones
  de cajas, sin ocultamiento de aristas.
- El servicio no tiene autenticación ni PostgreSQL todavía: un directorio de
  JSON y un mutex; sirve para una estación, no para un despliegue multiusuario.
- La UI dibuja cajas y cilindros desde el plan; no hay kernel B-Rep (paso 9)
  ni medición. Las sobreescrituras de biblioteca se editan como bloque JSON
  acotado (son datos anidados), no como formulario.
- La simulación modela las fijaciones como cajas (`machine.fixtures`) contra
  la punta y el radio de la herramienta; no modela el cabezal ni el
  portaherramientas, así que una prensa alta al lado de un taladro corto no
  se detecta. Sin compensación de radio por G41/G42 (el contorno se
  calcula desplazado), sin entradas en rampa ni pestañas de sujeción (se
  asume vacío). Un solo post (ISO genérico); Biesse/SCM/Homag son posts por
  escribir sobre el mismo `Program`, y la simulación los verificaría igual
  siempre que emitan el subconjunto ISO (o se le enseñe su dialecto).
- El kernel B-Rep (Open CASCADE) es el paso 9; no hay nada de eso acá.
