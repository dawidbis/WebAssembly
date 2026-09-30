import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Animacja pojawia się od tylu pikseli na kafel (z daleka rzeki są za wąskie – migotałyby). */
const FADE_FROM_PX = 1.5;
const FADE_TO_PX = 3.5;
/** Wartości nurtu w teksturze są zapisane modulo tyle kafli (wielokrotność długości fali). */
const FLOW_WRAP = 64;

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
 * Tekstura danych (filtrowanie „nearest”, jeden teksel = jeden kafel):
 * R = 255 rzeka, 128 jezioro, 0 reszta; G = odległość do ujścia modulo 64 (× 4);
 * B = odległość od brzegu jeziora (× 40). Shader interpoluje je ręcznie (dwuliniowo), a nurt
 * „rozwija” przez zawinięcie modulo, więc fazy płyną gładko także przez granicę kafli.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform vec2 uSize;
uniform float uTime;
uniform float uStrength;
uniform float uFade;

const float WRAP = ${FLOW_WRAP.toFixed(1)};

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

vec4 cellData(vec2 c) {
  return texture(uData, (c + 0.5) / uSize);
}

void main() {
  vec2 p = vUV * uSize;
  vec4 here = cellData(floor(p));
  bool river = here.r > 0.75;
  bool lake = here.r > 0.3 && here.r < 0.75;
  if (!river && !lake) discard;

  // Ręczna interpolacja dwuliniowa z 4 sąsiednich kafli.
  vec2 f = p - 0.5;
  vec2 i0 = floor(f);
  vec2 t = fract(f);
  float ref = here.g * 255.0 / 4.0;
  float flowSum = 0.0;
  float flowW = 0.0;
  float lakeDist = 0.0;
  for (int k = 0; k < 4; k++) {
    vec2 o = vec2(mod(float(k), 2.0), floor(float(k) / 2.0));
    vec4 s = cellData(i0 + o);
    float wgt = (o.x > 0.5 ? t.x : 1.0 - t.x) * (o.y > 0.5 ? t.y : 1.0 - t.y);
    if (s.r > 0.75) {
      float v = s.g * 255.0 / 4.0;
      v += WRAP * floor((ref - v) / WRAP + 0.5);
      flowSum += v * wgt;
      flowW += wgt;
    }
    lakeDist += (s.r > 0.3 && s.r < 0.75 ? s.b * 255.0 / 40.0 : 0.0) * wgt;
  }

  float white = 0.0;
  if (river) {
    // Nurt: jasne smugi płyną w stronę malejącej odległości do ujścia (z prądem), ok. 3 kafle/s.
    float flow = flowW > 0.0 ? flowSum / flowW : ref;
    float along = flow + uTime * 3.0;
    float cross = (p.x - p.y) * 0.35;
    float ripple = pow(1.0 - abs(fract(along / 4.0 + noise(vec2(along * 0.15, cross)) * 0.9) - 0.5) * 2.0, 6.0);
    float breakup = smoothstep(0.35, 0.75, noise(vec2(along * 0.45, cross * 1.7)));
    float glint = smoothstep(0.72, 0.95, noise(vec2(along * 0.9, cross * 3.0)));
    white = 0.32 * ripple * breakup + 0.28 * glint;
  } else {
    // Jezioro: powolne, delikatne zmarszczki i piana lekko falująca przy brzegu.
    float n = noise(p * 0.12 + vec2(uTime * 0.05, uTime * 0.035));
    float lines = pow(1.0 - abs(fract(n * 5.0 + uTime * 0.12) - 0.5) * 2.0, 10.0);
    float lap = 0.9 + 0.35 * sin(uTime * 0.9 + noise(p * 0.2) * 6.2832);
    float foam = (1.0 - smoothstep(0.25, lap, lakeDist)) * (0.5 + 0.3 * noise(p * 0.5 + uTime * 0.2));
    white = 0.16 * lines * smoothstep(0.8, 2.0, lakeDist) + 0.4 * foam;
  }

  float a = clamp(white * uStrength, 0.0, 0.8) * uFade;
  if (a <= 0.003) discard;
  finalColor = vec4(vec3(0.88, 0.96, 1.0) * a, a);
}
`;

interface InlandUniforms {
  uTime: number;
  uStrength: number;
  uFade: number;
}

/** Animacja rzek (nurt płynący do ujścia) i jezior (zmarszczki, piana przy brzegu). */
export class InlandWaterLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private time = 0;
  private strength = 0.8;
  private speed = 1;

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, terrain, riverFlow, lakeDist } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      if (terrain[i] === 2) {
        data[o] = 255;
        data[o + 1] = (riverFlow[i] % FLOW_WRAP) * 4;
      } else if (terrain[i] === 1) {
        data[o] = 128;
        data[o + 2] = Math.min(255, lakeDist[i] * 40);
      }
      data[o + 3] = 255;
    }
    this.texture = new Texture({
      source: new BufferImageSource({ resource: data, width: w, height: h, scaleMode: 'nearest' }),
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
        inlandUniforms: {
          uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
          uTime: { value: this.time, type: 'f32' },
          uStrength: { value: this.strength, type: 'f32' },
          uFade: { value: 0, type: 'f32' },
        },
      },
    });
    this.mesh = new Mesh({ geometry, shader });
    this.mesh.visible = false;
    this.view.addChild(this.mesh);
  }

  configure(strength: number, speed: number): void {
    this.strength = strength;
    this.speed = speed;
    const u = this.uniforms();
    if (u) u.uStrength = strength;
  }

  /** Wołane co klatkę: czas animacji i widoczność zależna od przybliżenia. */
  update(seconds: number, tilesPerPixel: number): void {
    if (!this.mesh) return;
    const px = 1 / tilesPerPixel;
    const fade = Math.min(1, Math.max(0, (px - FADE_FROM_PX) / (FADE_TO_PX - FADE_FROM_PX)));
    this.mesh.visible = fade > 0 && this.strength > 0;
    if (!this.mesh.visible) return;
    this.time = (this.time + seconds * this.speed) % 3600;
    const u = this.uniforms();
    if (u) {
      u.uTime = this.time;
      u.uFade = fade * fade * (3 - 2 * fade);
    }
  }

  destroy(): void {
    this.clear();
    this.view.destroy();
  }

  private uniforms(): InlandUniforms | null {
    return (this.mesh?.shader?.resources['inlandUniforms']?.uniforms as InlandUniforms | undefined) ?? null;
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
