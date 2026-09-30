import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Ustawienia animacji – wartości 0..1 (prędkość: mnożnik). */
export interface WaveSettings {
  /** Przybój i piana przy brzegu. */
  shore: number;
  /** Jasność błysków słońca na tafli oceanu. */
  ambient: number;
  /** Ilość błysków słońca (0..1). */
  glitter: number;
  speed: number;
}

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
 * Tekstura danych (liniowo filtrowana): R = odległość od brzegu (kafle / 255, ląd = 0),
 * G = głębokość oceanu (0..1). Współrzędne w kaflach: vUV * uSize.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform float uTime;
uniform vec2 uSize;
uniform float uShore;
uniform float uAmbient;
uniform float uGlitter;
uniform float uTilesPerPixel;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void main() {
  vec4 data = texture(uData, vUV);
  float dist = data.r * 255.0;
  if (dist < 0.5) discard; // ląd, jeziora, rzeki (odległość od brzegu 0)
  float depth = data.g;
  vec2 p = vUV * uSize;

  // --- Fale przybojowe: grzbiety co ~7 kafli płyną w stronę brzegu i wygasają dalej od niego.
  float warp = noise(p * 0.045 + uTime * 0.04);
  float phase = dist / 7.0 + uTime * 0.32 + warp * 1.6;
  float crest = pow(1.0 - abs(fract(phase) - 0.5) * 2.0, 4.0);
  float nearShore = 1.0 - smoothstep(1.5, 16.0, dist);
  float breakup = smoothstep(0.3, 0.7, noise(p * 0.11 + vec2(uTime * 0.15, 0.0)));
  float shore = crest * nearShore * (0.35 + 0.65 * breakup);

  // --- Piana przy samej linii brzegu: pulsuje, gdy fala uderza.
  float surge = 1.6 + 1.0 * sin(uTime * 1.25 + warp * 6.2832);
  float foam = (1.0 - smoothstep(0.6, surge, dist)) * (0.45 + 0.35 * noise(p * 0.3 + uTime * 0.3));

  // --- Błyski słońca na tafli: w każdej komórce co jakiś czas zapala się na ułamek sekundy
  // ostry punkt z krótkim krzyżykiem, a następny błysk pojawia się już gdzie indziej.
  // Wolno wędrujące plamy (fale akurat ustawione do słońca) zagęszczają błyski.
  // Rozmiar komórki zależy od przybliżenia (kafle na piksel), żeby błyski miały zawsze kilka pikseli.
  float tilesPerPixel = uTilesPerPixel;
  float cellSize = 4.0 * exp2(max(0.0, ceil(log2(tilesPerPixel * 1.6))));
  vec2 cell = floor(p / cellSize);
  vec2 local = fract(p / cellSize);
  float period = 0.7 + 1.1 * hash(cell + 2.3);
  float tt = uTime / period + hash(cell + 9.1);
  float flash = floor(tt);
  float life = fract(tt);
  vec2 seed = cell + flash * vec2(3.71, 1.37);
  vec2 center = 0.25 + 0.5 * vec2(hash(seed + 4.2), hash(seed + 7.7));
  vec2 d = (local - center) * cellSize / max(1.0, tilesPerPixel * 1.2);   // w „pikselach błysku”
  float core = exp(-dot(d, d) * 0.9);
  float cross = exp(-abs(d.x) * 1.6 - d.y * d.y * 6.0) + exp(-abs(d.y) * 1.6 - d.x * d.x * 6.0);
  float twinkle = pow(sin(3.14159 * life), 6.0);
  float patches = smoothstep(0.45, 0.8, noise(p * 0.012 + vec2(uTime * 0.03, uTime * 0.018)));
  float facets = smoothstep(0.35, 0.75, noise(p * 0.09 + vec2(-uTime * 0.12, uTime * 0.08)));
  float shallow = 1.0 - smoothstep(0.08, 0.45, depth);
  float chance = uGlitter * (0.12 + 0.6 * patches) * (0.4 + 0.6 * facets) * (1.0 + 0.4 * shallow);
  float on = step(hash(seed + 0.5), chance);
  float lighten = on * twinkle * (core + 0.35 * cross) * uAmbient;
  // Lekkie cienie falowania pod błyskami, żeby tafla nie była płaska.
  float swell = noise(p * 0.05 + vec2(uTime * 0.06, -uTime * 0.04)) * 2.0 - 1.0;
  float darken = max(0.0, -swell) * 0.07 * uAmbient;

  float white = clamp(max(shore * 0.7, foam) * uShore + lighten, 0.0, 0.95);
  // Premultiplied alpha: biel rozjaśnia, czarny z alfą przyciemnia.
  finalColor = vec4(vec3(0.92, 0.97, 1.0) * white, clamp(white + darken, 0.0, 0.9));
}
`;

interface WaveUniforms {
  uTime: number;
  uShore: number;
  uAmbient: number;
  uGlitter: number;
  uTilesPerPixel: number;
}

/** Animowana woda nad terenem: przybój, piana przy brzegu i błyski słońca na tafli oceanu. */
export class WaveLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private time = 0;
  private tilesPerPixel = 1;
  private settings: WaveSettings = { shore: 0.8, ambient: 0.8, glitter: 0.4, speed: 1 };

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, terrain, shade, coastDist } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      const ocean = terrain[i] === 0;
      data[o] = ocean ? coastDist[i] : 0;
      data[o + 1] = ocean ? shade[i] : 0;
      data[o + 3] = 255;
    }
    this.texture = new Texture({
      source: new BufferImageSource({ resource: data, width: w, height: h, scaleMode: 'linear' }),
    });

    const geometry = new MeshGeometry({
      positions: new Float32Array([0, 0, w, 0, w, h, 0, h]),
      uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
      indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
    });
    const shader = Shader.from({
      gl: { vertex, fragment },
      resources: {
        uData: this.texture.source,
        waveUniforms: {
          uTime: { value: this.time, type: 'f32' },
          uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
          uShore: { value: this.settings.shore, type: 'f32' },
          uAmbient: { value: this.settings.ambient, type: 'f32' },
          uGlitter: { value: this.settings.glitter, type: 'f32' },
          uTilesPerPixel: { value: this.tilesPerPixel, type: 'f32' },
        },
      },
    });
    this.mesh = new Mesh({ geometry, shader });
    this.view.addChild(this.mesh);
  }

  configure(settings: WaveSettings): void {
    this.settings = settings;
    const u = this.uniforms();
    if (u) {
      u.uShore = settings.shore;
      u.uAmbient = settings.ambient;
      u.uGlitter = settings.glitter;
    }
  }

  /** Ile kafli mapy przypada na piksel ekranu – rozmiar błysków dopasowuje się do przybliżenia. */
  setTilesPerPixel(value: number): void {
    this.tilesPerPixel = value;
    const u = this.uniforms();
    if (u) u.uTilesPerPixel = value;
  }

  /** Przesuwa animację o `seconds` (czas rzeczywisty; prędkość skaluje go). */
  tick(seconds: number): void {
    this.time = (this.time + seconds * this.settings.speed) % 3600;
    const u = this.uniforms();
    if (u) u.uTime = this.time;
  }

  destroy(): void {
    this.clear();
    this.view.destroy();
  }

  private uniforms(): WaveUniforms | null {
    return (this.mesh?.shader?.resources['waveUniforms']?.uniforms as WaveUniforms | undefined) ?? null;
  }

  private clear(): void {
    if (this.mesh) {
      const { geometry, shader } = this.mesh;
      this.mesh.destroy();
      geometry.destroy();
      shader?.destroy();
    }
    this.mesh = null;
    this.texture?.destroy(true);
    this.texture = null;
  }
}
