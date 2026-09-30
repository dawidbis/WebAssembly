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

/** Dryf błysków w lewo (kafle na sekundę). */
const float GLITTER_DRIFT = 1.2;

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
  // Cały wzór błysków powoli dryfuje w lewo (próbkujemy przesunięte w prawo współrzędne).
  vec2 g = p + vec2(uTime * GLITTER_DRIFT, 0.0);
  // Komórka mieści dwa błyski; jest na tyle duża, że nawet długie smugi nie wystają poza
  // sąsiednie komórki, które sprawdzamy (3×3).
  float cellSize = 8.0 * exp2(max(0.0, ceil(log2(tilesPerPixel * 1.6))));
  float pxScale = max(1.0, tilesPerPixel * 1.2);   // kafle na „piksel błysku”
  vec2 baseCell = floor(g / cellSize);
  float lighten = 0.0;
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      for (int k = 0; k < 2; k++) {
        vec2 cell = baseCell + vec2(float(i), float(j));
        vec2 id = cell + float(k) * vec2(17.3, 29.1);
        float period = 1.2 + 2.8 * hash(id + 2.3);
        float phase0 = hash(id + 9.1);
        float tt = uTime / period + phase0;
        float flash = floor(tt);
        float life = fract(tt);
        vec2 seed = id + flash * vec2(3.71, 1.37);
        vec2 centerG = (cell + 0.12 + 0.76 * vec2(hash(seed + 4.2), hash(seed + 7.7))) * cellSize;
        vec2 d = (g - centerG) / pxScale;              // w „pikselach błysku”
        if (dot(d, d) > 90.0) continue;
        // Czy błysk świeci – liczone raz dla całego błysku: w jego środku i w chwili zapalenia,
        // żeby nie ucinał się w połowie ani nie gasł nagle, gdy plama przesunie się dalej.
        float born = (flash - phase0) * period;
        vec2 centerP = centerG - vec2(born * GLITTER_DRIFT, 0.0);
        float patches = smoothstep(0.45, 0.8, noise(centerG * 0.012 + vec2(born * 0.01, born * 0.018)));
        float facets = smoothstep(0.35, 0.75, noise(centerG * 0.09 + vec2(-born * 0.06, born * 0.05)));
        float shallow = 1.0 - smoothstep(0.08, 0.45, texture(uData, centerP / uSize).g);
        float chance = uGlitter * 1.6 * (0.12 + 0.6 * patches) * (0.4 + 0.6 * facets) * (1.0 + 0.4 * shallow);
        if (hash(seed + 0.5) > chance) continue;

        // Losowy wygląd tego błysku: rozmiar, obrót, jasność i kształt.
        float size = 0.55 + 1.1 * pow(hash(seed + 11.1), 2.0);
        float angle = hash(seed + 12.7) * 3.14159;
        float ca = cos(angle);
        float sa = sin(angle);
        vec2 r = vec2(ca * d.x + sa * d.y, -sa * d.x + ca * d.y) / size;
        float bright = 0.45 + 0.75 * hash(seed + 13.9);
        float kind = hash(seed + 15.3);
        float shape;
        if (kind < 0.35) {
          // Kropka, lekko spłaszczona.
          float squash = 1.0 + 0.8 * hash(seed + 16.1);
          shape = exp(-(r.x * r.x / squash + r.y * r.y * squash) * 0.9);
        } else if (kind < 0.6) {
          // Smuga: wydłużony błysk w losowym kierunku.
          float len = 2.0 + 2.5 * hash(seed + 17.7);
          shape = exp(-(r.x * r.x / (len * len) + r.y * r.y * 2.5));
        } else if (kind < 0.85) {
          // Gwiazdka: rdzeń i promienie o różnej długości.
          float ray1 = 0.9 + 1.4 * hash(seed + 18.3);
          float ray2 = 0.9 + 1.4 * hash(seed + 19.9);
          shape = exp(-dot(r, r) * 1.1)
            + 0.45 * exp(-abs(r.x) * ray1 - r.y * r.y * 7.0)
            + 0.45 * exp(-abs(r.y) * ray2 - r.x * r.x * 7.0);
        } else {
          // Para drobnych kropek obok siebie.
          vec2 off = vec2(0.9 + 0.8 * hash(seed + 20.5), 0.0);
          shape = 0.8 * (exp(-dot(r - off, r - off) * 2.2) + exp(-dot(r + off, r + off) * 1.6));
        }
        // Każdy błysk ma własny rytm: ostry albo miękki, czasem z drżeniem przed zgaśnięciem.
        float twinkle = pow(sin(3.14159 * life), 2.0 + 7.0 * hash(seed + 21.1));
        float shimmer = hash(seed + 22.7) < 0.3 ? 0.65 + 0.35 * sin(life * 40.0 + seed.x) : 1.0;
        lighten += twinkle * shimmer * bright * shape;
      }
    }
  }
  lighten *= uAmbient;
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
