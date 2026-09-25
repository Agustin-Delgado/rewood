/**
 * The realistic photo: the furniture exactly as the viewer shows it (same
 * parts, same open doors and drawers, same colours and hardware), rebuilt
 * with physical materials in a plain room and path traced until it reads
 * as a photograph. Nothing here decides geometry: every box comes from a
 * part mesh of the live scene, every handle from its hardware instance.
 *
 * Textures and the environment are CC0 (Poly Haven), see
 * `static/render/LEEME.txt`.
 */
import * as THREE from 'three';
import { HDRLoader } from 'three/examples/jsm/loaders/HDRLoader.js';
import { WebGLPathTracer } from 'three-gpu-pathtracer';
import type { Decor, ManufacturingPlan, Material, Part } from '@rewood/engine/browser';
import { AXIS_VEC } from './geometry';

/** Where a wood print repeats, mm: about a real panel's figure. */
const WOOD_TILE = 900;
const FLOOR_TILE = 1400;

export type PhotoSource = { scene: THREE.Scene; camera: THREE.Camera };

type Assets = { env: THREE.DataTexture; wood: THREE.Texture; floor: THREE.Texture };
let assets: Promise<Assets> | null = null;

/** The grey-scale grain every wood decor is tinted from: mean about 0.8,
 * so the decor's colour comes through at close to its own value. */
async function loadWood(url: string): Promise<THREE.Texture> {
	const img = await new THREE.ImageLoader().loadAsync(url);
	const c = document.createElement('canvas');
	c.width = img.width;
	c.height = img.height;
	const g = c.getContext('2d')!;
	g.drawImage(img, 0, 0);
	const data = g.getImageData(0, 0, c.width, c.height);
	const px = data.data;
	let sum = 0;
	for (let i = 0; i < px.length; i += 4) sum += 0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2];
	const mean = sum / (px.length / 4);
	for (let i = 0; i < px.length; i += 4) {
		const l = 0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2];
		const v = Math.min(255, (l / mean) * 205);
		px[i] = px[i + 1] = px[i + 2] = v;
	}
	g.putImageData(data, 0, 0);
	const t = new THREE.CanvasTexture(c);
	t.wrapS = t.wrapT = THREE.RepeatWrapping;
	t.colorSpace = THREE.SRGBColorSpace;
	t.anisotropy = 8;
	return t;
}

function loadAssets(): Promise<Assets> {
	assets ??= (async () => {
		const [env, wood, floor] = await Promise.all([
			new HDRLoader().setDataType(THREE.FloatType).loadAsync('/render/loft.hdr'),
			loadWood('/render/wood.jpg'),
			new THREE.TextureLoader().loadAsync('/render/floor.jpg')
		]);
		env.mapping = THREE.EquirectangularReflectionMapping;
		floor.wrapS = floor.wrapT = THREE.RepeatWrapping;
		floor.colorSpace = THREE.SRGBColorSpace;
		floor.anisotropy = 8;
		return { env, wood, floor };
	})();
	return assets;
}

/** Furniture axis index → three.js axis index (x = X, y = Z, z = Y). */
const THREE_AXIS = [0, 2, 1];

/** A small, stable offset per part, so neighbouring panels do not show the
 * same stretch of grain. */
function grainOffset(id: string): number {
	let h = 0;
	for (const ch of id) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
	return (h % 997) / 997;
}

/**
 * A box of real size with UVs in millimetres over `tile`, the texture's
 * grain (its V) laid along `grainAxis` (a three.js axis index, in the
 * box's own frame) wherever that axis lies in the face.
 */
function panelGeometry(size: THREE.Vector3, grainAxis: number, offset: number, tile: number): THREE.BufferGeometry {
	const pos: number[] = [];
	const nor: number[] = [];
	const uv: number[] = [];
	const idx: number[] = [];
	const half = [size.x / 2, size.y / 2, size.z / 2];
	for (let n = 0; n < 3; n++) {
		for (const sign of [1, -1]) {
			const [a, b] = [0, 1, 2].filter((i) => i !== n);
			// V along the grain when the grain lies in this face; else along
			// the longer side.
			const vAxis = grainAxis !== n ? (grainAxis === a || grainAxis === b ? grainAxis : a) : size.getComponent(a) >= size.getComponent(b) ? a : b;
			const uAxis = vAxis === a ? b : a;
			const base = pos.length / 3;
			for (const [cu, cv] of [
				[-1, -1],
				[1, -1],
				[1, 1],
				[-1, 1]
			]) {
				const p = [0, 0, 0];
				p[n] = sign * half[n];
				p[uAxis] = cu * half[uAxis];
				p[vAxis] = cv * half[vAxis];
				pos.push(p[0], p[1], p[2]);
				const q = [0, 0, 0];
				q[n] = sign;
				nor.push(q[0], q[1], q[2]);
				uv.push((p[uAxis] + half[uAxis]) / tile + offset, (p[vAxis] + half[vAxis]) / tile + offset * 0.37);
			}
			// Wind the quad to face outwards.
			const e1 = new THREE.Vector3(), e2 = new THREE.Vector3();
			e1.setComponent(uAxis, 1);
			e2.setComponent(vAxis, 1);
			const faces = e1.clone().cross(e2).getComponent(n) * sign > 0;
			if (faces) idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
			else idx.push(base, base + 2, base + 1, base, base + 3, base + 2);
		}
	}
	const g = new THREE.BufferGeometry();
	g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
	g.setAttribute('normal', new THREE.Float32BufferAttribute(nor, 3));
	g.setAttribute('uv', new THREE.Float32BufferAttribute(uv, 2));
	g.setIndex(idx);
	return g;
}

