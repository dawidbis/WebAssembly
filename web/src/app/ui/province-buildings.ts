import { Component, computed, inject, input } from '@angular/core';

import type { BuildBlock } from '../../generated/BuildBlock';
import type { Building } from '../../generated/Building';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { amount, full, short } from './format';

/** Nazwy budynków (wydobycie – nazwa z surowca prowincji, z raportu). */
const NAMES: Record<Building, string> = {
  extraction: 'Wydobycie',
  factory: 'Fabryka',
  city: 'Miasto',
  university: 'Uniwersytet',
  fortifications: 'Umocnienia',
  port: 'Port',
  infrastructure: 'Infrastruktura',
};

/** Co daje poziom budynku (podpowiedź). */
const EFFECTS: Record<Building, string> = {
  extraction: '+1,0 surowca prowincji na dzień za poziom (× obsada robotnikami)',
  factory: 'Zaopatrzenie z surowców – produkcja w kolejnym etapie',
  city: '+25 000 do limitu populacji państwa za poziom',
  university: 'Poziomy w państwie sumują się w progi technologii (kolejny etap)',
  fortifications: 'Większe straty atakującego',
  port: 'Handel morski (kolejny etap)',
  infrastructure: 'Punkty na budynki w prowincji: +50 za poziom',
};

/** Dlaczego nie można budować – podpowiedź przy wyłączonym przycisku. */
const BLOCKS: Record<BuildBlock, string> = {
  notPlaying: 'Budować można dopiero po wyborze startu',
  notOwner: 'Prowincja nie jest twoja – budujesz tam, gdzie masz ponad 50% kafli',
  maxLevel: 'Najwyższy poziom',
  busy: 'W prowincji trwa inna budowa',
  noPoints: 'Brak wolnych punktów infrastruktury – rozbuduj infrastrukturę',
  locked: 'Wymaga centrum dowodzenia na poziomie 3',
  notCoastal: 'Port tylko w prowincji nad morzem',
  noGold: 'Za mało złota',
  noMaterials: 'Za mało materiałów inżynieryjnych',
};

/**
 * Budynki prowincji w ramce prowincji (tylko w grze): punkty infrastruktury, trwająca budowa z postępem
 * i anulowaniem, poziomy budynków i budowa kolejnego poziomu (tylko zaznaczona własna prowincja).
 * Zasady i koszty liczy rdzeń (`Game::province_report`) – tu tylko wyświetlanie.
 */
@Component({
  selector: 'app-province-buildings',
  templateUrl: './province-buildings.html',
  styleUrls: ['./ui.css', './province-buildings.css'],
})
export class ProvinceBuildings {
  private readonly session = inject(GameSession);
  private readonly store = inject(MapStore);
  /** Prowincja w ramce i czy jest zaznaczona (przyciski tylko przy zaznaczonej – kursor schodzi z mapy). */
  readonly province = input.required<number>();
  readonly selected = input(false);

  protected readonly view = computed(() => {
    const r = this.session.provinceReport();
    if (!r || r.province !== this.province()) return null;
    const c = r.construction;
    const coastal = this.store.map()?.provinces[r.province - 1]?.coastal ?? false;
    const name = (b: Building) => (b === 'extraction' ? r.extraction : NAMES[b]);
    return {
      mine: r.mine,
      canAct: r.mine && this.selected(),
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
        // Port poza wybrzeżem – bez wiersza.
        .filter((o) => o.building !== 'port' || coastal || o.level > 0)
        .map((o) => {
          const max = o.level >= o.maxLevel;
          const cost = [`${full(o.gold)} złota`, o.materials > 0 ? `${amount(o.materials)} mat. inż.` : '', `${o.days} dni`]
            .filter(Boolean)
            .join(' · ');
          const needs = [o.points > 0 ? `${o.points} pkt infrastruktury` : '', o.staff > 0 ? `obsada ${full(o.staff)} robotników` : '']
            .filter(Boolean)
            .join(', ');
          return {
            building: o.building,
            name: name(o.building),
            level: o.level,
            maxLevel: o.maxLevel,
            gold: short(o.gold),
            blocked: o.blocked,
            title: max
              ? `${EFFECTS[o.building]}. Najwyższy poziom.`
              : `${EFFECTS[o.building]}.\nPoziom ${o.level + 1}: ${cost}${needs ? `\nNa poziom: ${needs}` : ''}` +
                (o.blocked ? `\n${BLOCKS[o.blocked]}` : ''),
          };
        }),
    };
  });

  protected build(building: Building): void {
    this.session.build(this.province(), building);
  }

  protected cancel(): void {
    this.session.cancelBuild(this.province());
  }
}
