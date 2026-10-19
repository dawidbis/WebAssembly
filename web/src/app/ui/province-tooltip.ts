import { Component, computed, effect, inject, signal, untracked } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { RESOURCES } from '../render/resources';
import { GoodsIcon } from './goods-icon';
import { ProvinceWindowToggle } from './province-window-toggle';

/** Po tylu ms bez ruchu kursora nad prowincją pokazuje się dymek. */
const DELAY_MS = 800;
/** Ruch kursora o więcej niż tyle px chowa dymek. */
const MOVE_PX = 6;
/** Odsunięcie dymka od kursora (px). */
const OFFSET = 16;
/** Numery infrastruktury i umocnień w tablicy poziomów budynków (`Building as usize`) i liczba rodzajów. */
const INFRASTRUCTURE = 6;
const FORTIFICATIONS = 4;
const BUILDING_COUNT = 8;
/** Ikona infrastruktury (droga) – jak budowa na mapie. */
const ROAD = 'M7 20 10 4M17 20 14 4M12 6v2M12 11v2M12 16v2';
/** Ikona umocnień (mur z blankami) – jak w oknie prowincji. */
const WALL = 'M4 20V8h3v3h3V8h4v3h3V8h3v12z';

/**
 * Dymek nad prowincją w grze właściwej: pokazuje się, gdy kursor chwilę stoi nad mapą (`DELAY_MS`), i znika
 * przy ruchu, kliknięciu albo przewijaniu. Najważniejsze w skrócie: flaga i nazwa, właściciele z udziałem,
 * surowiec, poziom infrastruktury.
 */
