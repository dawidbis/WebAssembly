import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import { CURRENT_SCALE, type MapPayload } from '../worker/protocol';

/** Ustawienia animacji – wartości 0..1 (prędkość: mnożnik). */
export interface WaveSettings {
  shore: number;
  /** Jasność paczek fal na otwartym oceanie. */
  open: number;
  /** Jak często pojawiają się paczki fal na otwartym oceanie. */
  density: number;
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

const vec2 OPEN_CELL = vec2(64.0);
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

  // --- Otwarty ocean: paczki grzbietów jak przy brzegu, tylko większe. Każda paczka
  // rodzi się w losowym miejscu, dryfuje z prądem morskim i gaśnie. Grzbiety są poprzeczne
  // do prądu i przesuwają się nieco szybciej niż sama paczka.
  float open = 0.0;
  vec2 base = floor(p / OPEN_CELL);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 cell = base + vec2(float(i), float(j));
      float period = 10.0 + hash(cell + 3.1) * 8.0;
      float tt = uTime / period + hash(cell);
      float cycle = floor(tt);
      float life = fract(tt);
      if (hash(cell + cycle * 1.93 + 5.0) > uDensity) continue;
      vec2 start = (cell + 0.15 + 0.7 * vec2(hash(cell + cycle * 7.13), hash(cell + cycle * 3.71 + 11.0))) * OPEN_CELL;
      vec4 s = texture(uData, start / uSize);
      if (s.r * 255.0 < 14.0) continue;           // paczki rodzą się tylko z dala od brzegu
      vec2 v = (s.gb * 255.0 - 128.0) / CURRENT_SCALE * uCurrent;
      float speed = length(v);
      vec2 dir = speed > 0.05 ? v / speed : vec2(1.0, 0.0);
      vec2 d = p - (start + v * life * period);
      float along = dot(d, dir);
      float across = dot(d, vec2(-dir.y, dir.x));
      float envelope = exp(-along * along / 260.0 - across * across / 1800.0);
      float phase = along / 13.0 - life * period * (0.25 + speed * 0.12) + noise(p * 0.03 + cell) * 1.2;
      float crest = pow(1.0 - abs(fract(phase) - 0.5) * 2.0, 4.0);
      float breakup = smoothstep(0.35, 0.75, noise(vec2(across * 0.09, along * 0.02) + cell * 3.7));
      float fade = smoothstep(0.0, 0.2, life) * (1.0 - smoothstep(0.6, 1.0, life));
      open += crest * envelope * fade * (0.2 + 0.8 * breakup);
    }
  }
  open *= smoothstep(8.0, 20.0, dist);

  float a = clamp(max(shore * 0.7, foam) * uShore + open * 0.65 * uOpen, 0.0, 0.85);
  vec3 color = vec3(0.92, 0.97, 1.0);
  finalColor = vec4(color * a, a);
}
`;

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
  private settings: WaveSettings = { shore: 0.8, open: 0.7, density: 0.35, current: 1, speed: 1 };

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
          uDensity: { value: this.settings.density, type: 'f32' },
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
      u.uDensity = settings.density;
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
