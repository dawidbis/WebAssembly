import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Od ilu pikseli na kafel drzewa zaczynają się pojawiać i przy ilu są w pełni widoczne. */
const FADE_FROM_PX = 4;
const FADE_TO_PX = 8;

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
 * R = gęstość lasu, G = (biom · 5 + drugi biom) · 10, B = udział drugiego biomu (0..128).
 *
 * Każdy kafel lasu to kandydat na drzewo w losowym miejscu kafla (w tajdze i dżungli dwóch
 * kandydatów – gęstszy las). Korona wystaje poza kafel, więc piksel sprawdza drzewa z 3×3
 * sąsiednich kafli, od górnych do dolnych (dolne zasłaniają górne). Gatunek wynika z biomu;
 * w strefie przejścia każde drzewo losuje gatunek według udziału biomów.
 * Każde drzewo ma własną jasność i odcień, a wolnozmienny szum dodaje całe płaty innej zieleni.
 * Światło z lewego górnego rogu, cień w prawo w dół.
 */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform vec2 uSize;
uniform float uTilesPerPixel;
uniform float uFade;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Gładki szum wartości 0..1 – płaty lasu o innym odcieniu.
float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Kolor „nad” dotychczasowym (premultiplied alpha).
void over(inout vec4 acc, vec3 color, float alpha) {
  acc.rgb = color * alpha + acc.rgb * (1.0 - alpha);
  acc.a = alpha + acc.a * (1.0 - alpha);
}

float disc(vec2 q, vec2 c, vec2 r, float aa) {
  vec2 d = (q - c) / r;
  return 1.0 - smoothstep(1.0 - aa / min(r.x, r.y), 1.0 + aa / min(r.x, r.y), length(d));
}

// Trójkąt równoramienny: wierzchołek (0, top), podstawa na wysokości bottom, połowa szerokości halfw.
float cone(vec2 q, float top, float bottom, float halfw, float aa) {
  float t = (q.y - top) / (bottom - top);
  float side = halfw * t - abs(q.x);
  return smoothstep(-aa, aa, side) * smoothstep(-aa, aa, bottom - q.y) * step(0.0, t);
}

float bar(vec2 q, float x0, float y0, float y1, float halfw, float aa) {
  return smoothstep(-aa, aa, halfw - abs(q.x - x0)) * step(y0, q.y) * step(q.y, y1);
}

// Pofalowana korona (dąb): promień zmienia się z kątem.
float lumpy(vec2 q, vec2 c, float radius, float phase, float aa) {
  vec2 r = q - c;
  float ang = atan(r.y, r.x);
  float wave = 1.0 + 0.11 * sin(5.0 * ang + phase) + 0.06 * sin(9.0 * ang + phase * 1.7);
  float edge = radius * wave;
  return 1.0 - smoothstep(edge - aa, edge + aa, length(r));
}

const vec3 TRUNK = vec3(0.30, 0.22, 0.15);
const vec3 SNOW = vec3(0.90, 0.93, 0.96);

// Odcień drzewa: jasność i przesunięcie między żółtawą a niebieskawą zielenią.
vec3 tint(vec3 green, float bright, float hue) {
  return green * bright + vec3(0.05, 0.025, -0.04) * hue;
}

// Świerk / jodła: dwa piętra stożka, opcjonalnie z czapą śniegu.
void conifer(inout vec4 acc, vec2 q, vec3 green, bool snowy, float aa) {
  over(acc, TRUNK, bar(q, 0.0, 0.2, 0.38, 0.04, aa));
  float lower = cone(q, -0.3, 0.26, 0.36, aa);
  float upper = cone(q, -0.62, -0.02, 0.28, aa);
  vec3 lit = q.x < 0.0 ? green * 1.2 : green * 0.88;
  over(acc, lit, lower);
  over(acc, lit * 1.05, upper);
  if (snowy) {
    float capLow = lower * (1.0 - step(-0.12, q.y)) * (1.0 - upper);
    float capUp = upper * (1.0 - step(-0.4, q.y));
    over(acc, SNOW, 0.85 * max(capLow, capUp));
  }
}

