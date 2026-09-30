import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import { CURRENT_SCALE, type MapPayload } from '../worker/protocol';

/** Ustawienia animacji – wartości 0..1 (prędkość: mnożnik). */
export interface WaveSettings {
  shore: number;
  /** Jasność paczek fal na otwartym oceanie. */
  open: number;
  /** Rzadkość grzywaczy na otwartym oceanie: 0 = gęsto, 1 = prawie wcale. */
  rarity: number;
  /** Mnożnik siły prądów morskich (0 = paczki stoją w miejscu). */
  current: number;
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
 * G/B = prąd morski (vx, vy) zakodowany jak `MapPayload.currents`. Współrzędne w kaflach: vUV * uSize.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform float uTime;
uniform vec2 uSize;
uniform float uShore;
uniform float uOpen;
uniform float uDensity;
uniform float uCurrent;

const vec2 OPEN_CELL = vec2(26.0);
const float OPEN_CYCLE = 10.0;
const float OPEN_WAVELENGTH = 3.5;
const float CURRENT_SCALE = ${CURRENT_SCALE.toFixed(1)};

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

  // --- Otwarty ocean: małe grzywacze narysowane jak przybój (1–3 krótkie grzbiety w poprzek
  // prądu), niesione prądem morskim. Piksel cofa się wzdłuż prądu do miejsca narodzin paczki
  // (q = p − v·wiek), więc paczki płyną bez ucinania na granicach komórek. Dwie warstwy
  // przesunięte o pół cyklu dają ciągły ruch; każda paczka gaśnie przed końcem swojego cyklu.
  vec2 v = (data.gb * 255.0 - 128.0) / CURRENT_SCALE * uCurrent;
  float speed = length(v);
  vec2 dir = speed > 0.05 ? v / speed : vec2(1.0, 0.0);
  vec2 side = vec2(-dir.y, dir.x);
  float open = 0.0;
  for (int layer = 0; layer < 2; layer++) {
    float tt = uTime / OPEN_CYCLE + float(layer) * 0.5;
    float cycle = floor(tt);
    float age = fract(tt);
    vec2 q = p - v * age * OPEN_CYCLE;
    vec2 cell = floor(q / OPEN_CELL);
    vec2 seed = cell + vec2(cycle * 7.31 + float(layer) * 19.7, cycle * 3.17);
    if (hash(seed + 5.0) > uDensity) continue;
    // Środek w środkowej połowie komórki – paczka nie wystaje poza komórkę.
    vec2 center = (cell + 0.25 + 0.5 * vec2(hash(seed + 1.3), hash(seed + 8.9))) * OPEN_CELL;
    vec2 d = q - center;
    float along = dot(d, dir);
    float across = dot(d, side);
    float crests = 1.0 + floor(pow(hash(seed + 2.7), 2.0) * 3.0);   // 1..3, najczęściej 1
    float halfLen = 2.5 + 3.0 * hash(seed + 4.1);                   // połowa długości grzbietu (kafle)
    float u = along / OPEN_WAVELENGTH + crests * 0.5;                 // grzbiety w u ∈ [0, crests]
    float crest = pow(1.0 - abs(fract(u) - 0.5) * 2.0, 4.0)
      * smoothstep(-0.2, 0.3, u) * (1.0 - smoothstep(crests - 0.3, crests + 0.2, u));
    float bend = across / halfLen;
    float span = 1.0 - smoothstep(0.55, 1.0, abs(bend));
    float breakup = smoothstep(0.25, 0.7, noise(vec2(across * 0.35, u) + seed * 3.7));
    float start = 0.1 + 0.35 * hash(seed + 6.6);
    float fade = smoothstep(start, start + 0.08, age) * (1.0 - smoothstep(start + 0.25, start + 0.45, age));
    open += crest * span * fade * (0.3 + 0.7 * breakup);
  }
  open *= smoothstep(8.0, 20.0, dist);

  float a = clamp(max(shore * 0.7, foam) * uShore + open * 0.65 * uOpen, 0.0, 0.85);
  vec3 color = vec3(0.92, 0.97, 1.0);
  finalColor = vec4(color * a, a);
}
`;

/** Rzadkość (0..1) → udział komórek z grzywaczem w danym cyklu. Kwadrat daje czulszy koniec „rzadko”. */
function densityOf(rarity: number): number {
  return 0.6 * (1 - rarity) ** 2;
}

interface WaveUniforms {
  uTime: number;
  uShore: number;
  uOpen: number;
  uDensity: number;
  uCurrent: number;
}

/** Animowane fale nad terenem: przybój, piana przy brzegu, paczki fal niesione prądami na otwartym oceanie. */
export class WaveLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private time = 0;
  private settings: WaveSettings = { shore: 0.8, open: 0.7, rarity: 0.4, current: 1, speed: 1 };

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, terrain, coastDist, currents } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      const ocean = terrain[i] === 0;
      data[o] = ocean ? coastDist[i] : 0;
      data[o + 1] = currents[i * 2];
      data[o + 2] = currents[i * 2 + 1];
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
          uOpen: { value: this.settings.open, type: 'f32' },
          uDensity: { value: densityOf(this.settings.rarity), type: 'f32' },
          uCurrent: { value: this.settings.current, type: 'f32' },
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
      u.uOpen = settings.open;
      u.uDensity = densityOf(settings.rarity);
      u.uCurrent = settings.current;
    }
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
