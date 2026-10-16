import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { NATION_COLORS } from './nations';

/** Kawałki terytorium (na GPU idą tylko te, w których coś się zmieniło). */
const CHUNK = 512;
/** Najgrubsza granica państwa na mapie terenu (kafle). */
const MAX_BORDER = 6;
/** Nowo zajęty kafel: rozbłysk i przejście w kolor państwa (ms). */
const FADE_MS = 300;
/** Jednostka czasu zmiany w kanale A (ms); 255 = kafel „stary” (bez przejścia). */
const TIME_UNIT_MS = 20;
const STABLE = 255;
/** Po tylu ms kafel wraca do „starych” (czas w kanale A zawija się co 255 jednostek). */
const SETTLE_MS = 1000;
/** Mapa polityczna: przyciemnienie kafla na granicy prowincji i na granicy państwa (jak `render/owners.ts`). */
const PROVINCE_EDGE = 0.8;
const NATION_EDGE = 0.5;
/** Mapa terenu: krycie granicy państwa w najgłębszym jej kaflu (przy krawędzi – pełne). */
const BORDER_INNER = 0.35;
/** Kanał B: krawędź na mapie politycznej. */
const EDGE_PROVINCE = 128;
const EDGE_NATION = 255;

const vertex = /* glsl */ `
in vec2 aPosition;
in vec2 aUV;
out vec2 vUV;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

void main() {
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
  vUV = aUV;
}
`;

/**
 * Dane kafla (liczone na CPU): R – państwo, G – siła granicy państwa (mapa terenu), B – krawędź na mapie
 * politycznej (0,5 – prowincji, 1 – państwa), A – chwila zmiany (255 – bez przejścia).
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform sampler2D uPalette;
uniform vec2 uSize;
uniform float uPolitical;
uniform float uFill;
uniform float uBorder;
uniform float uNow;

void main() {
  vec4 d = texture(uData, (floor(vUV * uSize) + 0.5) / uSize);
  float n = floor(d.r * 255.0 + 0.5);
  if (n < 0.5) discard;
  vec3 color = texture(uPalette, vec2((n + 0.5) / 256.0, 0.5)).rgb;
  float a;
  if (uPolitical > 0.5) {
    color *= d.b > 0.75 ? ${NATION_EDGE.toFixed(2)} : (d.b > 0.25 ? ${PROVINCE_EDGE.toFixed(2)} : 1.0);
    a = 1.0;
  } else {
    a = 1.0 - (1.0 - uFill) * (1.0 - uBorder * d.g);
  }
  // Nowo zajęty kafel: jasny rozbłysk przechodzi w kolor państwa.
  float t = floor(d.a * 255.0 + 0.5);
  if (t < 254.5) {
    float k = clamp(mod(uNow - t + 255.0, 255.0) * ${TIME_UNIT_MS.toFixed(1)} / ${FADE_MS.toFixed(1)}, 0.0, 1.0);
    color = mix(vec3(1.0), color, 0.35 + 0.65 * k);
    a = max(a, 0.6 * (1.0 - k));
  }
  if (a <= 0.001) discard;
  finalColor = vec4(color * a, a);
}
`;

interface Chunk {
  x0: number;
  y0: number;
  w: number;
  h: number;
  data: Uint8Array;
  source: BufferImageSource;
  mesh: Mesh<MeshGeometry, Shader>;
  dirty: boolean;
}

export interface TerritoryLook {
  /** Mapa polityczna (pełne kolory z krawędziami) albo mapa terenu (tło i granica). */
  political: boolean;
  fill: number;
  border: number;
  borderWidth: number;
}

/**
 * Terytoria państw na żywo – co turę, bez przemalowania całej mapy: mapa polityczna (kolory, krawędzie
 * prowincji i państw) i mapa terenu (półprzezroczyste tło, granica państwa w głąb). Krawędzie i siłę
 * granicy liczy CPU tylko wokół zmienionych kafli; GPU rysuje kolor z palety. Nowo zajęte kafle
 * rozbłyskują i przechodzą w kolor państwa. Flagi i nazwy maluje dalej worker (`paintTerritories`).
 */
export class TerritoryLayer {
  readonly view = new Container();
  private chunks: Chunk[] = [];
  private width = 0;
  private height = 0;
  private per = 0;
  private province: Uint16Array = new Uint16Array(0);
  /** Państwo na kafel (żywe). */
  private nation: Uint8Array = new Uint8Array(0);
  /** Znacznik przeliczenia kafla w bieżącej partii (bez powtórek). */
  private stamp: Uint32Array = new Uint32Array(0);
  private batch = 0;
  private palette: Texture | null = null;
  private look: TerritoryLook = { political: false, fill: 0.25, border: 0.85, borderWidth: 3 };
  private readonly start = performance.now();
  /** Kafle w trakcie przejścia (kafel, chwila zmiany w ms). */
  private recent: { tile: number; at: number }[] = [];

