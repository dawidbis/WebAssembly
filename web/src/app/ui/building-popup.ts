import { Component, computed, inject } from '@angular/core';

import type { BuildOption } from '../../generated/BuildOption';
import type { ProvinceReport } from '../../generated/ProvinceReport';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { BUILDING_ICONS, UNIVERSITY_ICON } from '../render/buildings';
import { FactoryPanel } from './factory-panel';
import { amount, full, short } from './format';
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
        @if (v.staff; as s) {
          <p class="staff" [class.short]="s.short" [title]="s.title">Obsada: {{ s.text }}</p>
        }
        <p class="effect">{{ v.effect }}</p>
        @if (v.university; as u) {
          <p class="uni" [title]="u.title">
            <svg class="icon small" viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="universityIcon" /></svg>
            {{ u.text }}
          </p>
        }
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
    .staff { color: #4fb3a5; }
    .uni { display: flex; align-items: center; gap: 6px; color: #b8c6d2; }
    .icon.small { width: 16px; height: 16px; }
    .staff.short { color: #e8cc72; }
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
    const owner = option ? this.ownerText(option) : '';
    const staff = option ? this.staff(option) : null;
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
      owner,
      staff,
      university: p.building === 'city' && report ? this.university(report) : null,
      effect: BUILDING_EFFECTS[p.building],
      extra,
      factory: p.building === 'factory',
      mine: !!report?.mine || !!option?.mine,
    };
  });

  protected readonly universityIcon = UNIVERSITY_ICON;

  /** Kto kontroluje budynek – właściciel jego kafla (rdzeń: `Game::controller`). */
  private ownerText(option: BuildOption): string {
    if (option.mine) return 'twój';
    const owners = this.session.owners();
    if (option.controller > 0 && owners) return `kontroluje: ${this.session.playerName(owners.nations[option.controller - 1])}`;
    return 'nikt nie kontroluje';
  }

  /** Obsada własnego budynku (pokrycie robotnikami wspólne dla państwa); null – obcy albo bez obsady. */
  private staff(option: BuildOption): { text: string; short: boolean; title: string } | null {
    const need = option.staff * option.level;
    if (!option.mine || need <= 0) return null;
    const coverage = this.session.myEconomy()?.staffCoverage ?? 1;
    return {
      text: `${short(need * coverage)}/${short(need)}`,
      short: coverage < 1,
      title: `Robotnicy w budynku: ${full(need * coverage)} z ${full(need)} potrzebnych (pokrycie ${Math.round(coverage * 100)}% – wspólne dla państwa)`,
    };
  }

  /** Uniwersytet w mieście: poziom i obsada (pokrycie robotnikami wspólne dla państwa). */
  private university(report: ProvinceReport): { text: string; title: string } {
    const u = report.options.find((o) => o.building === 'university');
    if (!u || u.level === 0) return { text: 'Uniwersytet: brak', title: 'Uniwersytet stoi w mieście – zbudujesz go w oknie prowincji' };
    const need = u.staff * u.level;
    const coverage = u.mine ? (this.session.myEconomy()?.staffCoverage ?? 1) : null;
    return {
      text: `Uniwersytet poz. ${u.level}` + (coverage !== null ? ` · obsada ${short(need * coverage)}/${short(need)}` : ''),
      title: 'Poziomy uniwersytetów sumują się w państwie – progi technologii (okno zapasów → Odblokowania)',
    };
  }

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
