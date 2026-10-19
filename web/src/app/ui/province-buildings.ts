import { Component, computed, inject, input } from '@angular/core';

import type { BuildBlock } from '../../generated/BuildBlock';
import type { BuildOption } from '../../generated/BuildOption';
import type { Building } from '../../generated/Building';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { amount, full, short } from './format';
import { FactoryPanel } from './factory-panel';

/** Nazwy budynków (wydobycie – nazwa z surowca prowincji, z raportu). */
export const BUILDING_NAMES: Record<Building, string> = {
  extraction: 'Wydobycie',
  factory: 'Fabryka',
  city: 'Miasto',
  university: 'Uniwersytet',
  fortifications: 'Umocnienia',
  port: 'Port',
  infrastructure: 'Infrastruktura',
  commandCenter: 'Centrum dowodzenia',
};

/** Ikony budynków (24 × 24, obrys). */
const ICONS: Record<Building, string> = {
  extraction: 'M4 20h16M7 20V10l5-5 5 5v10M10 20v-5h4v5',
  factory: 'M3 20V10l5 3V10l5 3V6h3v14zM17 6h3v14',
  city: 'M3 20V9h5v11M8 20V4h7v16M15 20v-8h6v8M10 7h3M10 10h3M10 13h3',
  university: 'M2 9l10-5 10 5-10 5zM6 11v5c3 2 9 2 12 0v-5M22 9v6',
  fortifications: 'M4 20V8h3v3h3V8h4v3h3V8h3v12z',
  port: 'M12 3v15M8 7h8M5 13c0 4 3 6 7 6s7-2 7-6M3 14l2-2 2 2M17 14l2-2 2 2',
  infrastructure: 'M7 20 10 4M17 20 14 4M12 6v2M12 11v2M12 16v2',
  commandCenter: 'M5 21V4M5 4h12l-3 4 3 4H5',
};

/** Co daje poziom budynku (podpowiedź). */
export const BUILDING_EFFECTS: Record<Building, string> = {
  extraction: '+1,0 surowca prowincji na dzień za poziom (× obsada robotnikami)',
  factory: '1,0 sztuki zaopatrzenia na dzień za poziom (× obsada), jeden rodzaj naraz – z surowców puli',
  city: '+25 000 do limitu populacji państwa za poziom',
  university: 'Poziomy w państwie sumują się w progi technologii (okno zapasów → Odblokowania)',
  fortifications: 'Obrona kafli prowincji +50% za poziom (koszt kafla i tempo frontu atakującego) – dla tego, kto ma ponad połowę prowincji',
  port: 'Desanty morskie (poziom: do 5 / 15 / 40 tys. wojska na desant) i handel morski (przepustowość)',
  infrastructure: 'Punkty na budynki w prowincji: +50 za poziom',
  commandCenter: 'Odblokowania: zaopatrzenie, liczba ataków, przeprawy, umocnienia (okno zapasów → Odblokowania)',
};

/** Dlaczego nie można budować – podpowiedź przy wyłączonym przycisku. */
const BLOCKS: Record<BuildBlock, string> = {
  notPlaying: 'Budować można dopiero po wyborze startu',
  notOwner: 'Prowincja nie jest twoja – budujesz tam, gdzie masz ponad 50% kafli',
  maxLevel: 'Najwyższy poziom',
  busy: 'W prowincji trwa inna budowa',
  noPoints: 'Brak wolnych punktów infrastruktury – rozbuduj infrastrukturę',
  locked: 'Wymaga wyższego centrum dowodzenia (umocnienia: poziom 1 od CD 2, 2 od CD 3, 3 od CD 5)',
  notCapital: 'Centrum dowodzenia tylko w stolicy',
  notCoastal: 'Port tylko w prowincji nad morzem',
  noCity: 'Uniwersytet stoi w mieście – najpierw zbuduj miasto',
  noGold: 'Za mało złota',
  noMaterials: 'Za mało materiałów inżynieryjnych',
};

/** Obsada własnego budynku (robotnicy przy pokryciu `coverage`); null – obcy, bez obsady albo niezbudowany. */
function staffOf(o: BuildOption, coverage: number): { have: number; need: number } | null {
  return o.mine && o.staff > 0 && o.level > 0 ? { have: o.staff * o.level * coverage, need: o.staff * o.level } : null;
}

