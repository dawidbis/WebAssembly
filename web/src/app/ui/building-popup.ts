import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { BUILDING_ICONS } from '../render/buildings';
import { FactoryPanel } from './factory-panel';
import { amount } from './format';
import { BUILDING_EFFECTS, BUILDING_NAMES } from './province-buildings';

/** Szerokość okienka (px) – do przycięcia pozycji przy krawędzi ekranu. */
const WIDTH = 330;

/**
 * Okienko budynku z mapy (kliknięcie ikony): nazwa i poziom, co daje; fabryka – panel produkcji
 * (produkt, receptura z wyborem zamienników); własna prowincja – przejście do okna budynków.
 * Zamyka je ×, Esc albo kliknięcie obok.
 */
@Component({
  selector: 'app-building-popup',
  imports: [FactoryPanel],
  host: { '(document:pointerdown)': 'onOutside($event)', '(document:keydown.escape)': 'close()' },
  template: `
    @if (view(); as v) {
      <section class="frame popup" [style.left.px]="v.x" [style.top.px]="v.y" [attr.aria-label]="v.name">
        <header class="head">
          <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="v.icon" /></svg>
          <span class="title">{{ v.name }} <small>poz. {{ v.level }}</small></span>
          <button type="button" class="close" (click)="close()" title="Zamknij" aria-label="Zamknij">×</button>
        </header>
        <p class="where">{{ v.province }}@if (v.owner) { · {{ v.owner }}}</p>
        <p class="effect">{{ v.effect }}</p>
        @if (v.extra) {
          <p class="extra">{{ v.extra }}</p>
        }
        @if (v.factory) {
          <app-factory-panel [province]="v.id" />
        }
        @if (v.mine) {
          <button type="button" class="open" (click)="openBuildings(v.id)">Budynki prowincji</button>
        }
      </section>
    }
  `,
  styles: `
    .popup { position: fixed; z-index: 15; display: grid; gap: 6px; width: min(${WIDTH}px, calc(100vw - 16px)); padding: 8px 10px; font-size: 12px; }
    .head { display: flex; align-items: center; gap: 8px; }
    .icon { width: 20px; height: 20px; fill: none; stroke: #d8e2ea; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
    .title { flex: 1; font-weight: 600; font-size: 13px; }
    .title small { color: #8ea3b4; font-weight: 400; }
    .close { width: 22px; height: 22px; padding: 0; line-height: 1; }
    p { margin: 0; }
    .where { color: #8ea3b4; }
    .effect { color: #b8c6d2; }
    .extra { color: #8fd6ae; }
    .open { justify-self: start; padding: 2px 10px; font-size: 12px; }
  `,
  styleUrls: ['./ui.css'],
})
export class BuildingPopup {
  private readonly store = inject(MapStore);
  private readonly session = inject(GameSession);

  protected readonly view = computed(() => {
    const p = this.store.buildingPopup();
    if (!p) return null;
    const r = this.session.provinceReport();
    const report = r && r.province === p.province ? r : null;
    const option = report?.options.find((o) => o.building === p.building);
    const province = this.store.map()?.provinces[p.province - 1];
    const owners = this.session.owners();
    const main = owners?.shares[p.province - 1]?.reduce((a, b) => (b[1] > a[1] ? b : a), [0, 0] as [number, number]);
    const owner = main && main[0] > 0 ? this.session.playerName(owners!.nations[main[0] - 1]) : '';
    const extra =
      p.building === 'extraction' && report && report.yieldPerDay > 0
        ? `Wydobycie prowincji: +${amount(report.yieldPerDay)}/d`
        : '';
    // Przy krawędzi ekranu okienko przesuwa się do środka.
    const x = Math.max(8, Math.min(p.x + 14, innerWidth - WIDTH - 8));
    const y = Math.max(8, Math.min(p.y - 20, innerHeight - 320));
    return {
      id: p.province,
      x,
      y,
      icon: BUILDING_ICONS[p.building],
      name: p.building === 'extraction' && report ? report.extraction : BUILDING_NAMES[p.building],
      level: option?.level ?? '…',
      province: province?.name || `#${p.province}`,
      owner: report?.mine ? '' : owner,
      effect: BUILDING_EFFECTS[p.building],
      extra,
      factory: p.building === 'factory',
      mine: !!report?.mine,
    };
  });

  protected openBuildings(province: number): void {
    this.close();
    this.store.buildingsClosedFor.set(0);
    this.store.selectedProvince.set(province);
  }

  protected close(): void {
    this.store.buildingPopup.set(null);
  }

  protected onOutside(event: PointerEvent): void {
    // Klik w mapę obsługuje renderer (inna ikona podmienia okienko) – zamykamy tylko przy klikach w UI obok.
    const target = event.target as HTMLElement;
    if (this.store.buildingPopup() && !target.closest('app-building-popup') && target.tagName !== 'CANVAS') this.close();
  }
}
