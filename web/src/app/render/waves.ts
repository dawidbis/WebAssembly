import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Ustawienia animacji – wartości 0..1 (prędkość: mnożnik). */
export interface WaveSettings {
  shore: number;
  open: number;
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
 * Tekstura danych (liniowo filtrowana): R = odległość od brzegu (kafle / 255),
 * G = głębokość, B = 1 dla nie-oceanu. Współrzędne w kaflach: vUV * uSize.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform float uTime;
uniform vec2 uSize;
uniform float uShore;
uniform float uOpen;

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
  if (data.b > 0.5) discard; // ląd, jeziora, rzeki
  float dist = data.r * 255.0;
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

  // --- Otwarty ocean: rzadkie grzywacze pojawiające się i znikające w losowych miejscach.
  vec2 cellSize = vec2(18.0, 12.0);
  vec2 cell = floor(p / cellSize);
  float period = 5.0 + hash(cell + 3.1) * 5.0;
  float t = uTime / period + hash(cell);
  float cycle = floor(t);
  float life = fract(t);
  vec2 rnd = vec2(hash(cell + cycle * 7.13), hash(cell + cycle * 3.71 + 11.0));
  float active = step(hash(cell + cycle * 1.93 + 5.0), 0.28);
  vec2 q = (fract(p / cellSize) - 0.2 - rnd * 0.6) * cellSize;   // kafle od środka grzywacza
  q.x += life * 3.0;                                             // grzywacz dryfuje z wiatrem
  float streak = 1.0 - smoothstep(0.4, 1.3, length(q * vec2(0.28, 1.1)));
  float pulse = smoothstep(0.0, 0.12, life) * (1.0 - smoothstep(0.18, 0.45, life));
  float offshore = smoothstep(10.0, 24.0, dist);
  float open = active * streak * pulse * offshore;

  float a = clamp(max(shore * 0.7, foam) * uShore + open * 0.6 * uOpen, 0.0, 0.85);
  vec3 color = vec3(0.92, 0.97, 1.0);
  finalColor = vec4(color * a, a);
}
`;

interface WaveUniforms {
  uTime: number;
  uShore: number;
  uOpen: number;
}

/** Animowane fale nad terenem: przybój, piana przy brzegu, grzywacze na otwartym oceanie. */
export class WaveLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private time = 0;
  private settings: WaveSettings = { shore: 0.8, open: 0.5, speed: 1 };

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, terrain, shade, coastDist } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      const ocean = terrain[i] === 0;
      data[o] = ocean ? coastDist[i] : 0;
      data[o + 1] = ocean ? shade[i] : 0;
      data[o + 2] = ocean ? 0 : 255;
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
