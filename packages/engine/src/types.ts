/**
 * TypeScript view of the engine's JSON contract. The source of truth is the
 * serde model in `crates/rewood-core` (`spec.rs`, `model.rs`, `plan.rs`);
 * keep this file in step with it. Every length is millimetres.
 */

// ---------------------------------------------------------------------------
// Input: FurnitureSpec
// ---------------------------------------------------------------------------

/** A literal or an expression over other parameters (`"width - 2 * thickness"`). */
export type NumOrExpr = number | boolean | string;

export type Severity = 'INFO' | 'WARNING' | 'ERROR' | 'FATAL';

export interface PlacementRule {
  endOffset: number;
  maxSpacing: number;
  /** `[[maxLength, count], ...]` ascending; first row not exceeded wins. */
  countByLength?: [number, number][];
  /** Explicit positions from the joint start; overrides the rules above. */
  fixed?: number[];
  /** Fixed pitch from `endOffset` (a System 32 row); overrides `maxSpacing`. */
  pitch?: number;
}

export interface JointSpec {
  hardware: string[];
  /** Overrides each hardware's own distribution rule for this component's joints. */
  placement?: PlacementRule;
}

export interface HandleSpec {
  hardware: string[];
  /** Doors: distance from the opening edge to the handle centre line (default 40). */
  fromEdge?: NumOrExpr;
  /** From the panel bottom to the handle centre; omitted = the middle. */
  position?: NumOrExpr;
}

/** A height range inside a carcass, mm from its bottom. */
export interface ZoneSpec {
  from: NumOrExpr;
  to: NumOrExpr;
}

/** Which edges get the default edge band. `default` = the generator's choice. */
export type EdgeBanding = 'default' | 'none' | 'front' | 'all';

export interface GrooveSpec {
  inset?: NumOrExpr;
  depth?: NumOrExpr;
  clearance?: NumOrExpr;
}

export interface BackSpec {
  material: string;
  groove?: GrooveSpec;
}

export interface CarcassSpec {
  type: 'carcass';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  width?: NumOrExpr;
  height?: NumOrExpr;
  depth?: NumOrExpr;
  material?: string;
  joint: JointSpec;
  back?: BackSpec;
  /** Number of bays; `bays - 1` dividers split the inner width evenly. */
  bays?: NumOrExpr;
  /** Explicit inner width per bay; one entry may be "auto". Overrides `bays`. */
  bayWidths?: NumOrExpr[];
  /** Dividers' front edge back from the carcass front (room for a sliding door track). */
  dividerSetback?: NumOrExpr;
  edges?: EdgeBanding;
  /** Legs under the bottom panel, optionally with a plinth. */
  legs?: LegsSpec;
  /** Where it stands; `rotation` 0/90/180/270 degrees counter-clockwise from above, about that point (270 = front looking at +X). */
  origin?: { x?: NumOrExpr; y?: NumOrExpr; z?: NumOrExpr; rotation?: NumOrExpr };
  /** Wall-hung: a hanger on each side's inner face, top back corner. */
  hanging?: { hardware?: string[] };
}

export interface LegsSpec {
  /** Default `["leg_adjustable_100"]`. */
  hardware?: string[];
  /** Leg centre from the carcass outer edges (default 50). */
  inset?: NumOrExpr;
  /** Largest distance between legs along the width (default 600). */
  maxSpacing?: NumOrExpr;
  plinth?: PlinthSpec;
}

export interface PlinthSpec {
  material?: string;
  /** Plinth front back from the carcass front (default 40). */
  setback?: NumOrExpr;
  /** Default `["plinth_clip"]`. */
  clips?: string[];
  edges?: EdgeBanding;
}