  setMap(map: MapPayload): void {
    this.clear();
    this.width = map.width;
    this.height = map.height;
    this.per = Math.ceil(map.width / CHUNK);
    this.province = map.province;
    this.nation = new Uint8Array(map.width * map.height);
    this.stamp = new Uint32Array(map.width * map.height);
    this.palette ??= paletteTexture();
    for (let y0 = 0; y0 < map.height; y0 += CHUNK) {
      for (let x0 = 0; x0 < map.width; x0 += CHUNK) {
        this.chunks.push(this.chunk(x0, y0, Math.min(CHUNK, map.width - x0), Math.min(CHUNK, map.height - y0)));
      }
    }
  }

  /** Pełny stan właścicieli; kafle inne niż dotąd przechodzą płynnie (gdy zmiany nie przyszły osobno). */
  setOwners(tiles: Uint8Array | null, animate = true): void {
    if (this.nation.length === 0) return;
    const changed: number[] = [];
    for (let i = 0; i < this.nation.length; i++) {
      const n = tiles ? tiles[i] : 0;
      if (this.nation[i] === n) continue;
      this.nation[i] = n;
      changed.push(i);
    }
    this.refresh(changed, animate);
  }

  /**
   * Przewidywany start (zanim tura z wyborem wróci z serwera): kafle prowincji `province` dostają
   * państwo `nation`, a kafle poprzedniego startu `previous` wracają do niczyich. Prawdziwe zmiany
   * z tury (i pełne `owners`) i tak nadpiszą te kafle.
   */
  preview(province: number, previous: number, nation: number): void {
    if (this.nation.length === 0) return;
    const changed: number[] = [];
    for (let i = 0; i < this.province.length; i++) {
      const p = this.province[i];
      if (p !== province && (p !== previous || previous === 0)) continue;
      const n = p === province ? nation : 0;
      if (this.nation[i] === n) continue;
      this.nation[i] = n;
      changed.push(i);
    }
    this.refresh(changed, true);
  }

  /** Zmiany kafli z tury (`kafel << 8 | państwo`). */
  apply(changes: Uint32Array): void {
    if (this.nation.length === 0) return;
    const changed: number[] = [];
    for (const change of changes) {
      const tile = change >>> 8;
      if (tile >= this.nation.length) continue;
      this.nation[tile] = change & 255;
      changed.push(tile);
    }
    this.refresh(changed, true);
  }

  setLook(look: TerritoryLook): void {
    const recompute = look.borderWidth !== this.look.borderWidth;
    this.look = look;
    if (recompute && this.nation.length > 0) {
      // Inna grubość granicy – przelicz wszystkie kafle państw.
      const owned: number[] = [];
      for (let i = 0; i < this.nation.length; i++) if (this.nation[i] !== 0) owned.push(i);
      this.refresh(owned, false);
    }
  }

  /** Co klatkę: koniec przejść, wysyłka zmienionych kawałków, mundury. */
  update(): void {
    const now = performance.now();
    let settled = 0;
    while (settled < this.recent.length && now - this.recent[settled].at > SETTLE_MS) {
      const tile = this.recent[settled++].tile;
      const [c, o] = this.locate(tile);
      c.data[o + 3] = STABLE;
      c.dirty = true;
    }
    if (settled > 0) this.recent = this.recent.slice(settled);
    const time = this.now();
    for (const c of this.chunks) {
      if (c.dirty) {
        c.source.update();
        c.dirty = false;
      }
      const u = c.mesh.shader?.resources['territoryUniforms']?.uniforms as Record<string, number> | undefined;
      if (u) {
        u['uPolitical'] = this.look.political ? 1 : 0;
        u['uFill'] = this.look.fill;
        u['uBorder'] = this.look.border;
        u['uNow'] = time;
      }
    }
  }

  destroy(): void {
    this.clear();
    this.palette?.destroy(true);
    this.view.destroy();
  }

  /**
   * Zmienione kafle: przejście (nowy właściciel) i przeliczenie krawędzi oraz granicy w ich otoczeniu
   * (promień grubości granicy – tam mogła się zmienić odległość od krawędzi państwa).
   */
  private refresh(changed: number[], animate: boolean): void {
    if (changed.length === 0) return;
    const now = performance.now();
    const time = this.now();
    const r = Math.min(MAX_BORDER, Math.max(1, this.look.borderWidth));
    const w = this.width;
    this.batch++;
    for (const tile of changed) {
      if (animate && this.nation[tile] !== 0) {
        const [c, o] = this.locate(tile);
        c.data[o + 3] = time;
        this.recent.push({ tile, at: now });
      }
      const x = tile % w;
      const y = (tile - x) / w;
      for (let yy = Math.max(0, y - r); yy <= Math.min(this.height - 1, y + r); yy++) {
        for (let xx = Math.max(0, x - r); xx <= Math.min(w - 1, x + r); xx++) {
          const i = yy * w + xx;
          if (this.stamp[i] === this.batch) continue;
          this.stamp[i] = this.batch;
          this.compute(i, r);
        }
      }
    }
  }