type Catalog = {
	materials: Record<string, Material>;
	decors: Record<string, Decor>;
};

/** Physical materials, one per look, shared between the parts that have it. */
function materialFor(part: Part, cat: Catalog, a: Assets, cache: Map<string, THREE.Material>): THREE.Material {
	const decor = part.decor ? cat.decors[part.decor] : undefined;
	const key = part.material + '|' + (part.decor ?? '');
	const hit = cache.get(key);
	if (hit) return hit;
	let m: THREE.Material;
	if (part.material.startsWith('glass')) {
		m = new THREE.MeshPhysicalMaterial({ color: '#eef6f5', transmission: 1, thickness: 5, roughness: 0.02, ior: 1.5 });
	} else if (part.material.startsWith('mirror')) {
		m = new THREE.MeshPhysicalMaterial({ color: '#ffffff', metalness: 1, roughness: 0.02 });
	} else if (part.material.startsWith('hdf')) {
		// The white fibreboard back: matte.
		m = new THREE.MeshPhysicalMaterial({ color: '#efece6', roughness: 0.85 });
	} else if (decor) {
		// Melamine: a satin, slightly reflective face; wood prints carry
		// their grain.
		m = new THREE.MeshPhysicalMaterial({
			color: decor.hex,
			map: decor.grain ? a.wood : null,
			roughness: decor.grain ? 0.55 : 0.45,
			specularIntensity: 0.6,
			clearcoat: 0.08,
			clearcoatRoughness: 0.4
		});
	} else {
		// Painted MDF and the rest: a plain matte paint.
		m = new THREE.MeshPhysicalMaterial({ color: '#d9d4cb', roughness: 0.7 });
	}
	cache.set(key, m);
	return m;
}

/**
 * The photo scene from the live one: part boxes rebuilt at their real size
 * with physical materials, hardware copied instance by instance as metal,
 * hole and groove marks left out (a photo shows the hardware, not the
 * drilling plan), a floor and a wall behind.
 */
export async function buildPhotoScene(src: PhotoSource, plan: ManufacturingPlan, cat: Catalog) {
	const a = await loadAssets();
	const scene = new THREE.Scene();
	const cache = new Map<string, THREE.Material>();
	const metals = new Map<string, THREE.Material>();
	const box = new THREE.Box3();
	const parts = new Map(plan.parts.map((p) => [p.id, p]));
	src.scene.updateMatrixWorld(true);

	const m = new THREE.Matrix4();
	const pos = new THREE.Vector3(), quat = new THREE.Quaternion(), scl = new THREE.Vector3();
	src.scene.traverse((o) => {
		if (!o.visible) return;
		const partId = o.userData.part as string | undefined;
		if (partId && o instanceof THREE.Mesh) {
			const part = parts.get(partId);
			if (!part) return;
			o.matrixWorld.decompose(pos, quat, scl);
			// The grain runs along the part's length (its local X).
			const grain = THREE_AXIS[AXIS_VEC[part.placement.x].findIndex((c) => c !== 0)];
			const geo = panelGeometry(scl.clone(), grain, grainOffset(part.id), WOOD_TILE);
			const mesh = new THREE.Mesh(geo, materialFor(part, cat, a, cache));
			mesh.position.copy(pos);
			mesh.quaternion.copy(quat);
			scene.add(mesh);
			box.expandByObject(mesh);
			return;
		}
		if (o.userData.tag === 'hardware' && o instanceof THREE.InstancedMesh) {
			const colour = new THREE.Color();
			for (let i = 0; i < o.count; i++) {
				o.getMatrixAt(i, m);
				m.premultiply(o.matrixWorld);
				if (o.instanceColor) o.getColorAt(i, colour);
				else colour.set('#9aa0a6');
				const k = colour.getHexString();
				let mat = metals.get(k);
				if (!mat) {
					// Dark hardware (handles in black) is a satin powder coat;
					// the rest brushed steel.
					const dark = colour.r + colour.g + colour.b < 0.6;
					mat = new THREE.MeshPhysicalMaterial({
						color: colour.clone(),
						metalness: dark ? 0.3 : 0.9,
						roughness: dark ? 0.45 : 0.3
					});
					metals.set(k, mat);
				}
				const mesh = new THREE.Mesh(o.geometry, mat);
				mesh.matrixAutoUpdate = false;
				mesh.matrix.copy(m);
				scene.add(mesh);
			}
			return;
		}
		if (o.userData.tag === 'rail' && o instanceof THREE.Mesh) {
			const mesh = new THREE.Mesh(o.geometry, new THREE.MeshPhysicalMaterial({ color: '#c9ccd0', metalness: 1, roughness: 0.25 }));
			mesh.matrixAutoUpdate = false;
			mesh.matrix.copy(o.matrixWorld);
			scene.add(mesh);
		}
	});

	// The room: a floor under the lowest part, a wall just behind the back.
	const size = box.getSize(new THREE.Vector3());
	const centre = box.getCenter(new THREE.Vector3());
	const span = Math.max(size.x, size.z, size.y) * 6;
	const floorTex = a.floor.clone();
	floorTex.repeat.set(span / FLOOR_TILE, span / FLOOR_TILE);
	floorTex.needsUpdate = true;
	const floor = new THREE.Mesh(
		new THREE.PlaneGeometry(span, span),
		new THREE.MeshPhysicalMaterial({ map: floorTex, roughness: 0.55, clearcoat: 0.3, clearcoatRoughness: 0.25 })
	);
	floor.rotation.x = -Math.PI / 2;
	floor.position.set(centre.x, box.min.y, centre.z);
	scene.add(floor);
	const wall = new THREE.Mesh(
		new THREE.PlaneGeometry(span, span),
		new THREE.MeshPhysicalMaterial({ color: '#d8d0c4', roughness: 0.92 })
	);
	wall.position.set(centre.x, box.min.y + span / 2, box.min.z - 15);
	scene.add(wall);
	// A skirting board where the wall meets the floor.
	const skirting = new THREE.Mesh(
		new THREE.BoxGeometry(span, 70, 12),
		new THREE.MeshPhysicalMaterial({ color: '#f4f1eb', roughness: 0.6 })
	);
	skirting.position.set(centre.x, box.min.y + 35, box.min.z - 9);
	scene.add(skirting);

	// Light: the loft's daylight all round, and a window's soft light from
	// the front left.
	scene.environment = a.env;
	scene.environmentIntensity = 0.9;
	scene.background = new THREE.Color('#e9e5de');
	const window_ = new THREE.RectAreaLight('#fff6ea', 6, size.y * 1.2, size.y * 1.2);
	window_.position.set(box.min.x - size.x * 1.2, box.min.y + size.y * 1.1, box.max.z + size.y * 1.5);
	window_.lookAt(centre);
	scene.add(window_);

	return scene;
}