export interface ShelvesSpec {
  type: 'shelves';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  carcass?: string;
  /** 1-based bay; omitted = every bay. */
  bay?: NumOrExpr;
  /** Last bay of a range from `bay`: every bay in it gets its own set (a stack two or three modules wide). */
  lastBay?: NumOrExpr;
  zone?: ZoneSpec;
  /** Shelves spread evenly over the zone. Exclusive with `positions`. */
  count?: NumOrExpr;
  /**
   * Underside height of each shelf from the carcass bottom: fixed shelves
   * (horizontal dividers), full inner depth, `setback` default 0.
   */
  positions?: NumOrExpr[];
  /** Default 20, or 0 with `positions`. */
  setback?: NumOrExpr;
  material?: string;
  /** `joint` (default): butted with `joint` hardware. `pins`: on System 32 rows, movable. */
  support?: 'joint' | 'pins';
  /** Required for `support: joint`. */
  joint?: JointSpec;
  /** For `support: pins`: the row pattern and the loose pins; library defaults. */
  pins?: {
    row?: string[];
    hardware?: string[];
    /** Travel each shelf keeps, mm up and down; 0 (default) drills only the holes it rests on. */
    adjust?: NumOrExpr;
  };
  edges?: EdgeBanding;
}

export interface WorktopSpec {
  type: 'worktop';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  /** Carcass and panel ids it rests on; empty = all of them. */
  carcasses?: string[];
  overhang?: { front?: NumOrExpr; back?: NumOrExpr; sides?: NumOrExpr };
  material?: string;
  /** Screws through each carcass top; default `screw_4x30_face`, 80 mm in. */
  fixing?: JointSpec;
  edges?: EdgeBanding;
}

export interface DoorsSpec {
  type: 'doors';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  /** Consecutive bays one door set covers, from `bay`; default 1. */
  span?: NumOrExpr;
  /** `inset`: inside the opening, with an inset hinge. Default overlay. */
  mount?: FrontMount;
  carcass?: string;
  bay?: NumOrExpr;
  /** Last bay of a range from `bay`: every bay in it gets its own set (a stack two or three modules wide). */
  lastBay?: NumOrExpr;
  zone?: ZoneSpec;
  count: NumOrExpr;
  gap?: NumOrExpr;
  material?: string;
  /** Hinges per door; `null` = none. Defaults to `hinge_35_overlay`. Names the family: the engine picks the arm (overlay / half overlay on a divider / inset). */
  hinge?: JointSpec | null;
  /** One door per bay: the panel it hangs on. `auto` (default) = away from the furniture's middle. */
  hingeSide?: 'auto' | 'left' | 'right';
  /** `true` swaps the damped hinge variant in, `false` the plain one; omitted keeps `hinge`. */
  softClose?: boolean;
  /** A catch on the panel opposite the hinge (or under the top for a pair of doors) with its plate on the door. */
  catch?: CatchSpec;
  handle?: HandleSpec;
  /** A fixed front (panel ciego): dowelled to the panels' front edges, no hinge, catch or handle. */
  fixed?: boolean;
  /** Fasteners of a fixed front; default dowels. */
  fixing?: JointSpec;
  /** `up`: lift-up flap hung on the top (gas struts); `down`: drop-down flap hung on the bottom (stays). One per bay. */
  opening?: 'side' | 'up' | 'down';
  /** What holds a flap open, one per side; default `lift_stay` / `flap_stay`. */
  stays?: string[];
  /** A mirror (or glass) glued over each door's front, `inset` in from its edges (default mirror_4, 3 mm). */
  facing?: { material?: string; inset?: NumOrExpr; adhesive?: string[] };
  edges?: EdgeBanding;
}

export interface CatchSpec {
  /** `kind: catch` hardware; default `magnetic_catch`. */
  hardware?: string[];
}

export interface DrawersSpec {
  type: 'drawers';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  /** `inset` = inner drawer (behind a door). Default overlay. */
  mount?: FrontMount;
  /** Inner drawers: fronts set back from the carcass front. */
  setback?: NumOrExpr;
  carcass?: string;
  bay?: NumOrExpr;
  /** Last bay of a range from `bay`: every bay in it gets its own set (a stack two or three modules wide). */
  lastBay?: NumOrExpr;
  zone?: ZoneSpec;
  count: NumOrExpr;
  frontHeight?: NumOrExpr;
  gap?: NumOrExpr;
  boxHeight?: NumOrExpr;
  material?: string;
  boxMaterial?: string;
  bottomMaterial?: string;
  bottomGroove?: GrooveSpec;
  /** Box corner fasteners. */
  joint: JointSpec;
  /** One slide hardware id; its `slide` block sets the box depth. */
  slide: JointSpec;
  /** `true` swaps the damped slide of the same length and style in, `false` the plain one; omitted keeps `slide`. */
  softClose?: boolean;
  /** Screws fixing the front to the box front; `null` = not modelled. Defaults to `screw_4x30_face`. */
  frontFixing?: JointSpec | null;
  handle?: HandleSpec;
  /** Rails for hanging files along the top of the box sides (default `file_rails_legal`). */
  files?: { hardware?: string[]; screws?: string[] };
  edges?: EdgeBanding;
}