@Component({
  selector: 'app-province-tooltip',
  imports: [GoodsIcon, ProvinceWindowToggle],
  template: `
    @if (info(); as i) {
      <div
        class="frame tip"
        [class.touch]="i.touch"
        [style.left.px]="i.x"
        [style.top.px]="i.y"
        [attr.role]="i.touch ? 'dialog' : 'tooltip'"
        [attr.aria-label]="i.name"
      >
        <header>
          @if (i.owners[0]; as main) {
            <img [src]="main.flag" alt="" width="21" height="14" />
          }
          <strong>{{ i.name }}</strong>
          @if (i.touch) {
            <app-province-window-toggle class="open" [province]="i.province" (used)="hide()" />
          }
        </header>
        <div class="row owners">
          @for (o of i.owners; track o.name) {
            <span class="owner"><img [src]="o.flag" alt="" width="15" height="10" />{{ o.name }} <b>{{ o.share }}%</b></span>
          } @empty {
            <span class="nobody">niczyja</span>
          }
        </div>
        <div class="row">
          <app-goods-icon [resource]="i.resource" [size]="16" />
          <span>{{ i.resourceName }}</span>
        </div>
        <div class="row" title="Poziom infrastruktury (bazowo z cech prowincji, rozbudowa w oknie prowincji)">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="road" /></svg>
          <span>Infrastruktura <b>{{ i.infrastructure }}</b></span>
        </div>
        @if (i.fortifications > 0) {
          <div class="row" title="Umocnienia: obrona kafli +50% za poziom dla tego, kto ma ponad połowę prowincji">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="wall" /></svg>
            <span>Umocnienia <b>{{ i.fortifications }}</b></span>
          </div>
        }
      </div>
    }
  `,
  styles: `
    :host {
      position: fixed; inset: 0; pointer-events: none; z-index: 12;
      font: 12px/1.4 system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; font-variant-numeric: tabular-nums;
    }
    .tip {
      position: absolute; display: grid; gap: 4px; padding: 6px 9px; font-size: 12px; color: #d8e2ea; min-width: 140px; max-width: 260px;
      background: rgb(12 24 36 / 0.92); border: 1px solid #243847; border-radius: 6px; box-shadow: 0 4px 16px rgb(0 0 0 / 0.35);
    }
    header { display: flex; align-items: center; gap: 6px; }
    /* Dotyk: dymek przyjmuje dotknięcia (ikonka okna prowincji w prawym górnym rogu). */
    .tip.touch { pointer-events: auto; }
    .open { margin-left: auto; }
    header img { border-radius: 2px; box-shadow: 0 0 0 1px rgb(0 0 0 / 0.5); }
    strong { font-size: 13px; color: #fff; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .row { display: flex; align-items: center; gap: 6px; }
    .owners { flex-wrap: wrap; column-gap: 10px; row-gap: 2px; }
    .owner { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; }
    .owner img { border-radius: 1px; }
    .nobody { color: #8ea3b4; }
    b { color: #fff; font-weight: 600; }
    svg { width: 16px; height: 16px; fill: none; stroke: #c9b37a; stroke-width: 2; stroke-linecap: round; }
  `,
  host: {
    '(window:pointermove)': 'onMove($event)',
    '(window:pointerdown)': 'hide($event)',
    '(window:wheel)': 'hide()',
    '(window:keydown)': 'hide()',
  },
})
export class ProvinceTooltip {
  private readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  protected readonly road = ROAD;
  protected readonly wall = WALL;
  /** Punkt ekranu i prowincja dymka (null – schowany). */
  private readonly at = signal<{ x: number; y: number; province: number; touch?: boolean } | null>(null);
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    // Telefon: dotknięcie prowincji (`MapStore.tapped`) pokazuje dymek od razu – jak dłuższe najechanie myszą.
    effect(() => {
      const tap = this.store.tapped();
      untracked(() => {
        clearTimeout(this.timer);
        this.at.set(tap ? { ...tap, touch: true } : null);
      });
    });
  }

  protected onMove(event: PointerEvent): void {
    if (event.pointerType === 'touch' || this.at()?.touch) return;
    const shown = this.at();
    if (shown && Math.hypot(event.clientX - shown.x, event.clientY - shown.y) <= MOVE_PX) return;
    this.hide();
    // Tylko nad mapą (płótno renderera) i w grze właściwej.
    if (!(event.target instanceof HTMLCanvasElement) || this.session.phase() !== 'playing' || event.buttons !== 0) return;
    const { clientX: x, clientY: y } = event;
    this.timer = setTimeout(() => {
      const province = this.store.hoveredProvince();
      if (province > 0) this.at.set({ x, y, province });
    }, DELAY_MS);
  }

  /** Chowa dymek – poza dotknięciem samego dymku (ikonka okna prowincji). */
  protected hide(event?: Event): void {
    if (event?.target instanceof Element && event.target.closest('.tip')) return;
    clearTimeout(this.timer);
    if (this.at()) this.at.set(null);
    if (this.store.tapped()) this.store.tapped.set(null);
  }

  protected readonly info = computed(() => {
    const at = this.at();
    const province = at && this.store.map()?.provinces[at.province - 1];
    if (!at || !province) return null;
    const owners = this.session.owners();
    const shares = owners?.shares[at.province - 1] ?? [];
    const levels = this.session.buildingLevels();
    const infrastructure = levels?.[(at.province - 1) * BUILDING_COUNT + INFRASTRUCTURE] || province.infrastructure;
    // Dymek nie wychodzi poza ekran (szacunkowa wielkość).
    const x = Math.min(at.x + OFFSET, window.innerWidth - 270);
    const y = Math.min(at.y + OFFSET, window.innerHeight - 110);
    return {
      x,
      y,
      touch: !!at.touch,
      province: at.province,
      name: province.name || `Prowincja #${province.id}`,
      owners: shares
        .map(([nation, tiles]) => ({
          name: this.session.playerName(owners!.nations[nation - 1]),
          flag: flagDataUrl(nation, 21, 14),
          share: Math.round((tiles / province.area) * 100),
        }))
        .sort((a, b) => b.share - a.share),
      resource: province.resource,
      resourceName: RESOURCES[province.resource]?.name ?? '–',
      infrastructure,
      fortifications: levels?.[(at.province - 1) * BUILDING_COUNT + FORTIFICATIONS] ?? 0,
    };
  });
}
