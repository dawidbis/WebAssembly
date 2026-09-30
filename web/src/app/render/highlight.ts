import { BufferImageSource, Container, Mesh, MeshGeometry, Shader, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/** Krycie białej nakładki: zaznaczona prowincja (wnętrze, skraj) i prowincja pod kursorem (wnętrze, skraj). */
const SELECTED_FILL = 0.26;
const SELECTED_EDGE = 0.85;
const HOVER_FILL = 0.16;
const HOVER_EDGE = 0.45;

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

/** Tekstura numerów prowincji (R = młodszy bajt, G = starszy, filtrowanie „nearest”). */
const fragment = /* glsl */ `
in vec2 vUV;
out vec4 finalColor;

uniform sampler2D uData;
uniform vec2 uSize;
uniform float uSelected;
uniform float uHover;

float provinceAt(vec2 c) {
  vec4 d = texture(uData, (c + 0.5) / uSize);
  return floor(d.r * 255.0 + 0.5) + floor(d.g * 255.0 + 0.5) * 256.0;
}

void main() {
  vec2 c = floor(vUV * uSize);
  float id = provinceAt(c);
  bool selected = uSelected > 0.5 && abs(id - uSelected) < 0.5;
  bool hovered = uHover > 0.5 && abs(id - uHover) < 0.5;
  if (!selected && !hovered) discard;
  // Skraj: kafel prowincji, którego sąsiad należy do innej prowincji albo jest wodą.
  float edge = 0.0;
  if (abs(provinceAt(c + vec2(1.0, 0.0)) - id) > 0.5) edge = 1.0;
  if (abs(provinceAt(c - vec2(1.0, 0.0)) - id) > 0.5) edge = 1.0;
  if (abs(provinceAt(c + vec2(0.0, 1.0)) - id) > 0.5) edge = 1.0;
  if (abs(provinceAt(c - vec2(0.0, 1.0)) - id) > 0.5) edge = 1.0;
  float a = selected
    ? mix(${SELECTED_FILL.toFixed(2)}, ${SELECTED_EDGE.toFixed(2)}, edge)
    : mix(${HOVER_FILL.toFixed(2)}, ${HOVER_EDGE.toFixed(2)}, edge);
  finalColor = vec4(vec3(1.0) * a, a);
}
`;

interface HighlightUniforms {
  uSelected: number;
  uHover: number;
}

/** Podświetlenie prowincji: lekkie pod kursorem, mocniejsze z wyraźnym skrajem dla zaznaczonej (kliknięcie). */
export class HighlightLayer {
  readonly view = new Container();
  private mesh: Mesh<MeshGeometry, Shader> | null = null;
  private texture: Texture | null = null;
  private selected = 0;
  private hovered = 0;

  setMap(map: MapPayload): void {
    this.clear();
    const { width: w, height: h, province } = map;
    const data = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      data[i * 4] = province[i] & 255;
      data[i * 4 + 1] = province[i] >> 8;
      data[i * 4 + 3] = 255;
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
        highlightUniforms: {
          uSize: { value: new Float32Array([w, h]), type: 'vec2<f32>' },
          uSelected: { value: this.selected, type: 'f32' },
          uHover: { value: this.hovered, type: 'f32' },
        },
      },
    });
    this.mesh = new Mesh({ geometry, shader });
    this.view.addChild(this.mesh);
    this.set(this.selected, this.hovered);
  }

  /** Numery prowincji: zaznaczonej i pod kursorem (0 = brak). */
  set(selected: number, hovered: number): void {
    this.selected = selected;
    this.hovered = hovered;
    if (!this.mesh) return;
    this.mesh.visible = selected > 0 || hovered > 0;
    const u = this.mesh.shader?.resources['highlightUniforms']?.uniforms as HighlightUniforms | undefined;
    if (u) {
      u.uSelected = selected;
      u.uHover = hovered;
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