/** A free-standing panel holding a worktop up (a desk end without a pedestal). */
export interface PanelSpec {
  type: 'panel';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  /** X of the outer face; default 0. */
  x?: NumOrExpr;
  /** Where the inner face looks: `right` = the panel at [x, x + t] (default), `left` = [x - t, x]. */
  facing?: 'right' | 'left';
  y?: NumOrExpr;
  z?: NumOrExpr;
  /** Default "depth". */
  depth?: NumOrExpr;
  /** Default "height". */
  height?: NumOrExpr;
  material?: string;
  /** Into the worktop; default minifix + dowel. */
  joint?: JointSpec;
  edges?: EdgeBanding;
}

/** A modesty panel in every gap between the supports of a worktop, under it, near the back. */
export interface ModestySpec {
  type: 'modesty';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  /** The worktop; omitted = the only one. */
  worktop?: string;
  /** Default 300. */
  height?: NumOrExpr;
  /** Back face from the back of the supports; default 20. */
  inset?: NumOrExpr;
  material?: string;
  /** Into the supports at both ends; default minifix + dowel. */
  joint?: JointSpec;
  edges?: EdgeBanding;
}

export interface SlidingDoorsSpec {
  type: 'sliding_doors';
  id: string;
  when?: NumOrExpr;
  carcass?: string;
  /** 2 to twice the track's lanes. */
  count: NumOrExpr;
  /** Where neighbouring doors meet; default 30. */
  overlap?: NumOrExpr;
  /** To the sides; default 2. */
  gap?: NumOrExpr;
  material?: string;
  /** `kind: sliding_track` (by the metre); default `sliding_track_2`. */
  track?: string[];
  screws?: string[];
  rollers?: string[];
  guides?: string[];
  edges?: EdgeBanding;
}

export interface SinkSpec {
  type: 'sink';
  id: string;
  when?: NumOrExpr;
  carcass?: string;
  bay?: NumOrExpr;
  /** `kind: sink` from the library. */
  hardware: string[];
  /** Basin centre back from the carcass front; default mid depth. */
  fromFront?: NumOrExpr;
  /** Sideways from the bay's middle. */
  offset?: NumOrExpr;
  /** An opening in the back for the pipes (`kind: passage`, default `pipe_passage`). */
  passage?: { hardware?: string[]; height?: NumOrExpr };
}

export type ComponentSpec =
  | CarcassSpec
  | ShelvesSpec
  | DoorsSpec
  | DrawersSpec
  | RailSpec
  | WorktopSpec
  | PanelSpec
  | ModestySpec
  | SlidingDoorsSpec
  | SinkSpec;

/** A choice offered on a template, bound to a literal parameter. */
export interface OptionSpec {
  param: string;
  label: string;
  /** Heading the UI groups it under. */
  group?: string;
  help?: string;
  min?: NumOrExpr;
  max?: NumOrExpr;
  step?: number;
  unit?: string;
  /** Pick one of these values instead of a number. */
  choices?: { value: number; label: string }[];
  /** `false` = does not apply right now (shown dimmed). */
  when?: NumOrExpr;
}

/** An option as the engine resolved it: bounds evaluated, current value. */
export interface PlanOption {
  param: string;
  label: string;
  group?: string;
  help?: string;
  kind: 'number' | 'toggle' | 'choice';
  value: number | boolean;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  choices?: { value: number; label: string }[];
  active: boolean;
}

export interface ConstraintSpec {
  id: string;
  expr: string;
  /** `false` = the rule does not apply. */
  when?: NumOrExpr;
  severity?: Severity;
  message?: string;
}

export type GrainKind = 'directional' | 'none';