void drawTree(inout vec4 acc, vec2 p, vec2 cell, float slot, vec4 d) {
  vec2 id = cell + slot * vec2(37.1, 17.3);
  float code = floor(d.g * 25.5 + 0.5);
  float primary = floor(code / 5.0);
  float other = code - primary * 5.0;
  float kind = hash(id + 7.7) < d.b * 255.0 / 256.0 ? other : primary;
  bool dense = kind == 2.0 || kind == 3.0;
  // Drugi kandydat tylko w tajdze i dżungli.
  if (slot > 0.5 && !dense) return;
  if (hash(id) > d.r * (dense ? 0.98 : 0.95)) return;

  float jungle = kind == 3.0 ? 1.0 : 0.0;
  vec2 center = cell + 0.2 + 0.6 * vec2(hash(id + 1.3), hash(id + 2.9));
  float size = (kind == 2.0 ? 0.7 : 0.8) + (0.4 + 0.25 * jungle) * hash(id + 4.1);   // w kaflach
  vec2 q = (p - center) / size;                                                     // y w dół, podstawa ok. y = 0.35
  if (dot(q, q) > 1.4) return;
  float aa = uTilesPerPixel / size;

  // Odcień: indywidualny + płaty (w dżungli wyraźne).
  float blotch = vnoise(cell * (0.09 + 0.05 * jungle));
  float bright = (0.82 + 0.34 * hash(id + 5.5)) * (0.85 + (0.3 + 0.25 * jungle) * blotch);
  float hue = (hash(id + 6.6) * 2.0 - 1.0) * 0.7 + (blotch - 0.5) * (1.0 + 1.6 * jungle);

  // Cień na ziemi.
  over(acc, vec3(0.0), 0.28 * disc(q, vec2(0.16, 0.34), vec2(0.4, 0.14), aa));

  if (kind == 2.0) {
    conifer(acc, q, tint(vec3(0.14, 0.30, 0.24), bright, hue * 0.6), hash(id + 9.1) < 0.42, aa);
  } else if (kind == 3.0) {
    // Dżungla: duże, nachodzące na siebie kępy; czasem wyższe drzewo wystające ponad resztę.
    vec3 green = tint(vec3(0.09, 0.35, 0.13), bright, hue);
    bool emergent = hash(id + 8.8) < 0.1;
    float s = emergent ? 1.2 : 1.0;
    float a = disc(q, vec2(-0.17, -0.02) * s, vec2(0.31) * s, aa);
    float b = disc(q, vec2(0.18, -0.04) * s, vec2(0.29) * s, aa);
    float c = disc(q, vec2(0.0, -0.24) * s, vec2(0.32) * s, aa);
    over(acc, green * 0.72, max(a, b));
    over(acc, green * 1.08, c);
    over(acc, green * 1.35, 0.6 * disc(q, vec2(-0.09, -0.33) * s, vec2(0.13) * s, aa));
    over(acc, green * 1.2, 0.45 * disc(q, vec2(-0.22, -0.1) * s, vec2(0.1) * s, aa));
    if (emergent) over(acc, green * 1.45, 0.5 * disc(q, vec2(0.05, -0.38), vec2(0.16), aa));
  } else if (kind == 1.0) {
    // Oaza: palma – cienki pień i gwiaździsta korona.
    vec3 green = tint(vec3(0.34, 0.58, 0.24), bright, hue * 0.5);
    over(acc, TRUNK * 1.3, bar(q, 0.08 * (q.y - 0.36), -0.24, 0.36, 0.05, aa));
    vec2 r = q - vec2(0.0, -0.28);
    float ang = atan(r.y, r.x);
    float reach = 0.46 * (0.4 + 0.6 * abs(cos(ang * 2.5)));
    float fronds = 1.0 - smoothstep(reach - aa, reach + aa, length(r));
    over(acc, q.x < 0.0 ? green * 1.15 : green * 0.9, fronds);
  } else if (kind == 4.0) {
    // Step: mały, okrągły krzew.
    vec3 green = tint(vec3(0.44, 0.50, 0.24), bright, hue * 0.5);
    vec2 c = vec2(0.0, 0.04);
    float crown = disc(q, c, vec2(0.26), aa);
    float light = clamp(0.5 - (q.x - c.x + q.y - c.y) / 0.62, 0.0, 1.0);
    over(acc, green * (0.8 + 0.45 * light), crown);
  } else if (hash(id + 11.3) < 0.2) {
    // Umiarkowany: co piąte drzewo iglaste (świerk, bez śniegu).
    conifer(acc, q, tint(vec3(0.13, 0.33, 0.22), bright, hue * 0.5), false, aa);
  } else {
    // Umiarkowany: dąb – pofalowana korona z kępami liści, jaśniejsza od strony światła.
    vec3 green = tint(vec3(0.27, 0.48, 0.19), bright, hue);
    vec2 c = vec2(0.0, -0.12);
    float phase = hash(id + 12.1) * 6.2832;
    over(acc, TRUNK, bar(q, 0.0, 0.08, 0.36, 0.055, aa));
    float crown = lumpy(q, c, 0.4, phase, aa);
    float light = clamp(0.5 - (q.x - c.x + q.y - c.y) / 0.96, 0.0, 1.0);
    over(acc, green * (0.72 + 0.5 * light), crown);
    // Kępy liści: jaśniejsze od światła, ciemniejsze w cieniu – wszystko w obrysie korony.
    over(acc, green * 1.28, 0.6 * crown * lumpy(q, c + vec2(-0.14, -0.14), 0.15, phase + 1.0, aa));
    over(acc, green * 1.15, 0.5 * crown * lumpy(q, c + vec2(0.1, -0.2), 0.12, phase + 2.0, aa));
    over(acc, green * 1.1, 0.4 * crown * lumpy(q, c + vec2(-0.2, 0.06), 0.11, phase + 3.0, aa));
    over(acc, green * 0.68, 0.55 * crown * lumpy(q, c + vec2(0.14, 0.13), 0.14, phase + 4.0, aa));
  }
}

