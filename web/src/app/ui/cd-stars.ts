import { Component, computed, input } from '@angular/core';

/** Gwiazdki w rzędach (od góry) dla poziomu 1–6 – jak nad miastem stolicy (`render/buildings.ts`). */
const STAR_ROWS: readonly (readonly number[])[] = [[1], [2], [2, 1], [2, 2], [3, 2], [3, 3]];
/** 1–2 brąz, 3–4 srebro, 5–6 złoto (`STAR_COLORS` w `render/buildings.ts`). */
const STAR_COLORS = ['#c47a3a', '#c47a3a', '#c9d1d9', '#c9d1d9', '#f2c94c', '#f2c94c'];
const STAR = 'M5 0.6 6.2 3.6 9.4 3.8 6.9 5.8 7.8 8.9 5 7.2 2.2 8.9 3.1 5.8 0.6 3.8 3.8 3.6Z';

/** Poziom centrum dowodzenia jako gwiazdki (zamiast „CD n”). */
@Component({
  selector: 'app-cd-stars',
  template: `
    <svg [attr.width]="view().w * size()" [attr.height]="view().h * size()" [attr.viewBox]="'0 0 ' + view().w * 10 + ' ' + view().h * 10" role="img" [attr.aria-label]="'Centrum dowodzenia ' + level()">
      @for (s of view().stars; track $index) {
        <path [attr.d]="star" [attr.transform]="'translate(' + s.x + ' ' + s.y + ')'" [attr.fill]="view().color" stroke="#0c1824" stroke-width="0.8" stroke-linejoin="round" />
      }
    </svg>
  `,
  styles: `:host { display: inline-flex; vertical-align: middle; line-height: 0; }`,
})
export class CdStars {
  readonly level = input.required<number>();
  /** Wysokość jednego rzędu (px). */
  readonly size = input(9);
  protected readonly star = STAR;

  protected readonly view = computed(() => {
    const level = Math.max(1, Math.min(6, Math.round(this.level())));
    const rows = STAR_ROWS[level - 1];
    const w = Math.max(...rows);
    const stars = rows.flatMap((n, row) => Array.from({ length: n }, (_, i) => ({ x: (w - n) * 5 + i * 10, y: row * 10 })));
    return { w, h: rows.length, stars, color: STAR_COLORS[level - 1] };
  });
}