export interface Material {
  /** Supplier id (`libraries.suppliers`); empty = none. */
  supplier?: string;
  /** Longest unsupported span (mm) a horizontal panel should bridge; default 50 × thickness. */
  maxSpan?: number;
  /** Purchase price of one sheet, in the profile's currency; 0 = unknown. */
  pricePerSheet?: number;
  /** Cut to size by its supplier (glass, mirror): no nesting, banding or CNC; priced by m². */
  outsourced?: boolean;
  pricePerM2?: number;
  id: string;
  name: string;
  nominalThickness: number;
  actualThickness: number;
  sheetLength: number;
  sheetWidth: number;
  grain: GrainKind;
  density: number;
  wasteFactor?: number;
  /** Decor a part of this material gets when the spec names none. */
  defaultDecor?: string;
  /** Core as distributors' order sheets write it: "AGL", "MDF", "TERCI". */
  base?: string;
  /** How order sheets list a board sold in a single finish. */
  listing?: Listing;
}

/** Maker, line and colour as a distributor's order sheet lists them. */
export interface Listing {
  brand: string;
  line: string;
  colour: string;
}

/** A design a sheet is sold in (Faplac "Blanco Nature"): colour, not geometry. */
export interface Decor {
  id: string;
  name: string;
  brand: string;
  line?: string;
  /** The maker's design code. */
  code?: string;
  /** A wood print: the supplier keeps it along the part's length. */
  grain?: boolean;
  /** Approximate colour for the 3D view, `#rrggbb`. */
  hex: string;
  /** Material ids it is sold in. */
  materials: string[];
  /** Sheet size when the design comes in another one. */
  sheetLength?: number;
  sheetWidth?: number;
  /** How order sheets list it, when not brand / line / name in capitals. */
  listing?: Listing;
}

export interface EdgeMaterial {
  /** Supplier id (`libraries.suppliers`); empty = none. */
  supplier?: string;
  /** Per metre; 0 = unknown. */
  pricePerMetre?: number;
  id: string;
  name: string;
  thickness: number;
}

export type JointSide = 'edge_part' | 'face_part';
export type HoleLocation = 'contact_face' | 'edge' | 'face_offset' | 'face_inset';

export interface HoleSpec {
  label: string;
  side: JointSide;
  location: HoleLocation;
  offsetFromEdge?: number;
  offsetAlong?: number;
  /** Fixtures with a 2D screw pattern (a leg's base plate). */
  offsetAcross?: number;
  diameter: number;
  /** `null` = through hole. */
  depth?: number | null;
  countersink?: { diameter: number; depth: number };
}

export interface HardwareDef {
  /** Supplier id (`libraries.suppliers`); empty = none. */
  supplier?: string;
  /** What one unit carries, kg (hinge: its share of the door; slide pair: the loaded drawer). */
  maxLoadKg?: number;
  /** Hinges: which door mount the arm is made for, damper, opening angle (default 110). */
  hinge?: { mount: FrontMount | 'half_overlay'; softClose?: boolean; opening?: number; glass?: boolean; maxDoor?: [number, number] };
  /** Catches: the plate on the door (`kind: strike`); `push` = a push latch. */
  catch?: { strike?: string; push?: boolean };
  id: string;
  name: string;
  kind: string;
  compatibleThickness: [number, number];
  placement: PlacementRule;
  /** Slides only. `style` `ball` (default) or `roller`. */
  slide?: { length: number; sideClearance: number; axisFromBoxBottom: number; softClose?: boolean; style?: string };
  /** Legs only. */
  leg?: { height: number; baseDiameter: number; caster?: boolean };
  /** Hanging-file rails only: the inner box width they span and the height a folder needs. */
  files?: { minInner: number; maxInner: number; minHeight: number };
  /** Sliding door tracks only: lanes, their pitch and the room the track takes. */
  sliding?: { lanes: number; lanePitch: number; frontInset: number; depth: number; bottomClearance: number; topClearance: number; maxWidth?: number; widthDeduction?: number; doors?: number };
  /** Sinks only: how far the bowl hangs under the top. */
  sink?: { below: number };
  /** Openings cut through the part the fixture sits on (a sink, a drain, a pipe passage). */
  cutouts?: { label: string; width: number; height: number; radius?: number; offsetAlong?: number; offsetAcross?: number }[];
  /** Spacers only: how far it moves a slide (and an inner drawer) off the panel, mm. */
  spacer?: { thickness: number };
  holes: HoleSpec[];
  /** `through`: only for a fastener crossing that much panel, [from, to) mm (a screw long enough for it). */
  bomItems?: { name: string; quantity: number; unitPrice?: number; through?: [number, number] }[];
}