  /** Dane kafla: państwo, siła granicy (odległość od innego państwa, wody, ziemi niczyjej), krawędź polityczna. */
  private compute(i: number, r: number): void {
    const [c, o] = this.locate(i);
    const n = this.nation[i];
    c.dirty = true;
    c.data[o] = n;
    if (n === 0) {
      c.data[o + 1] = 0;
      c.data[o + 2] = 0;
      return;
    }
    const w = this.width;
    const h = this.height;
    const x = i % w;
    const y = (i - x) / w;
    // Krawędź polityczna: sąsiad w prowincji z innym państwem albo inna prowincja.
    const p = this.province[i];
    let edge = 0;
    const side = (j: number) => {
      const q = this.province[j];
      if (q !== 0 && this.nation[j] !== n) edge = EDGE_NATION;
      else if (q !== p && edge === 0) edge = EDGE_PROVINCE;
    };
    if (x > 0) side(i - 1);
    if (x + 1 < w) side(i + 1);
    if (y > 0) side(i - w);
    if (y + 1 < h) side(i + w);
    // Granica na mapie terenu: najbliższy kafel innego państwa (poza mapą – też „inny”).
    let best = Infinity;
    for (let dy = -r; dy <= r; dy++) {
      const yy = y + dy;
      for (let dx = -r; dx <= r; dx++) {
        const xx = x + dx;
        const other = xx < 0 || yy < 0 || xx >= w || yy >= h || this.nation[yy * w + xx] !== n;
        if (other) best = Math.min(best, dx * dx + dy * dy);
      }
    }
    const depth = Math.sqrt(best) - 1;
    const width = this.look.borderWidth;
    const t = width > 1 ? depth / (width - 1) : 0;
    const strength = depth < width ? 1 - (1 - BORDER_INNER) * Math.min(1, t) : 0;
    c.data[o + 1] = Math.round(strength * 255);
    c.data[o + 2] = edge;
  }

  /** Kawałek i przesunięcie kafla w jego danych. */
  private locate(tile: number): [Chunk, number] {
    const x = tile % this.width;
    const y = (tile - x) / this.width;
    const c = this.chunks[Math.floor(y / CHUNK) * this.per + Math.floor(x / CHUNK)];
    return [c, ((y - c.y0) * c.w + (x - c.x0)) * 4];
  }

  /** Czas w jednostkach kanału A (0–254, zawija się). */
  private now(): number {
    return Math.floor((performance.now() - this.start) / TIME_UNIT_MS) % 255;
  }

  private chunk(x0: number, y0: number, w: number, h: number): Chunk {
    const data = new Uint8Array(w * h * 4);
    for (let o = 3; o < data.length; o += 4) data[o] = STABLE;
    const source = new BufferImageSource({ resource: data, width: w, height: h, scaleMode: 'nearest' });
    const mesh = new Mesh({
      geometry: new MeshGeometry({
        positions: new Float32Array([0, 0, w, 0, w, h, 0, h]),
        uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      }),
      shader: Shader.from({
        gl: { vertex, fragment, preferredFragmentPrecision: 'highp' },
        resources: {
          uData: source,
          uPalette: this.palette!.source,
          territoryUniforms: {
            uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
            uPolitical: { value: 0, type: 'f32' },
            uFill: { value: this.look.fill, type: 'f32' },
            uBorder: { value: this.look.border, type: 'f32' },
            uNow: { value: 0, type: 'f32' },
          },
        },
      }),
    });
    mesh.position.set(x0, y0);
    this.view.addChild(mesh);
    return { x0, y0, w, h, data, source, mesh, dirty: true };
  }

  private clear(): void {
    for (const c of this.chunks) {
      const { geometry, shader } = c.mesh;
      c.mesh.destroy();
      geometry.destroy();
      shader?.destroy();
      c.source.destroy();
    }
    this.chunks = [];
    this.recent = [];
  }
}

/** Paleta 256 × 1: kolor państwa `n` w kolumnie `n`. */
function paletteTexture(): Texture {
  const data = new Uint8Array(256 * 4);
  for (let n = 1; n < 256; n++) {
    const [r, g, b] = NATION_COLORS[(n - 1) % NATION_COLORS.length];
    data.set([r, g, b, 255], n * 4);
  }
  return new Texture({ source: new BufferImageSource({ resource: data, width: 256, height: 1, scaleMode: 'nearest' }) });
}
