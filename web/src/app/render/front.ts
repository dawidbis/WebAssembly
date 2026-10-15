import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { NATION_COLORS } from './nations';

/** Kafel zajęty na froncie pojawia się w kolorze państwa w tym czasie (ms), z krótkim rozbłyskiem. */
const FADE_MS = 300;
/** Jednostka czasu zmiany w teksturze (ms) – 16 bitów, zawija się co ok. 11 min (wystarczy na przenikanie). */
const TIME_UNIT_MS = 10;
/** Kawałki nakładki – aktualizowane są tylko te, w których coś się zmieniło. */
const CHUNK = 512;

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
 * Dane kafla: R – państwo teraz, G – państwo na namalowanych warstwach terytoriów, B/A – chwila zmiany
 * (młodszy/starszy bajt, w `TIME_UNIT_MS`). Rysowane są tylko kafle, gdzie R ≠ G (zajęte od ostatniego
 * pełnego przemalowania), w kolorze państwa z palety.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform sampler2D uPalette;
uniform vec2 uSize;
uniform float uNow;
uniform float uAlpha;

void main() {
  vec4 d = texture(uData, (floor(vUV * uSize) + 0.5) / uSize);
  float now = floor(d.r * 255.0 + 0.5);
  float painted = floor(d.g * 255.0 + 0.5);
  if (now < 0.5 || abs(now - painted) < 0.5) discard;
  float t = floor(d.b * 255.0 + 0.5) + floor(d.a * 255.0 + 0.5) * 256.0;
  float age = mod(uNow - t + 65536.0, 65536.0) * ${TIME_UNIT_MS.toFixed(1)};
  float k = clamp(age / ${FADE_MS.toFixed(1)}, 0.0, 1.0);
  vec3 color = texture(uPalette, vec2((now + 0.5) / 256.0, 0.5)).rgb;
  // Rozbłysk przy zajęciu: jaśniej na początku, potem kolor państwa.
  color = mix(vec3(1.0), color, 0.4 + 0.6 * k);
  float a = uAlpha * k;
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

interface FrontUniforms {
  uNow: number;
  uAlpha: number;
}

/**
 * Front na żywo: kafle zajęte od ostatniego pełnego przemalowania terytoriów (`paintTerritories` idzie
 * najwyżej raz na sekundę) – co turę, z płynnym pojawianiem się. Po przemalowaniu (`setPainted`) te kafle
 * są już na zwykłych warstwach i nakładka je pomija.
 */
export class FrontLayer {
  readonly view = new Container();
  private chunks: Chunk[] = [];
  private width = 0;
  private palette: Texture | null = null;
  private alpha = 1;
  private readonly start = performance.now();

  setMap(map: MapPayload): void {
    this.clear();
    this.width = map.width;
    this.palette ??= paletteTexture();
    for (let y0 = 0; y0 < map.height; y0 += CHUNK) {
      for (let x0 = 0; x0 < map.width; x0 += CHUNK) {
        this.chunks.push(this.chunk(x0, y0, Math.min(CHUNK, map.width - x0), Math.min(CHUNK, map.height - y0)));
      }
    }
  }

  /** Państwa na kaflach z pełnego stanu (nowy kafel względem nakładki – pojawia się płynnie od teraz). */
  setOwners(tiles: Uint8Array): void {
    const now = this.now();
    for (const c of this.chunks) {
      for (let row = 0; row < c.h; row++) {
        const from = (c.y0 + row) * this.width + c.x0;
        for (let col = 0; col < c.w; col++) {
          const o = (row * c.w + col) * 4;
          const n = tiles[from + col];
          if (c.data[o] === n) continue;
          c.data[o] = n;
          c.data[o + 2] = now & 255;
          c.data[o + 3] = now >> 8;
          c.dirty = true;
        }
      }
    }
  }

  /** Zmiany kafli z tury (`kafel << 8 | państwo`). */
  apply(changes: Uint32Array): void {
    const now = this.now();
    const per = Math.ceil(this.width / CHUNK);
    for (const change of changes) {
      const tile = change >>> 8;
      const x = tile % this.width;
      const y = (tile - x) / this.width;
      const c = this.chunks[Math.floor(y / CHUNK) * per + Math.floor(x / CHUNK)];
      if (!c) continue;
      const o = ((y - c.y0) * c.w + (x - c.x0)) * 4;
      c.data[o] = change & 255;
      c.data[o + 2] = now & 255;
      c.data[o + 3] = now >> 8;
      c.dirty = true;
    }
  }

  /** Właściciele, z których namalowano warstwy terytoriów – te kafle nakładka pomija. */
  setPainted(tiles: Uint8Array | null): void {
    for (const c of this.chunks) {
      for (let row = 0; row < c.h; row++) {
        const from = (c.y0 + row) * this.width + c.x0;
        for (let col = 0; col < c.w; col++) {
          const o = (row * c.w + col) * 4 + 1;
          const n = tiles ? tiles[from + col] : 0;
          if (c.data[o] === n) continue;
          c.data[o] = n;
          c.dirty = true;
        }
      }
    }
  }

  /** Krycie koloru państwa (jak warstwa terytoriów w bieżącym widoku). */
  setAlpha(alpha: number): void {
    this.alpha = alpha;
  }

  /** Co klatkę: czas do przenikania i wysłanie zmienionych kawałków na GPU. */
  update(): void {
    const now = this.now();
    for (const c of this.chunks) {
      if (c.dirty) {
        c.source.update();
        c.dirty = false;
      }
      const u = c.mesh.shader?.resources['frontUniforms']?.uniforms as FrontUniforms | undefined;
      if (u) {
        u.uNow = now;
        u.uAlpha = this.alpha;
      }
    }
  }

  destroy(): void {
    this.clear();
    this.palette?.destroy(true);
    this.view.destroy();
  }

  private now(): number {
    return Math.floor((performance.now() - this.start) / TIME_UNIT_MS) & 0xffff;
  }

  private chunk(x0: number, y0: number, w: number, h: number): Chunk {
    const data = new Uint8Array(w * h * 4);
    const source = new BufferImageSource({ resource: data, width: w, height: h, scaleMode: 'nearest' });
    const geometry = new MeshGeometry({
      positions: new Float32Array([0, 0, w, 0, w, h, 0, h]),
      uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
      indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
    });
    const shader = Shader.from({
      gl: { vertex, fragment, preferredFragmentPrecision: 'highp' },
      resources: {
        uData: source,
        uPalette: this.palette!.source,
        frontUniforms: {
          uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
          uNow: { value: 0, type: 'f32' },
          uAlpha: { value: this.alpha, type: 'f32' },
        },
      },
    });
    const mesh = new Mesh({ geometry, shader });
    mesh.position.set(x0, y0);
    this.view.addChild(mesh);
    return { x0, y0, w, h, data, source, mesh, dirty: false };
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