export type OperationKind =
  | 'CUT'
  | 'DRILL'
  | 'BORE'
  | 'POCKET'
  | 'GROOVE'
  | 'DADO'
  | 'RABBET'
  | 'COUNTERSINK'
  | 'CONTOUR'
  | 'CHAMFER'
  | 'ROUND'
  | 'EDGE_BAND'
  | 'CUTOUT';

export type ToolKind = 'drill' | 'end_mill' | 'compression_bit';

export interface ToolDef {
  id: string;
  number: number;
  kind: ToolKind;
  diameter: number;
  maxDepthPerPass: number;
  feedXy: number;
  feedZ: number;
  rpm: number;
}

export interface ManufacturingProfile {
  id: string;
  name: string;
  version: string;
  maxPartSize: [number, number];
  minPartSize: [number, number];
  minHoleDiameter: number;
  tools: ToolDef[];
  horizontalDrilling: boolean;
  postProcessor: string;
  minEdgeDistance: number;
  minHoleSpacing: number;
  minRemainingThickness: number;
  allowedOperations: OperationKind[];
  tolerances: { length: number; holePosition: number; holeDiameter: number };
  nesting?: { kerf: number; margin: number; mode?: 'max_rects' | 'guillotine' };
  /** Currency label of every price in the libraries. */
  currency?: string;
  /** `banded_panels` (default): saw, bander, then CNC drilling on the finished part. `nested_router`: the router works the raw panel (cut size, outline, edge drilling in setup E after banding). */
  workflow?: 'banded_panels' | 'nested_router';
  /** Travel, table clearance and kinematics for the NC simulation. */
  machine?: {
    travelZ: number;
    maxCutBelowBlank: number;
    rapidFeed: number;
    toolChangeSeconds: number;
    /** Machine cost per hour; 0 = unknown. */
    hourlyRate?: number;
    /** Clamps, stops and pods the tool must never enter (blank coordinates; `top` relative to the blank surface). */
    fixtures?: { name: string; x: number; y: number; length: number; width: number; top: number }[];
  };
}

/** The engine's default libraries, as `libraries()` returns them. */
export interface LibrariesSnapshot {
  materials: {
    version: string;
    materials: Record<string, Material>;
    edgeMaterials: Record<string, EdgeMaterial>;
    decors: Record<string, Decor>;
  };
  hardware: { version: string; items: Record<string, HardwareDef> };
  profile: ManufacturingProfile;
  suppliers: { version: string; suppliers: Record<string, Supplier> };
}

export type DeepPartial<T> = {
  [K in keyof T]?: NonNullable<T[K]> extends object ? DeepPartial<NonNullable<T[K]>> : T[K];
};

export interface LibraryOverrides {
  /** Patches by id (an existing id changes only the keys given; a new id must be complete). */
  materials?: (DeepPartial<Material> & { id: string })[];
  edgeMaterials?: (DeepPartial<EdgeMaterial> & { id: string })[];
  hardware?: (DeepPartial<HardwareDef> & { id: string })[];
  suppliers?: (DeepPartial<Supplier> & { id: string })[];
  /** A patch over the default profile: only the keys given change. */
  profile?: DeepPartial<ManufacturingProfile>;
}

export interface FurnitureSpec {
  schemaVersion: '1.0';
  id: string;
  name: string;
  version?: string;
  parameters?: Record<string, NumOrExpr>;
  material: string;
  edgeMaterial?: string;
  /** Colour of the furniture (a decor id); absent = the material's default. */
  decor?: string;
  /** Colour of doors and drawer fronts; absent = the same as `decor`. */
  frontDecor?: string;
  components: ComponentSpec[];
  constraints?: ConstraintSpec[];
  libraries?: LibraryOverrides;
  /** What a person picks on this template; each option drives one parameter. */
  options?: OptionSpec[];
}

// ---------------------------------------------------------------------------
// Output: ManufacturingPlan
// ---------------------------------------------------------------------------

