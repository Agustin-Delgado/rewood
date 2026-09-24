# rewood

[![CI](https://github.com/Agustin-Delgado/rewood/actions/workflows/ci.yml/badge.svg)](https://github.com/Agustin-Delgado/rewood/actions/workflows/ci.yml)
Licencia MIT. Demo (UI sola, motor en WASM, sin servidor): https://rewood-mu.vercel.app

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
                     basic_cabinet, drawer_unit, wardrobe_1800, wardrobe_modules, wardrobe_rail,
                     bookcase_fixed, bookcase_adjustable, kitchen_run, nightstand, wall_cabinet,
                     tv_unit, desk, sideboard, invalid_cabinet
```

Dentro de `rewood-core`:

| módulo | qué hace |
|---|---|
| `expr` | lenguaje de expresiones (`width - 2 * thickness`, `if(...)`, `max(...)`, comparaciones, `and/or/not`) |
| `params` | grafo de parámetros: orden topológico, detección de ciclos, recálculo de dependientes |
| `spec` | formato de entrada propio, versionado (`schemaVersion: "1.0"`), `deny_unknown_fields` |
| `geometry` | paneles rectangulares en marcos alineados a ejes, caras semánticas, mapeo cara↔(u,v) |
| `library` | materiales, cantos, herrajes y perfil de fabricación (JSON embebido en `data/`, sobreescribible por id desde la spec) |
| `components` | generadores: `carcass`, `shelves`, `doors`, `drawers`, `rail`, `worktop`, `panel`, `modesty` → piezas + pedidos de unión; `when` decide cuáles entran |
| `options` | opciones de plantilla: cotas resueltas, valores fuera de rango (`SPEC-50x`) |
| `stagger` | agujeros de dos uniones que se cruzan en un panel: corre el herraje que se puede correr a lo largo de su línea |
| `joints` | siete tipos de unión (`butt`, `hinge`, `slide`, `face_to_face`, `handle`, `fixture` para patas, clips y colgadores, `row` para hileras Sistema 32), reparto de herrajes a lo largo (y en filas) de la unión, perforaciones por herraje |
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

`package` escribe (en un directorio vacío, o encima de un paquete anterior, que se borra entero: un programa viejo no queda al lado del nuevo) `parts/P001.dxf…` (para importar en un CAM), `cnc/P001_A.nc…`
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
profundidad (`DRILL_FRONT_D35_L12_5`, `DRILL_EDGE_LEFT_D8_L34`, `GROOVE_FRONT_W3_2_L8`; R12 no admite punto en el nombre de capa, así que el decimal va con guion bajo).

**Pedido al proveedor de placas** (`proveedor/`, también suelto desde el botón
*Pedido al proveedor* de la UI y como `?role=supplier` en el servidor): lo que
necesita quien corta, cantea y agujerea, y nada más. No lleva herrajes, precios,
nuestro nesting ni G-code. Trae tres cosas:

- `despiece.csv`: planilla para Excel en es-AR (`;`, coma decimal, BOM), con
  código, pieza, cantidad, material, color con su código Faplac, espesor, largo y
  ancho **finales con el canto incluido** (al décimo de mm), veta y canto de L1/L2
  (lados largos) y A1/A2 (lados cortos). Cierra con un resumen de mecanizado y el
  número de plano.
- `planos.html`: las convenciones, un resumen de m² por placa y color y de metros
  de canto, el despiece, y un plano por pieza mecanizada. Se imprime a PDF, A4
  apaisado.
  - Cada plano va visto desde la cara A (la que queda hacia adentro; en puertas y
    frentes, la de atrás), con los cantos nombrados y marcados, cotas por
    coordenadas desde la esquina 0,0 y el tamaño total.
  - Las perforaciones van agrupadas por Ø, profundidad y X. Las de la cara B van
    en el sistema de coordenadas de la cara B, con la pieza girada sobre su largo.
  - Las perforaciones de canto van a lo largo del canto; también figuran ranuras y
    calados.
- `dxf/`: un DXF por pieza mecanizada, con `LEEME.txt` sobre sus capas.

Los vidrios y espejos no entran: van al vidriero.

**Color.** Una placa de melamina se vende en varios diseños (`decors` en
`materials.json`: Faplac líneas Lisos y Nature con su código, y el Wengue de
Egger, que viene en placa de 1830 × 2600 y así la usan el nesting y las compras). `decor` elige el
del mueble y `frontDecor` el de puertas y frentes de cajón. Sin `decor` se usa el
`defaultDecor` de la placa, Blanco Nature, y un color desconocido da `LIB-106`.

- El color va en la pieza, no cambia la geometría. Dos colores nunca comparten una
  placa en el nesting, y el BOM y las compras los separan, canto incluido: el canto
  se compra en el color de la pieza.
- Los diseños de madera llevan veta; un liso se puede girar al cortar.
- El 3D pinta cada pieza con el color que eligió la persona.

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
| `GET /manufacturing-orders`, `GET /manufacturing-orders/:id`, `GET …/:id/package[?role=cnc\|cutting\|assembly\|purchasing]` (zip), `GET …/:id/package/<ruta>` | consultar y bajar lo congelado; con `role`, sólo los archivos de ese proveedor (§52 multi-proveedor: el CNC recibe programas y DXF, la seccionadora despiece y nesting, el armador la documentación, compras las órdenes; el taller recibe `documentation/report_taller.html`, el informe sin costos ni órdenes de compra) |
| `GET …/:id/production`, `POST …/:id/production/steps` `{ part?, step, done }`, `POST …/:id/production/status` `{ status }` | seguimiento de producción (§52): pasos `cut`/`machined`/`edged` por pieza y `assembled`/`delivered` por orden, cada pieza sólo con los pasos que le tocan (un fondo sin agujeros ni canto se corta y nada más); el estado sale de los pasos (`planned`→`in_progress`→`done`) salvo `cancelled`, que es una decisión: una orden cancelada no avanza y al descancelarla sigue donde estaba, y `done` a mano pide todos los pasos hechos; eventos. Leer, cambiar y guardar va bajo un mismo lock (dos pedidos a la vez no se pisan). Es el lado mutable de la orden, el snapshot no se toca |
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

El servidor guarda la spec de cada mueble y de cada orden como JSON y la lee
recién al compilar: una orden escrita por un motor anterior abre aunque
algún campo haya cambiado de nombre (la spec sigue siendo estricta al
entrar, y un perfil congelado acepta campos que no conoce). Los ids de la URL
sólo llegan al disco con su forma (`prj-000001`), leer-cambiar-guardar va bajo
un lock, compilar corre fuera de los hilos de la API, y los errores de cuerpo
salen como `{ "error": … }`. CORS abre sólo a `localhost`, `127.0.0.1` y la
demo publicada; `--cors <origen>` suma otros (sin login, abrirlo a cualquier
página dejaba a cualquier sitio leer y cambiar los proyectos).

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
mueble de 400 de fondo y después lo corrige. La UI ya no tiene pestaña de
asistente: el endpoint sigue en el servidor. No se probó contra la API real en este entorno
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

**Catálogo y diseño.** El botón del encabezado abre el catálogo: categorías
(placares, escritorios, cocina, dormitorio, living, bibliotecas, baño,
oficina y recibidor) y, en cada
una, sus variantes como tarjetas con el alzado frontal del plan que compila
el motor para esa variante (`lib/catalog.ts`). Una variante es una plantilla
del repo más los valores de sus opciones: "escritorio con cajonera" y
"escritorio simple" son `fixtures/desk` con los lados puestos distinto;
"placard con cajonera doble" es `fixtures/wardrobe_modules` con dos módulos
de cajones. `packages/engine/test/catalog.test.ts` compila cada variante con
su preset y falla si alguna da ERROR o FATAL: lo que el catálogo ofrece se
fabrica, igual que las opciones que barre la auditoría. La pestaña **Diseño** dibuja las opciones que el plan trae
resueltas (`plan.options`): números con deslizador o con botones cuando
son pocos valores, interruptores, elecciones; sólo las que aplican (los
cajones de la izquierda aparecen si a la izquierda hay cajonera), con las
cotas que calculó el motor (el ancho de la cajonera baja si hay dos) y el
hallazgo con su arreglo si un valor queda afuera. Arriba, la variante, las
medidas totales, piezas, herrajes y si es fabricable.

Árbol de piezas por componente con nombres legibles ("Cajones · módulo 2",
cada cajón plegado con sus seis piezas, medidas redondeadas; mostrar/ocultar
por componente), vista 3D con
cada pieza como caja en su `aabb` con su contorno, perforaciones como cilindros
y ranuras como huecos (calculados desde el mismo (u, v) del plan, no geometría
propia), y **los herrajes dibujados como lo que son** donde el plan puso sus
agujeros (`lib/hardware3d.ts`, a partir del `source` de cada operación):
tarugo, excéntrica con su perno, tornillo, cazoleta con brazo y base, dos
guías por corredera, tirador o botón, pata, cierre y su placa, pasador de
estante; clickeables como antes. Un deslizador de vista explotada (misma regla
que `export/explode.rs`) y un botón **abrir** que gira cada puerta sobre la
línea de sus bisagras y saca cada cajón sobre sus correderas (doble clic en
una puerta o un cajón abre sólo ése; `lib/motion.ts`); en los dos cada
herraje viaja con la pieza que lo lleva, lo que une dos piezas flota entre ambas y una línea guía une los dos
agujeros que aparea. Botones de vista: perspectiva, alzados frontal y
posterior, laterales y planta (sin perspectiva, como los planos; se pueden
orbitar igual). Agujeros, ranuras y herrajes se dibujan instanciados (una
llamada por forma) y el plan vive fuera del proxy reactivo de Svelte: mover el
explotado o un deslizador de medida no reconstruye la escena. Panel de
parámetros con los nombres de la plantilla que recompila en vivo, valores
calculados plegados, detalle de la pieza seleccionada, editor de componentes por formulario (agregar, quitar,
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
  "edgeMaterial": "pvc_0_45mm",
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
  frentes vecinos se encuentran en el centro del divisor con una luz. En
  altura, igual: donde una zona termina dentro de la carcasa cada frente
  superpuesto deja media luz, así cajones y puerta quedan a una luz y no a
  dos. Con `frontHeight` los cajones ocupan sólo lo que cubren sus frentes.
- Varias carcasas: cada una acepta `origin: { x, y, z }` (expresiones; por
  defecto 0) y todo lo que la refiere (`carcass: "m2"`) se mueve con ella.
  Una línea de cocina son tres carcasas con `origin.x` = 0, `module`,
  `2 * module` (`fixtures/kitchen_run`); si se pisan lo dice `FAB-101`. Los
  generadores trabajan en el espacio de su carcasa y la traslación se aplica
  al final, incluida la de los puntos de anclaje de tiradores y patas.
  `origin.rotation` (0, 90, 180 o 270; `SPEC-332` si no es múltiplo de 90)
  gira la carcasa y lo que cuelga de ella alrededor de su origen, de a
  cuartos de vuelta: todo se genera y se une mirando a +Y y el giro se
  aplica a piezas y uniones antes de las reglas, exacto (sólo signos). A
  270° el frente mira a +X y el ancho corre hacia −Y: la hilera de la pared
  de al lado en una cocina en L (`fixtures/kitchen_corner`). Las hileras se
  controlan cada una en su marco (`SPEC-213`) y la bisagra `auto` busca el
  medio de su propia hilera. Una tapa (`worktop`) apoya sobre módulos
  girados con su huella real, y sus uniones con ellos se resuelven una vez
  girados, en el espacio del mueble (numeradas a continuación de las
  demás): `fixtures/bed_drawers` es una base de cama con dos cajoneras a
  90° y 270° (los cajones salen a los costados), sobre patas de 100 mm con
  zócalo retirado para que los frentes no rocen el piso, y la tapa de MDF
  encima.
- Cierres sobre un divisor: la puerta de la bahía de la izquierda baja el
  suyo 40 mm, así no queda tornillo contra tornillo con el de la bahía de al
  lado. La base de una bisagra que cae sobre un agujero del Sistema 32 que
  ya atraviesa el panel (soportes de los dos lados de un divisor) usa ese
  agujero.
- Recibidor: `fixtures/shoe_cabinet`, zapatero con puertas push-open o banco
  zapatero abierto, de 1 a 4 huecos con estantes cada par de zapatos.
- Puertas basculantes y rebatibles: `opening: "up"` cuelga la puerta de la
  tapa (bisagras en su canto de arriba, pistones a gas `lift_stay` a cada
  lado) y `opening: "down"` de la base (compases `flap_stay`); el tirador
  va a lo largo del canto libre. Una sola por bahía, sin cierre ni lado de
  bisagra, y su zona tiene que llegar a la tapa o la base: `SPEC-340`. La
  auditoría mide la cazoleta y la base a lo largo de la línea de bisagra,
  sea vertical u horizontal. `wall_cabinet` con `lift` (alacena
  basculante) y `tv_unit` con `sides: 3` (tapas rebatibles).
- Ruedas: `legs: { hardware: ["caster_50"] }` (una pata con
  `leg.caster`): el mueble anda, y con zócalo es `SPEC-338` (el arreglo lo
  saca). `fixtures/mobile_pedestal`: cajonera con ruedas para ir bajo el
  escritorio (un `constraint` avisa si con las ruedas no pasa).
- Carpetas colgantes: `drawers.files: {}` atornilla un par de rieles
  (`file_rails_legal`, uno por caja en la BOM) arriba de los laterales de
  cada caja; la caja tiene que medir por dentro lo que abarcan las carpetas
  (370–385 para oficio: varilla de 40 cm, rieles a 38 entre ejes; con el
  ancho de la carcasa como arreglo) y 250 de
  alto (con `boxHeight` como arreglo): `SPEC-339`. `fixtures/filing_cabinet`.
- Vidrio y espejo: un material `outsourced` (`glass_6`, `mirror_4`) lo
  corta, pule y perfora la vidriería: sus piezas no entran al nesting, no
  llevan canto ni programa de CNC, el despiece las marca "a medida
  (vidriería)" y la compra va por m² (`pricePerM2`) al proveedor
  `glass_supplier`. Una puerta de vidrio (`material: "glass_5"`, float de
  5 mm) cuelga de `hinge_glass_overlay`, la bisagra a presión Häfele
  361.93.641: toma el vidrio sin perforarlo (hasta 5 mm), se atornilla al
  lateral, viene de a pares con retén y sirve para puertas de hasta 450 × 600
  (`hinge.maxDoor`; más grandes es `SPEC-337`, error). Una bisagra de placa
  en una puerta de vidrio, o al revés, es `SPEC-337` con la otra como
  arreglo.
  `facing: {}` pega un espejo sobre el frente de cada puerta (3 mm adentro
  del borde, un cartucho de `mirror_adhesive` por puerta; con tirador es
  `SPEC-337`: abre con push-open). `fixtures/medicine_cabinet` (botiquín) y
  `fixtures/display_cabinet` (vitrina colgante de dos puertas de vidrio y
  estantes de templado de 6 sobre soportes, `maxSpan` 800).
- Bacha (`sink`): una bacha de la biblioteca (`kind: sink`) sobre la tapa
  de una carcasa, centrada en su bahía (`bay`) y a media profundidad
  (`fromFront`, `offset`). La bacha dice qué recortes le hace a la tapa
  (`cutouts`: el cuenco de una de embutir, sólo el desagüe de una de apoyo)
  y cuánto cuelga bajo la tapa (`sink.below`), que la bahía no puede usar
  para otra cosa (`SPEC-210`). `passage: {}` abre el fondo para los caños
  (`pipe_passage`, a media altura o en `height`). Un recorte que no entra en
  la bahía con 45 mm a cada lado (ahí están los excéntricos de la tapa) es
  `SPEC-336`. `fixtures/vanity`: vanitory colgante con bacha de embutir o de
  apoyo.
- Puertas corredizas (`sliding_doors`, `count` 2 a 4): cruzan toda la
  carcasa dentro del hueco, sobre un riel doble bajo la tapa y sobre la base
  (`track`, por metro, con su bloque `sliding`: carriles, paso, lo que ocupa
  desde el frente y las luces arriba y abajo). Alternan carril (la primera
  atrás) y se solapan `overlap` (30) donde se cruzan; cada una lleva dos
  ruedas abajo y dos guías arriba atornilladas al dorso, y el riel va
  atornillado cada 300 mm. Los divisores tienen que quedar detrás del riel
  (`dividerSetback` en la carcasa; `SPEC-335` con el valor como arreglo), y
  los estantes detrás del plano de las puertas (`SPEC-215`, con el
  retranqueo como arreglo; vale igual para una puerta embutida); los
  cajones, interiores y retranqueados (`SPEC-214`). Peso de la puerta contra
  sus dos ruedas (`DESIGN-111`), más de 1200 de ancho (`DESIGN-102`).
  `fixtures/wardrobe_sliding`. En el visor, "abrir" corre las puertas del
  carril de adelante sobre las de atrás.
- Frente fijo (`doors` con `fixed: true`): un panel ciego superpuesto,
  sin bisagra, tirador ni cierre (`SPEC-334`), unido con tarugos (`fixing`)
  a los cantos que tapa lo suficiente (laterales, tapa y base; un divisor,
  tapado sólo hasta su medio, queda afuera). Cierra la parte ciega de un
  esquinero: `kitchen_corner` es un módulo de dos bahías con el frente fijo
  en la primera, tan ancha como lo que ocupa la otra hilera más una luz, y
  la puerta en la segunda.
- Patas y zócalo: `legs: { hardware: ["leg_adjustable_100"], inset: 50,
  maxSpacing: 600, plinth?: { setback: 40, material?, clips: ["plinth_clip"] } }`
  en la carcasa. Dos filas de patas (a `inset` del fondo; la delantera, si
  hay zócalo, detrás de él: `setback` + espesor del zócalo + radio de la base,
  así el zócalo nunca atraviesa una pata) repartidas a lo ancho por la regla de la pata; cada una es una unión
  `fixture` sobre la cara exterior de la base con el patrón de tornillos del
  herraje (`offsetAlong`/`offsetAcross`). El zócalo es un panel entre los
  laterales, retirado `setback` del frente, con clips (`fixture` sobre su
  cara interior) en cada pata delantera; el manual lo presenta después de
  las patas. El origen del mueble sigue en la cara inferior de la base:
  las patas quedan en z < 0 (los planos y el 3D lo contemplan). `SPEC-312/313`
  si el retiro no deja la base de la pata bajo el panel o las dos filas no
  entran en la profundidad. `FAB-102` vigila lo mismo desde afuera: el volumen
  de cada pata (su base, a toda su altura) contra toda pieza y contra las
  otras patas.
- Estantes: `count: N` los reparte parejos en la zona (retranqueo 20 por
  defecto). Con `support: "pins"` apoyan en soportes: no llevan unión; cada
  estante baja a la línea de la grilla Sistema 32 más cercana (`row`, herraje
  `shelf_pin_row_5`: Ø5×12 cada 32 mm, a 37 del frente y a 37 del fondo,
  desde 37 sobre el piso de la carcasa, una línea libre bajo y sobre la
  zona) y los paneles llevan **sólo los agujeros donde apoya**: cuatro por
  estante. `pins: { adjust: 96 }` los hace **regulables**: suma ese recorrido
  (en mm, de a 32) arriba y abajo de cada estante, y los tramos que se tocan
  se unen en una hilera; un `adjust` tan alto como la zona da la hilera
  completa de siempre (`fixtures/bookcase_adjustable` usa 96). El estante queda 1 mm corto por lado y cuatro `shelf_pin_5` por
  estante van a la BOM por cantidad. Las cazoletas de las bisagras se
  ajustan a esa grilla (53 + 32k sobre el piso), así los dos agujeros de su
  base caen exactamente en la hilera y no se perforan dos veces (un
  agujero de hilera ya hecho absorbe el de la base). La grilla se mide desde
  el piso de la carcasa esté donde esté (una cajonera sobre zócalo, con
  `origin.z`, la tiene corrida con ella). En una puerta baja, dos bisagras
  que caen en la misma línea de grilla se separan a la siguiente. En una alacena colgada la
  hilera trasera termina bajo los colgadores. El manual coloca los
  regulables al final, con el mueble cerrado. `positions: [1000, "divider_z"]` en cambio pone **estantes fijos**
  a esas alturas (cara inferior, mm desde la base): toman toda la profundidad
  interior (retranqueo 0), llevan el herraje que diga `joint` (minifix +
  tarugo para un divisor horizontal estructural) y el manual de armado los
  fija antes que los regulables. Las zonas de lo que va arriba y abajo se
  declaran igual que siempre (`"from": "divider_z + carcass.thickness"`);
  `SPEC-310` si se dan `count` y `positions` a la vez, `SPEC-311` si una
  posición no cabe o van desordenadas.
- Puertas: hasta 2 por bahía, colgadas del panel que la limita (`hinge`, por
  defecto `hinge_35_overlay`; `null` para no colgarlas). Con 1 puerta, cuelga
  de donde diga `hingeSide`: `auto` (por defecto) la aleja de una cajonera
  vecina a su altura y, si no hay, del medio del mueble, así dos puertas
  vecinas abren hacia afuera; `left`/`right` la fuerzan. **La bisagra nombra la familia (cazoleta Ø35, ángulo de
  apertura); el brazo lo decide el motor por la geometría**: superpuesta
  sobre un lateral, **media superposición** (`hinge_35_half`, codo 9) sobre
  un divisor que comparte con la puerta vecina, embutida con `mount: inset`.
  `softClose: true` cambia por la variante con cierre suave de la misma
  familia (`false` por la lisa; sin el campo, queda la que dice `hinge`).
  `SPEC-314` cuando la biblioteca no tiene esa variante (la de 165° no viene
  embutida). `catch` agrega un **cierre**: `{ "hardware": ["magnetic_catch"] }`
  (imán) o `["push_latch"]` (push-open): el cuerpo va en la cara interior del
  panel opuesto a la bisagra, a ras del dorso de la puerta y a media altura,
  con los tornillos fuera de la línea de 37 donde van bases, correderas e
  hileras; la placa (`catch.strike` del herraje) en el dorso de la puerta,
  enfrentada. Con dos puertas por bahía se encuentran lejos de los laterales:
  el cierre va bajo la tapa (o sobre la base) a 25 del canto de apertura de
  cada una, y si la zona no llega a ninguna, `DESIGN-116` avisa que no se
  colocó. Un push-open con bisagras de cierre suave es `DESIGN-116`
  (el amortiguador lo anula); con tirador, info. `handle` agrega un tirador
  vertical a `fromEdge` del canto de apertura. `span: 2` hace que el juego
  cubra dos bahías consecutivas desde `bay` (una puerta ancha sobre dos
  bahías angostas; las puertas cuelgan de los paneles exteriores y el
  divisor queda atrás; `SPEC-204` si no hay tantas bahías). `mount: inset`
  la **embute**: queda dentro del hueco, a ras del frente de la carcasa,
  entre tapa y base, y una embutida no puede cruzar bahías porque el divisor
  queda en su plano (`SPEC-317`).
- Cajones: apilados desde el piso de su zona; el frente superpuesto, la caja
  (dos laterales, frente interior, trasera, fondo ranurado) y las correderas.
  La corredera define la profundidad de la caja (`slide.length`) y la holgura
  lateral (`slide.sideClearance`); `softClose: true` la cambia por la
  variante con cierre suave del mismo largo y estilo (`SPEC-321` si no
  existe: las de rodillo no la tienen). `frontFixing` (por defecto
  `screw_4x30_face`) atornilla el frente desde adentro de la caja en una
  grilla de columnas × 2 filas; `handle` centra un tirador horizontal.
  `mount: inset` los hace **cajones interiores**: el frente va dentro del
  hueco (entre los paneles, con la luz) y una puerta puede cerrar sobre la
  pila; `setback` los retranquea del frente de la carcasa —hace falta el
  espesor de la puerta más la luz cuando la puerta es embutida, y si no se
  pone el motor lo dice con `SPEC-214` y ofrece el valor—. Detrás de una
  puerta, la hoja abierta queda parada delante del hueco del lado de la
  bisagra y el cajón saldría contra ella: el motor lo mide en una primera
  pasada y monta la pila sobre **suplementos** de ese lado: tiras de
  melamina de 18 mm, una o dos (`slide_spacer_18/36`), las que hagan falta
  para dejar 3 mm libres; corren caja y frente hacia adentro. El suplemento
  va en la unión de la corredera (uno por corredera en la BOM, dibujado en el
  visor) y el manual de armado lo nombra.
- Alacena colgada: `hanging: {}` en la carcasa (herraje `cabinet_hanger`
  por defecto) pone un colgador regulable en la cara interior de cada
  lateral, contra la tapa y a 40 del fondo (unión `fixture`, tres tornillos),
  para engancharlo a un riel de pared. Sin patas: `fixtures/wall_cabinet`.
- Tapa de trabajo: `{ "type": "worktop", "carcasses": ["left", "right"],
  "overhang": { "front": 20, "back": 0, "sides": 0 } }` tiende un panel sobre
  una o varias carcasas (todas si no se nombran) de la más izquierda a la
  más derecha, con el vuelo pedido, y lo atornilla desde adentro de cada
  carcasa a través de su tapa (`face_to_face`, `screw_4x30_face` a 80 mm de
  los bordes para esquivar las excéntricas de la tapa). El hueco entre dos
  cajoneras queda abierto: un escritorio (`fixtures/desk`). `SPEC-319` si
  las carcasas no terminan a la misma altura; `DESIGN-115` con más de 2400
  de largo o más de 1200 de luz entre carcasas.
- Patas: una bajo cada lateral y **una bajo cada divisor** (ahí baja la
  carga, y una pata repartida pareja caía con sus tornillos sobre los
  agujeros de unión del divisor), más las que pida `maxSpacing` entre
  medio.
- Barral: `{ "type": "rail", "bay": 1, "fromTop": 60 }` cuelga un barral a
  `fromTop` bajo la tapa, centrado en la profundidad útil: dos soportes
  (`supports`, por defecto `rail_support_oval`, uniones `fixture` sobre las
  caras interiores de los paneles de la bahía) y la barra (`hardware`,
  `rail_oval_30`) que va a la BOM por metro (`0.573 m`), sin pieza. Publica
  `rail.height`, `rail.length` y `rail.hang` (altura libre debajo). Lo
  colgado ocupa 1500 mm bajo el barral: estantes o cajones ahí son
  `SPEC-210`; menos de 900 de altura libre es `DESIGN-114`. `SPEC-315` si
  no queda altura, `SPEC-316` si el herraje no es un barral. El 3D lo
  dibuja como barra entre sus dos soportes y el manual lo coloca al final.
  `fixtures/wardrobe_rail` junta barral, cajones interiores retranqueados y
  puerta embutida a la izquierda con puerta superpuesta a la derecha.
- Componentes opcionales: todo componente acepta `when`, una condición sobre
  los parámetros (`"when": "drawers > 0"`, `"when": "pedestal"`). Si da
  `false` el componente no se genera, y tampoco lo que cuelga de él
  (estantes o puertas de una carcasa apagada, el faldón de una tapa
  apagada); el plan los lista en `inactive`. Una condición que no da
  verdadero o falso es `SPEC-103`. Las restricciones también aceptan `when`
  (la luz para las piernas no aplica sin cajonera).
- Rango de bahías: `bay` + `lastBay` en estantes, puertas, cajones y barral
  repite el componente en cada bahía del rango, cada una con su juego (una
  cajonera doble son dos pilas lado a lado, cada una entre sus paneles).
  En puertas, `span` sigue agrupando bahías bajo una misma puerta.
  `SPEC-204` si el rango está vacío o se sale de la carcasa.
- Lateral de apoyo: `{ "type": "panel", "x": "width", "facing": "left" }` es
  un panel vertical suelto que sostiene una tapa (la punta de un escritorio
  sin cajonera): `x` es su cara exterior y `facing` hacia dónde mira la
  interior, donde van los agujeros. La tapa de trabajo apoya en carcasas y
  laterales (todos si no nombra ninguno) y se une al canto superior del
  lateral con su `joint` (minifix + tarugo). Faldón: `{ "type": "modesty",
  "height": 300, "inset": 20 }` pone un panel en cada hueco entre apoyos
  bajo la tapa, a `inset` del fondo, unido a los dos apoyos; es lo que
  impide que un escritorio sobre laterales se mueva de costado, así que sin
  él (o con un lateral que no sostiene nada) el motor avisa `DESIGN-117`.
  `SPEC-322/323` si no hay medida o tapa.
- Opciones: `"options": [{ "param": "drawers", "label": "Cajones", "group":
  "Cajonera", "min": 1, "max": "floor(height / 150)", "step": 1, "when":
  "pedestal" }]` declara lo que una persona elige sobre esa plantilla, cada
  opción atada a un parámetro literal: un parámetro booleano es un
  interruptor, `choices: [{ "value": 0, "label": "lateral" }, …]` una
  elección, lo demás un número entre cotas. Las cotas son expresiones (el
  ancho mínimo del placard depende de cuántos módulos tiene; el ancho de la
  cajonera, de si hay una o dos). El motor las resuelve y las pone en
  `plan.options`; un valor fuera de cota es `SPEC-503` con el arreglo
  "dejarlo en …", una opción sobre una fórmula `SPEC-502`, sobre un
  parámetro que no existe `SPEC-501`. Las opciones no cambian cómo se
  construye nada: lo cambian los parámetros que mueven. **Toda cota es
  fabricable**: el banco de auditoría (`tests/audit.rs`) compila cada fixture
  con cada opción en cada extremo (y en cada elección, más las opciones que
  esa elección enciende) y exige lo mismo que a los fixtures: ningún ERROR
  ni FATAL, ninguna pieza que pise a otra, ningún agujero que no aparee.

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
- `row`: un segmento sobre una cara de una pieza, un agujero por punto de
  fijación al paso del herraje (`placement.pitch`): las hileras Sistema 32
  de los estantes regulables. Sin ítems de BOM; los soportes los cuenta el
  componente.

`crates/rewood-core/tests/audit.rs` es la auditoría de banco: sobre todos
los fixtures y una grilla de variantes (tamaños, 1 o 2 puertas,
superpuestas o embutidas, con cierre magnético o cierre suave, tiradores
de 96 a 320 y botón, pilas de cajones sobre correderas telescópicas, con
cierre suave o de rodillo, placares de 2 a 4 bahías con push-open,
estantes regulables detrás de puertas) verifica lo que un carpintero
mediría: que ninguna pieza comparta volumen, que los dos agujeros de cada
fijación coincidan en el espacio (tarugo, perno y rosca, tornillos de
corredera enfrentados a 12,7 mm), que la excéntrica esté a 34 del canto y
en una cara interior, la cazoleta a 22,5 del canto en el dorso de la puerta
y su base a 37 del frente del lateral a la altura de la cazoleta, que el
brazo de la bisagra sea el del panel (superpuesta en un lateral, media en
un divisor, embutida adentro), tiradores pasantes a la distancia que declara
el herraje y del lado de apertura, cierres a ras del frente con su placa
enfrentada en el dorso de la puerta, cajas de cajón con su corredera
adentro, frentes a ras del plano correcto y ningún agujero fuera de su cara
ni más profundo que el panel. Una violación imprime todas las que haya.

### Qué hay en la biblioteca

**Biblioteca del taller.** Los estándares son el punto de partida, no un
límite: la pestaña **Biblioteca** de la UI muestra placas, cantos y herrajes
(agrupados por tipo, con los datos que importan de cada uno: largo y luz de
una corredera, puerta máxima de una bisagra de vidrio, descuento de un kit
corredizo, recorte de una bacha, entre centros de una manija, precios de
compra) y deja cambiarlos, volver cada uno al estándar, duplicar uno para
cargar otra medida que el taller compra, y exportar o importar la
biblioteca entera. Los cambios son sobreescrituras `libraries` (por id: un
id existente se parchea clave por clave, uno nuevo va completo), valen para
todos los muebles, los recuerda el navegador y viajan dentro de la spec que
se compila, se descarga o se guarda en el servidor: el plan sale igual en
cualquier lado. Los de un mueble solo siguen en Componentes y ganan sobre
los del taller (`lib/workshop.ts`; `packages/engine/test/workshop.test.ts`).

Todo de medida estándar y a la venta en Argentina (Faplac/Arauco, Egger,
Eurohard/Grupo Euro, Häfele, Ducasse, Ferrum), en
`crates/rewood-core/data/hardware.json` y `materials.json`. Un herraje o una
placa nueva tiene que serlo también:

- placas: melamina y MDF de 18 en 1830 × 2750, fondo de fibrofácil blanco
  de 3 en 1830 × 2600, fenólico de 18 en 1220 × 2440; canto `pvc_0_45mm`
  (tapacanto PVC 22 × 0,45, el de los fixtures) y `pvc_2mm` (22 × 2);

- unión: `minifix_15` (excéntrica Ø15 + perno B34), `dowel_8x30`,
  `confirmat_7x50`, `screw_4x30_face`;
- bisagras cazoleta Ø35: `hinge_35_overlay` (codo 0) / `hinge_35_half`
  (codo 9) / `hinge_35_inset` (codo 18), a 110°, cada una con variante
  `_soft`, y
  `hinge_35_overlay_165` (gran ángulo);
- correderas: telescópicas a bolillas `slide_ball_250` … `slide_ball_600`
  cada 50 mm (extracción total, 30 kg) con variantes `slide_ball_soft_*`, y
  de rodillo `slide_roller_300` … `slide_roller_500` (extracción parcial,
  25 kg, sin cierre suave);
- manijas barral `handle_bar_96/128/160/192/320` y botón `knob_single`;
- la pata plástica regulable 100–150 con clip de zócalo, puesta a 100, 120
  o 150 (`leg_adjustable_100/120/150`), y `plinth_clip`;
- cierres `magnetic_catch` (+ `magnetic_strike`) y `push_latch`
  (+ `push_latch_plate`);
- Un ítem de la BOM de un herraje puede llevar `through: [desde, hasta)`:
  vale sólo para el herraje cuyos agujeros pasantes cruzan esa cantidad de
  placa. Así el tirador pide M4×25 en una puerta (18 mm) y M4×45 en un
  frente de cajón con su frente interior detrás (36 mm).
- `hinge_glass_overlay` (bisagra para puerta de vidrio) y `mirror_adhesive`.
- `lift_stay` (pistón a gas) y `flap_stay` (compás), cada uno con su
  soporte en la puerta.
- `caster_50` (rueda con freno, `leg.caster`) y `file_rails_legal`
  (rieles para carpetas colgantes oficio).
- `sink_ferrum_imola` (bacha de embutir Ferrum Imola 47 × 39, recorte
  425 × 345 r40 —el definitivo es el de la plantilla de Ferrum—, cuelga
  195), `sink_vessel_400` (de apoyo, sólo el desagüe de 1 1/4", Ø45) y
  `pipe_passage` (200 × 160 en el fondo): herrajes que recortan en vez de
  perforar.
- `sliding_kit_2m/3m/4m`: el kit corredizo de aluminio para dos puertas de
  18 mm (rieles, ruedas, patines, perfiles y burletes; 45 kg por puerta).
  El motor toma el más corto que cubre la abertura; cada puerta mide la
  mitad de la abertura menos 7 y 46 menos de alto, como pide el kit (el
  perfil del riel —carriles, profundidad— es indicativo: mirá la ficha). Más
  `track_screw`, `sliding_roller` y `sliding_guide` para atornillarlo.
- `slide_spacer_18/36` (`kind: spacer`): suplementos de melamina de 18 (una
  o dos tiras) entre el panel y la corredera de un cajón interior.
- `shelf_pin_row_5` + `shelf_pin_5` (Sistema 32), `rail_oval_30` +
  `rail_support_oval`, `cabinet_hanger`.

**Los diámetros, profundidades y offsets de `data/hardware.json` son los
usuales para paneles de 18 mm** (el patrón de cada marca puede variar un
poco): conviene validarlos contra la ficha del proveedor antes de fabricar;
el motor los trata como datos.

## CAM

`profile.workflow` dice qué recibe la máquina. `banded_panels` (por defecto):
seccionadora → canteadora → CNC; el programa trabaja la pieza ya cortada y
canteada, a medida terminada, y no corta contorno. `nested_router`: el router
trabaja la placa en bruto, a medida de corte; todo punto de una cara se corre
el espesor del canto izquierdo e inferior, el contorno se corta al final a
medida de corte (fresa por fuera, 0,5 mm más profundo que el espesor) y las
perforaciones de canto van a una puesta **E** aparte, después de cantear,
sobre la pieza terminada. El `cnc/README.txt` del paquete dice cuál es.

`cam::programs(part, profile)` produce hasta tres programas por pieza: puesta
**A** (cara `front` arriba: perforaciones y ranuras del frente, perforaciones
de canto salvo en el router, y el contorno en el router), **B** sólo si el
dorso tiene mecanizados (pieza girada sobre su eje X: `(u, v)` del dorso →
`(u, ancho − v)`) y **E** en el router. Origen en la esquina inferior
izquierda, Z0 en la cara superior. Un recorte pasante (`CUTOUT`: rectángulo
con esquinas redondeadas o círculo) sale en la puesta de la cara donde se
dibujó, con la fresa más ancha que dobla sus esquinas (radio ≤ el del
recorte), por dentro del contorno, en pasadas hasta 0,5 mm más que el
espesor; la simulación exige el contorno entero a la profundidad final y
acepta la fresa dentro de ese corredor. En el DXF va en la capa `CUTOUT`. La
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
  (`CAM-303`), una pasada o un ciclo que baja más de lo que admite la
  herramienta de una vez (`CAM-309`), fuera de la mesa o de la carrera Z (`CAM-304`; este y
  `CAM-301` son `FATAL`: un programa que choca la máquina no sale), Z por debajo
  del piso permitido (`profile.machine.maxCutBelowBlank`) (`CAM-305`);
  herramienta dentro de una fijación declarada en `profile.machine.fixtures`
  (prensas, topes, ventosas: rectángulo en XY y altura de su cara superior
  respecto de la placa; una ventosa bajo un pasante se detecta igual que
  una prensa en el camino del contorno) (`CAM-308`).
- **Material removido (nivel 2)**: lo que el NC corta se cruza con lo que el
  `Program` pide. Una perforación, ranura, contorno o taladro horizontal que
  no aparece en el NC a su posición y profundidad es `CAM-306`, igual que una
  ranura cuyas pasadas a fondo no cubren su ancho (o se pasan); un corte que
  ninguna operación pidió, `CAM-307`: un agujero de más o un fresado bajo la
  superficie fuera del corredor de toda ranura y del contorno. Es la verificación del **post**: uno
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

### Agujeros que se cruzan en un panel

Un divisor con estantes a la misma altura de los dos lados, dos cajoneras
que lo comparten, el tornillo de una corredera frente a la base de una
bisagra: dos uniones perforan el mismo punto desde caras opuestas. Antes de
las reglas, `stagger` corre uno de los dos a lo largo de su línea de unión,
como haría un carpintero: un tarugo, una excéntrica o un tornillo de frente
de a 16 mm; el par de tornillos de una corredera a los agujeros siguientes
del riel (32 mm); una bisagra, cazoleta y base juntas, un paso de la grilla
Sistema 32. Nunca más cerca de la punta que el herraje más cercano de esa
unión, y el tarugo se mueve antes que la excéntrica. Las hileras de
soportes no se mueven (dos estantes a la misma altura a cada lado de un
divisor comparten un agujero pasante). Una bisagra también se corre si su
base cae donde un estante toca el panel. Si dos herrajes del reparto de una
unión corta caen a menos de 32 mm, se juntan al centro a 32 (o queda uno), y
dos bisagras de una puerta baja se separan lo que pide su cazoleta, no una
sola línea de grilla. Lo que no se puede despejar sigue siendo `FAB-205`
(dos herrajes de un mismo tipo en una misma unión son el reparto del autor:
`fixtures/invalid_cabinet` usa un tarugo con el reparto de la excéntrica y
ahí el tarugo se corre).

## Diagnósticos

Cada hallazgo tiene `code`, `severity` (`INFO`/`WARNING`/`ERROR`/`FATAL`),
`entity`, `location`, `message` y, cuando se puede, `suggestion`. Un `ERROR`
deja generar el plan (`status: errors`); un `FATAL` lo bloquea
(`manufacturingBlocked: true`).

| familia | qué cubre |
|---|---|
| `SPEC-*` | la spec no se puede leer o no tiene sentido (versión, campos, componentes; `002` más de 2000 piezas, que es una cantidad desbocada y no un mueble; `103` un `when` que no es condición; `302` ranura del fondo fuera de la pieza; `304` retiros o luces negativos; `318` más estantes sobre soportes que líneas libres en la grilla; `322/323` lateral o faldón sin medida o sin tapa, faldón con retiro negativo; `324` tirador en un cajón interior que no entra en su retiro; `330` reparto de herrajes con `maxSpacing` ≤ 0; `332` giro que no es de a 90°; `334` frente fijo embutido, con tirador o cierre, o sin canto donde fijarse; `337` puerta de vidrio con bisagra de placa (o al revés), espejo con tirador; `338` ruedas con zócalo; `340` puerta basculante o rebatible con otra en la bahía, con cierre, o cuya zona no llega a la tapa o la base; `339` cajón de carpetas colgantes que no tiene su ancho interior o su alto; `336` bacha que no está en la biblioteca, sin tapa o fondo donde ir, o que no entra en su bahía; `335` corredizas sin riel de la biblioteca, más gruesas que su carril o con divisores que llegan al frente; `501–503` opciones sobre un parámetro inexistente, sobre una fórmula, o con el valor fuera de cota); `SPEC-21x` es la distribución dentro de la carcasa una vez expandidos todos los componentes: dos frentes (o cajones y estantes) sobre la misma altura de una bahía (`210`, error), una bahía vacía (`211`, info), una bahía con frentes que la dejan abierta en un tramo sin estantes (`212`; con estantes es una estantería a la vista a propósito, como una biblioteca con puertas abajo), módulos de una hilera que se pisan o dejan una rendija ≤ 50 mm (`213`), estantes que llegan al plano de una puerta embutida o corrediza (`215`), cajones interiores en el plano de una puerta embutida (`214`, con el retranqueo como arreglo) |
| `PARAM-*` | ciclos o expresiones inválidas en parámetros (un resultado que no es un número finito, como la raíz de un negativo, es error; `and`/`or` cortan en el lado que decide; más de 64 niveles de anidamiento o 2000 símbolos no se aceptan) |
| `LIB-*` | material, canto o herraje desconocido |
| `CON-001` | una restricción declarada no se cumple |
| `JOINT-*` | las piezas no se tocan, el tipo de contacto no está soportado, o el herraje no es del tipo de la unión (`JOINT-104`) |
| `FAB-1xx` | geometría: piezas superpuestas (`101`); una pata que atraviesa una pieza u otra pata (`102`); una bisagra cuya base cae donde otra pieza toca el panel, o cuya cazoleta choca con algo detrás de la puerta, y que no encontró otra línea libre (`103`) |
| `FAB-2xx` | mecanizado: perforación fuera de cara, distancia al borde, profundidad, cruces de perforaciones, ranura, mecha inexistente, operación no admitida, ranura que deja poco material (`208`), perforación que cae dentro de una ranura (`209`: el tarugo iría donde corre el fondo), recorte a menos de la distancia mínima del borde o que se lleva una perforación o una ranura (`210`) |
| `FAB-3xx` | material/máquina: no sale de la placa (con el mismo margen que usa el nesting: una pieza que sólo entra en la placa pelada quedaría afuera de todo acomodo), excede el área de trabajo, espesor incompatible con el herraje |
| `DESIGN-1xx` | lo que un carpintero diría antes de cortar; nunca bloquea. `101` luz de estantes, tapa y base mayor que `maxSpan` de la placa (pandeo; la tapa y la base miden la bahía más ancha, no la carcasa); `102` puerta de más de 600 o menos de 200 mm; `103` luz entre frentes menor a 1,5 mm; `104` frentes de cajón de menos de 100 mm o caja sin altura para la corredera; `105` cajón de más de 900 mm; `106` menos de 150 mm libres entre estantes; `107` carcasa sin fondo; `108` medidas que no parecen milímetros o profundidad de más de 1000; `109` sin canto (info a nivel mueble, aviso en frentes con `edges: none`); `110` manija pasada la mitad de la puerta, del lado de la bisagra; `111` carga: puerta más pesada que lo que aguantan sus bisagras, o cajón cuya caja más 10 kg de contenido supera la corredera (`maxLoadKg` del herraje; el peso sale de la densidad del material); `113` (info) medida escrita como número donde va un parámetro; `114` barral con menos de 900 mm libres debajo; `115` tapa de trabajo de más de 2400 o con más de 1200 de luz entre apoyos; `117` tapa sobre laterales sueltos sin faldón, o lateral que no sostiene nada; `116` cierres: push-open con cierre suave (lo anula) o con tirador (info), o dos puertas por bahía sin tapa ni base al borde de la zona donde apoyar el cierre; `118` una puerta abierta a 95° choca con un cajón abierto (sacado lo que da su corredera) o con otra puerta abierta, con el otro lado de bisagra como arreglo cuando la puerta está sola en su bahía, o con un frente que no se mueve (un frente fijo, o uno cerrado de la hilera de al lado en una L); vale en cualquier giro (los cajones interiores detrás de una puerta no: van sobre distanciadores; queda sólo si la biblioteca no tiene uno bastante grueso); `119` una puerta o un frente de cajón a menos de 10 mm del piso en una carcasa apoyada sin patas: roza al abrir (el arreglo son patas con zócalo retirado; una carcasa colgada no se mira) |
| `CAM-2xx` | sin herramienta para una ranura o el contorno; perforación de canto sin taladro horizontal |
| `STAGE-*` | aviso de algo que la etapa actual no cubre (ninguno activo hoy) |

**Un `FATAL` bloquea de verdad.** El paquete de un plan bloqueado trae sólo
`BLOQUEADO.txt` (los hallazgos fatales), el informe, `plan.json` y el
manifiesto: nada de DXF, programas ni despiece; el CLI sale con 1, el
servidor no emite la orden (422) y la UI deshabilita la descarga. Un `ERROR`
deja exportar y avisa; la UI pide confirmación antes de emitir una orden con
errores.

**Arreglos con un click.** Cuando el motor sabe qué cambio resuelve un
hallazgo, el hallazgo trae `fix` (`{ label, component, field, value }`: un
campo de un componente —o del mueble— puesto a un valor; `null` lo quita).
`rewood_core::spec::apply_fix` lo aplica sobre el JSON de la spec y la UI lo
muestra como botón ("Dividir en 2 bahías", "Agregar fondo HDF 3 mm", "Usar
corredera 350", "Pegar 'm2' a 'm1'"). No es un solver: se aplica uno, se
recompila y se leen los hallazgos nuevos (partir en bahías puede destapar que
dos estantes enfrentados se cruzan en el divisor).

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
- Las opciones se barren de a una desde los valores de cada variante (más
  las que enciende cada elección): dos extremos combinados pueden dar un
  hallazgo que ninguno da solo. El motor lo dice igual; el configurador no
  lo evita.
- El fondo de un placard es una sola placa de HDF: más de ~1850 de ancho no
  sale de una placa y por eso `wardrobe_modules` topa ahí. Un fondo por
  módulo (con ranura en los divisores) es otro componente.
- La tapa de una cocina en L (en escuadra) no se modela: es una placa por
  hilera, y la mesada de piedra se encarga aparte.
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