/** Podpowiedź wiersza: działanie, koszt i wymagania następnego poziomu, obsada, blokada. */
function rowTitle(o: BuildOption, coverage: number): string {
  if (o.level >= o.maxLevel) return `${BUILDING_EFFECTS[o.building]}. Najwyższy poziom.`;
  const cost = [`${full(o.gold)} złota`, o.materials > 0 ? `${amount(o.materials)} mat. inż.` : '', `${o.days} dni`]
    .filter(Boolean)
    .join(' · ');
  const needs = [o.points > 0 ? `${o.points} pkt infrastruktury` : '', o.staff > 0 ? `obsada ${full(o.staff)} robotników` : '']
    .filter(Boolean)
    .join(', ');
  const staff = staffOf(o, coverage);
  const lines = [
    o.mine || o.controller === 0 ? '' : 'Kontroluje go inne państwo (kafel budynku).',
    `${BUILDING_EFFECTS[o.building]}.`,
    `Poziom ${o.level + 1}: ${cost}`,
    needs ? `Na poziom: ${needs}` : '',
    staff ? `Obsada: ${full(staff.have)} z ${full(staff.need)} robotników` : '',
    o.blocked ? BLOCKS[o.blocked] : '',
  ];
  return lines.filter(Boolean).join('\n');
}

/** Wiersz budynku w oknie prowincji. */
function buildingRow(o: BuildOption, name: string, coverage: number) {
  const staff = staffOf(o, coverage);
  return {
    building: o.building,
    name,
    level: o.level,
    maxLevel: o.maxLevel,
    pips: Array.from({ length: o.maxLevel }, (_, i) => i < o.level),
    gold: short(o.gold),
    materials: o.materials > 0 ? amount(o.materials) : '',
    blocked: o.blocked,
    max: o.level >= o.maxLevel,
    mine: o.mine,
    staff: staff ? { text: `${short(staff.have)}/${short(staff.need)}`, short: coverage < 1 } : null,
    title: rowTitle(o, coverage),
  };
}

/**
 * Budynki własnej prowincji (w oknie budynków): punkty infrastruktury, trwająca budowa z postępem
 * i anulowaniem, poziomy budynków z budową kolejnego poziomu, fabryka – wybór produktu i receptura
 * jako „funkcja” z ikonami i zapasami. Zasady i koszty liczy rdzeń (`Game::province_report`).
 */
@Component({
  selector: 'app-province-buildings',
  imports: [FactoryPanel],
  templateUrl: './province-buildings.html',
  styleUrls: ['./ui.css', './province-buildings.css'],
})
export class ProvinceBuildings {
  private readonly session = inject(GameSession);
  private readonly store = inject(MapStore);
  readonly province = input.required<number>();
  protected readonly icons = ICONS;

  protected readonly view = computed(() => {
    const r = this.session.provinceReport();
    if (!r || r.province !== this.province()) return null;
    const c = r.construction;
    const coastal = this.store.map()?.provinces[r.province - 1]?.coastal ?? false;
    const name = (b: Building) => (b === 'extraction' ? r.extraction : BUILDING_NAMES[b]);
    // Obsada: pokrycie robotnikami jest wspólne dla państwa (`staffCoverage`).
    const coverage = this.session.myEconomy()?.staffCoverage ?? 1;
    const mineOf = (b: Building) => r.options.find((o) => o.building === b)?.mine ?? false;
    return {
      mine: r.mine,
      canCancel: !!c && mineOf(c.building),
      points: r.points,
      used: r.usedPoints,
      fill: r.points > 0 ? Math.min(100, (r.usedPoints / r.points) * 100) : 0,
      yieldPerDay: r.yieldPerDay > 0 ? `+${amount(r.yieldPerDay)}/d` : '',
      construction: c && {
        name: `${name(c.building)} ${c.level}`,
        progress: (c.done / c.total) * 100,
        days: Math.ceil((c.total - c.done) / 10),
        paused: c.paused,
      },
      rows: r.options
        // Port poza wybrzeżem i centrum dowodzenia poza stolicą – bez wiersza.
        .filter((o) => (o.building !== 'port' || coastal || o.level > 0) && (o.building !== 'commandCenter' || o.level > 0))
        .map((o) => buildingRow(o, name(o.building), coverage)),
    };
  });

  protected build(building: Building): void {
    this.session.build(this.province(), building);
  }

  protected cancel(): void {
    this.session.cancelBuild(this.province());
  }
}