export type Vec3 = [number, number, number];
export type Axis = 'pos_x' | 'neg_x' | 'pos_y' | 'neg_y' | 'pos_z' | 'neg_z';
/** Faces in the part's own frame: `front` is local +Z, the preferred machining face. */
export type Face = 'front' | 'back' | 'left' | 'right' | 'bottom' | 'top';
export type Grain = 'length' | 'width' | 'none';
export type PlanStatus = 'ok' | 'warnings' | 'errors' | 'blocked';

export interface Dims {
  length: number;
  width: number;
  thickness: number;
}

export interface Placement {
  origin: Vec3;
  x: Axis;
  y: Axis;
}

export interface Aabb {
  min: Vec3;
  max: Vec3;
}

export interface OpSource {
  joint: string;
  hardware: string;
  fastener: number;
  label: string;
}

interface OperationBase {
  id: string;
  face: Face;
  source?: OpSource;
}

export interface DrillOperation extends OperationBase {
  type: 'DRILL';
  u: number;
  v: number;
  diameter: number;
  depth: number | null;
  through: boolean;
  countersink?: { diameter: number; depth: number };
}

export interface GrooveOperation extends OperationBase {
  type: 'GROOVE';
  from: [number, number];
  to: [number, number];
  width: number;
  depth: number;
}

export interface EdgeBandOperation extends OperationBase {
  type: 'EDGE_BAND';
  material: string;
  thickness: number;
  length: number;
}

/** A through opening centred on (u, v): `width` along u, `height` along v, corners rounded to `radius`. */
export interface CutoutOperation extends OperationBase {
  type: 'CUTOUT';
  u: number;
  v: number;
  width: number;
  height: number;
  radius: number;
}

export type Operation = DrillOperation | GrooveOperation | EdgeBandOperation | CutoutOperation;

export interface Part {
  /** Cut to size by the material's supplier (glass, mirror): no nesting, banding or CNC. */
  outsourced?: boolean;
  id: string;
  name: string;
  component: string;
  role: string;
  material: string;
  /** Decor id, for materials sold in several. */
  decor?: string;
  dims: Dims;
  cut: { length: number; width: number };
  grain: Grain;
  edges: Partial<Record<Face, string>>;
  placement: Placement;
  aabb: Aabb;
  operations: Operation[];
  overlapExempt?: string[];
}

export interface Fastener {
  hardware: string;
  index: number;
  position: Vec3;
}

export interface Joint {
  id: string;
  kind: 'butt' | 'hinge' | 'slide' | 'face_to_face' | 'handle' | 'fixture' | 'row';
  component: string;
  edgePart: string;
  facePart: string;
  hardware: string[];
  contact: Aabb;
  axis: Axis;
  length: number;
  fasteners: Fastener[];
}

export type FrontMount = 'overlay' | 'inset';

export interface RailSpec {
  type: 'rail';
  id: string;
  /** Boolean expression over the parameters; `false` leaves the component out (and what refers to it). */
  when?: NumOrExpr;
  carcass?: string;
  bay?: NumOrExpr;
  /** Last bay of a range from `bay`: every bay in it gets its own set (a stack two or three modules wide). */
  lastBay?: NumOrExpr;
  zone?: ZoneSpec;
  /** Rail centre below the top of its zone; default 60. */
  fromTop?: NumOrExpr;
  /** The bar (kind `rail`); default `["rail_oval_30"]`. */
  hardware?: string[];
  /** End supports; default `["rail_support_oval"]`. */
  supports?: string[];
}

export interface Diagnostic {
  code: string;
  severity: Severity;
  entity?: string;
  location?: string;
  message: string;
  suggestion?: string;
  /** A one-click change to the spec that resolves the finding. */
  fix?: Fix;
}

export interface Fix {
  /** Button text. */
  label: string;
  /** Component id; absent = the furniture root. */
  component?: string;
  /** Dotted path inside the component: `bays`, `origin.x`, `handle.fromEdge`. */
  field: string;
  /** New value; `null` removes the field. */
  value: unknown;
}

export interface PartListRow {
  partIds: string[];
  name: string;
  quantity: number;
  material: string;
  decor?: string;
  cutLength: number;
  cutWidth: number;
  finishedLength: number;
  finishedWidth: number;
  thickness: number;
  grain: Grain;
  edges: string;
  operations: number;
  weightKg: number;
}

