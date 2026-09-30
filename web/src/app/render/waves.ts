import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Ustawienia animacji – wartości 0..1 (prędkość: mnożnik). */
export interface WaveSettings {
  /** Przybój i piana przy brzegu. */
  shore: number;
  /** Delikatne falowanie całej powierzchni oceanu. */
  ambient: number;
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

  // --- Falowanie całego oceanu: trzy siatki refleksów w różnych skalach i kierunkach.
  // Każda powstaje tam, gdzie dwie warstwy dryfującego szumu się pokrywają, i na zmianę
  // wygasa i wraca (fazy przesunięte o 1/3 cyklu), więc wzór ciągle się przenika.
  float glint = 0.0;
  for (int k = 0; k < 3; k++) {
    float fk = float(k);
    float scale = 0.045 + 0.022 * fk;
    vec2 dir = vec2(cos(fk * 2.1 + 0.4), sin(fk * 2.1 + 0.4));
    vec2 a1 = p * scale + dir * uTime * 0.08 + fk * 31.7;
    vec2 a2 = p * scale * 1.55 + vec2(-dir.y, dir.x) * uTime * 0.065 + fk * 53.1 + 17.0;
    float n1 = noise(a1 + noise(a2 * 0.5) * 0.8);
    float n2 = noise(a2);
    float net = pow(1.0 - abs(n1 - n2), 4.0);          // niski wykładnik = szerokie, rozmyte refleksy
    float pulse = 0.5 + 0.5 * sin(uTime * 0.45 + fk * 2.0944);
    glint += net * pulse * pulse;
  }
  glint *= 0.8;
  float swell = noise(p * 0.05 + vec2(uTime * 0.06, -uTime * 0.04)) * 2.0 - 1.0;
  float shallow = 1.0 - smoothstep(0.08, 0.45, depth);
  float lighten = glint * (0.13 + 0.17 * shallow) * uAmbient;
  float darken = max(0.0, -swell) * (0.08 + 0.04 * shallow) * uAmbient;

  float white = clamp(max(shore * 0.7, foam) * uShore + lighten, 0.0, 0.85);
  // Premultiplied alpha: biel rozjaśnia, czarny z alfą przyciemnia.
  finalColor = vec4(vec3(0.92, 0.97, 1.0) * white, clamp(white + darken, 0.0, 0.9));
}
`;

interface WaveUniforms {
  uTime: number;
  uShore: number;
  uAmbient: number;
}

/** Animowana woda nad terenem: przybój, piana przy brzegu i delikatne falowanie całego oceanu. */
export class WaveLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private time = 0;
  private settings: WaveSettings = { shore: 0.8, ambient: 0.6, speed: 1 };

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