void main() {
  vec2 p = vUV * uSize;
  vec2 base = floor(p);
  vec4 acc = vec4(0.0);

  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 cell = base + vec2(float(i), float(j));
      if (cell.x < 0.0 || cell.y < 0.0 || cell.x >= uSize.x || cell.y >= uSize.y) continue;
      vec4 d = texture(uData, (cell + 0.5) / uSize);
      if (d.r < 0.02) continue;
      drawTree(acc, p, cell, 0.0, d);
      drawTree(acc, p, cell, 1.0, d);
    }
  }

  if (acc.a <= 0.0) discard;
  finalColor = acc * uFade;
}
`;

interface TreeUniforms {
  uTilesPerPixel: number;
  uFade: number;
}

/** Symbole drzew przy przybliżeniu – rysowane shaderem tylko w widocznym fragmencie mapy. */
export class TreeLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private enabled = true;
  private fadeValue = 0;

  /** Obecna widoczność drzew 0..1 (0 = oddalone albo wyłączone). */
  get fade(): number {
    return this.fadeValue;
  }

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, forest, biome, biomeOther, biomeMix } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      data[o] = forest[i];
      data[o + 1] = (biome[i] * 5 + biomeOther[i]) * 10;
      data[o + 2] = biomeMix[i];
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
        treeUniforms: {
          uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
          uTilesPerPixel: { value: 1, type: 'f32' },
          uFade: { value: 0, type: 'f32' },
        },
      },
    });
    this.mesh = new Mesh({ geometry, shader });
    this.mesh.visible = false;
    this.view.addChild(this.mesh);
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (!enabled && this.mesh) this.mesh.visible = false;
    if (!enabled) this.fadeValue = 0;
  }

  /** Wołane co klatkę: dopasowuje rysowanie do przybliżenia i wyłącza shader, gdy drzew nie widać. */
  update(tilesPerPixel: number): void {
    if (!this.mesh) return;
    const px = 1 / tilesPerPixel;
    const fade = Math.min(1, Math.max(0, (px - FADE_FROM_PX) / (FADE_TO_PX - FADE_FROM_PX)));
    this.mesh.visible = this.enabled && fade > 0;
    this.fadeValue = this.mesh.visible ? fade * fade * (3 - 2 * fade) : 0;
    const u = this.mesh.shader?.resources['treeUniforms']?.uniforms as TreeUniforms | undefined;
    if (u) {
      u.uTilesPerPixel = tilesPerPixel;
      u.uFade = fade * fade * (3 - 2 * fade);
    }
  }

  destroy(): void {
    this.clear();
    this.view.destroy();
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