/**
 * A photographer's camera: from the side the viewer looks at the furniture,
 * a 35 mm-ish lens, as close as it gets with the whole piece in frame and
 * a margin around it. An elevation (no perspective) gives its direction.
 */
export function photoCamera(src: PhotoSource, aspect: number, furniture: THREE.Box3): THREE.PerspectiveCamera {
	const cam = new THREE.PerspectiveCamera(32, aspect, 10, 100000);
	const dir = new THREE.Vector3();
	src.camera.getWorldDirection(dir);
	// Never from below the floor or straight down: a person's eye height.
	dir.y = Math.min(-0.15, Math.max(dir.y, -0.45));
	dir.normalize();
	const target = furniture.getCenter(new THREE.Vector3());
	const radius = furniture.getBoundingSphere(new THREE.Sphere()).radius;
	const vFov = THREE.MathUtils.degToRad(cam.fov);
	const hFov = 2 * Math.atan(Math.tan(vFov / 2) * aspect);
	const distance = (radius * 1.08) / Math.sin(Math.min(vFov, hFov) / 2);
	cam.position.copy(target).addScaledVector(dir, -distance);
	cam.lookAt(target);
	cam.updateProjectionMatrix();
	return cam;
}

/**
 * Path traces `scene` on `canvas`, sample after sample, until `stop` is
 * called or `maxSamples` is reached. `onProgress` gets the sample count.
 */
export function startRender(
	canvas: HTMLCanvasElement,
	scene: THREE.Scene,
	camera: THREE.Camera,
	maxSamples: number,
	onProgress: (samples: number) => void
) {
	const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, preserveDrawingBuffer: true });
	renderer.setPixelRatio(1);
	renderer.setSize(canvas.width, canvas.height, false);
	renderer.toneMapping = THREE.AgXToneMapping;
	renderer.toneMappingExposure = 0.85;
	renderer.outputColorSpace = THREE.SRGBColorSpace;
	const tracer = new WebGLPathTracer(renderer);
	tracer.tiles.set(1, 1);
	tracer.bounces = 4;
	tracer.filterGlossyFactor = 0.5;
	tracer.minSamples = 1;
	tracer.fadeDuration = 0;
	tracer.renderDelay = 0;
	tracer.setScene(scene, camera);
	let frame = 0;
	let running = true;
	const tick = () => {
		if (!running) return;
		tracer.renderSample();
		onProgress(Math.floor(tracer.samples));
		if (tracer.samples >= maxSamples) {
			running = false;
			return;
		}
		frame = requestAnimationFrame(tick);
	};
	frame = requestAnimationFrame(tick);
	return {
		stop() {
			running = false;
			cancelAnimationFrame(frame);
		},
		dispose() {
			running = false;
			cancelAnimationFrame(frame);
			tracer.dispose();
			renderer.dispose();
		}
	};
}