export interface Bom {
  sheets: {
    material: string;
    decor?: string;
    /** The material's name, with the decor's. */
    name: string;
    parts: number;
    netAreaM2: number;
    sheetLength: number;
    sheetWidth: number;
    /** Sheets the nesting used (area estimate only if nothing nested). */
    estimatedSheets: number;
    yieldRatio: number;
    /** Sheets × price per sheet; 0 when the material has no price. */
    cost: number;
  }[];
  hardware: {
    hardware: string;
    name: string;
    quantity: number;
    items: { name: string; quantity: number; cost: number }[];
    cost: number;
  }[];
  consumables: { material: string; decor?: string; name: string; lengthM: number; cost: number }[];
  totalWeightKg: number;
  /** Costs come from prices in the libraries; 0 = no price, never free. */
  materialsCost: number;
  machiningCost: number;
  totalCost: number;
  currency?: string;
  /** Materials or hardware used without a price: the total is a floor. */
  unpriced?: string[];
}

export interface PackageFile {
  /** Relative path inside the manufacturing package (`parts/P001.dxf`). */
  path: string;
  contents: string;
}

export interface NestedPart {
  part: string;
  /** Lower-left corner: x along the sheet length, y along its width. */
  x: number;
  y: number;
  /** Footprint after rotation. */
  length: number;
  width: number;
  rotated: boolean;
}

/** One saw cut in sheet coordinates (guillotine mode), in order. */
export interface SawCut {
  order: number;
  /** 1 rips between levels, 2 cross cuts, 3 trims. */
  stage: 1 | 2 | 3;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface SheetLayout {
  material: string;
  /** Parts of another decor never share the sheet. */
  decor?: string;
  /** 1-based, per material and decor. */
  index: number;
  sheetLength: number;
  sheetWidth: number;
  parts: NestedPart[];
  usedAreaM2: number;
  wasteRatio: number;
  /** Guillotine mode only. */
  cuts?: SawCut[];
}

export interface ManufacturingPlan {
  schemaVersion: string;
  furniture: { id: string; name: string; version: string };
  versions: { schema: string; engine: string; materials: string; hardware: string; profile: string; suppliers?: string };
  /** The full profile the plan was compiled against (snapshot). */
  profile: ManufacturingProfile;
  status: PlanStatus;
  manufacturingBlocked: boolean;
  parameters: Record<string, number | boolean>;
  /** The spec's options, resolved; absent when it has none. */
  options?: PlanOption[];
  /** Components left out by their `when`. */
  inactive?: string[];
  derived: Record<string, number>;
  parts: Part[];
  joints: Joint[];
  partList: PartListRow[];
  bom: Bom;
  nesting: SheetLayout[];
  /** NC programs as the simulation ran them (§28). */
  machining: Machining;
  /** The BOM split by supplier (§52). */
  purchasing: PurchaseOrder[];
  /** The library entries the parts use, so a frozen plan still names them. */
  catalog?: {
    materials: Record<string, Material>;
    edgeMaterials: Record<string, EdgeMaterial>;
    decors?: Record<string, Decor>;
  };
  diagnostics: { items: Diagnostic[] };
}

export interface PurchaseLine {
  kind: 'sheet' | 'edge_band' | 'hardware';
  /** Library id(s) the line comes from (merged hardware items list several). */
  id: string;
  name: string;
  quantity: number;
  unit: string;
  unitPrice: number;
  cost: number;
}

export interface PurchaseOrder {
  /** Supplier id; empty = items without a supplier. */
  supplier: string;
  name: string;
  leadDays: number;
  lines: PurchaseLine[];
  cost: number;
}

export interface Supplier {
  id: string;
  name: string;
  leadDays?: number;
  contact?: string;
  notes?: string;
}

export interface ProgramSummary {
  part: string;
  /** `A` (front up) or `B` (back up). */
  setup: string;
  operations: number;
  toolChanges: number;
  cutMm: number;
  rapidMm: number;
  /** Machining time on the profile's feeds; no loading, no edge banding. */
  seconds: number;
}

export interface Machining {
  postProcessor: string;
  totalSeconds: number;
  programs: ProgramSummary[];
}
