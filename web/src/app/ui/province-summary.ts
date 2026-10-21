import { Component, computed, inject, input } from '@angular/core';

import type { CapitalBlock } from '../../generated/CapitalBlock';
import type { CapitalReport } from '../../generated/CapitalReport';
import type { StanceReport } from '../../generated/StanceReport';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { RESOURCES } from '../render/resources';
import { BIOMES } from '../render/terrain';
import { amount, full, short } from './format';
import { GoodsIcon } from './goods-icon';
import { BotIcon } from './bot-icon';

/** Numer złota w tabeli surowców – poza rynkiem (wydobycie idzie prosto do skarbca). */
const GOLD = RESOURCES.findIndex((r) => r.name === 'Złoto');
/** Limit populacji: osoby na kafel (`PEOPLE_PER_TILE` w rdzeniu); miasto – reszta `populationCap` (poziomy rosnąco). */
const PEOPLE_PER_TILE = 10;

/** Dlaczego nie można przenieść stolicy. */
const CAPITAL_BLOCKS: Record<CapitalBlock, string> = {
  notPlaying: 'Stolicę można przenieść w grze właściwej.',
  notOwner: 'Stolica tylko w prowincji, w której masz ponad połowę kafli i kafel miasta.',
  alreadyCapital: 'To twoja stolica.',
  bridgehead: 'Stolica nie może stać w przyczółku odciętym od kraju.',
  busy: 'W stolicy trwa rozbudowa centrum dowodzenia.',
  cooldown: 'Stolicę można przenosić najwyżej raz na rok gry.',
  noGold: 'Za mało złota.',
};

/** Stolica w oknie prowincji: koszt przeniesienia i podpowiedź (przerwa po ostatniej zmianie, blokada). */
function capitalView(c: CapitalReport): { here: boolean; cost: string; blocked: boolean; title: string } {
  const wait = c.cooldownDays > 0 ? ` Następna zmiana za ${Math.ceil(c.cooldownDays)} d.` : '';
  const why = c.blocked && c.blocked !== 'cooldown' ? ` ${CAPITAL_BLOCKS[c.blocked]}` : '';
  return {
    here: c.here,
    cost: short(c.cost),
    blocked: !!c.blocked,
    title: `Przeniesienie stolicy razem z centrum dowodzenia (poziom zostaje) – ${full(c.cost)} złota; najwyżej raz na rok gry, każda kolejna zmiana droższa.${wait}${why}`,
  };
}

/**
 * Pokój (zielony) albo wojna (czerwony miecz) z innym udziałowcem w tej prowincji. Gdy on wypowiedział nam
 * wojnę – miecz bez możliwości zmiany. Bez raportu (oglądający bez państwa) – brak przycisku.
 */
function stanceView(
  s: StanceReport | undefined,
  name: string,
): { player: number; war: boolean; locked: boolean; title: string } | null {
  if (!s) return null;
  const locked = s.attacked && !s.declared;
  const title = locked
    ? `${name} zaatakował nas w tej prowincji – wojna, dopóki jej nie odwoła.`
    : s.war
      ? `Wojna z ${name} w tej prowincji: fronty zajmują sobie kafle. Kliknij – pokój.`
      : `Pokój z ${name} w tej prowincji: fronty nie zajmują sobie kafli. Kliknij – wojna.`;
  return { player: s.player, war: s.war, locked, title };
}

/**
 * Najważniejsze dane prowincji z punktu widzenia gry (na górze okna prowincji): surowiec, wydobycie
 * i cena, właściciel, wkład w limit populacji, infrastruktura (obecna i bazowa), wielkość i teren.
 * Liczby z rdzenia (`Game::province_report`), właściciele z `owners`.
 */
@Component({
  selector: 'app-province-summary',
  imports: [GoodsIcon, BotIcon],
  template: `
    @if (view(); as v) {
      <section class="summary" aria-label="Prowincja">
        <div class="resource">
          <app-goods-icon [resource]="v.resource" [size]="30" />
          <div class="what">
            <strong>{{ v.resourceName }}</strong>
            <span class="sub">{{ v.extraction }} · {{ v.yield }}</span>
          </div>
          <div class="price" [title]="v.priceTitle">
            <span class="sub">Cena rynkowa</span>
            <strong>{{ v.price }}</strong>
          </div>
        </div>
        <dl>
          <dt>Udział w prowincji</dt>
          <dd class="owners" title="Udział państw w kaflach prowincji; budynek kontroluje państwo, które ma kafel budynku">
            @for (o of v.owners; track o.name) {
              <span class="owner">
                <img [src]="o.flag" alt="" width="18" height="12" />{{ o.name }}@if (o.bot) {<app-bot-icon />} <b>{{ o.share }}%</b>
                @if (o.stance; as s) {
                  <button
                    type="button"
                    class="stance"
                    [class.war]="s.war"
                    [disabled]="s.locked"
                    [title]="s.title"
                    [attr.aria-label]="s.war ? 'Wojna w tej prowincji' : 'Pokój w tej prowincji'"
                    (click)="toggleStance(s.player, !s.war)"
                  >
                    @if (s.war) {
                      <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M5 19 19 5M15 5h4v4M3 21l3-3M7 15l2 2M5 13l6 6" /></svg>
                    } @else {
                      <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true"><path d="M12 21c-5-3-8-7-8-11a4.5 4.5 0 0 1 8-3 4.5 4.5 0 0 1 8 3c0 4-3 8-8 11Z" /></svg>
                    }
                  </button>
                }
              </span>
            } @empty {
              <span class="nobody">niczyja</span>
            }
          </dd>
          <dt>Limit populacji</dt>
          <dd [title]="v.capTitle">{{ v.cap }}</dd>
          <dt>Infrastruktura</dt>
          <dd title="Obecny poziom infrastruktury i poziom bazowy (z cech prowincji: biom, wyżyny i góry, rzeka i morze)">
            poz. {{ v.infrastructure }} <span class="sub">(bazowo {{ v.baseInfrastructure }})</span>
          </dd>
          <dt>Teren</dt>
          <dd>{{ v.terrain }}</dd>
          @if (v.capital; as c) {
            <dt>Stolica</dt>
            <dd>
              @if (c.here) {
                <span class="capital" title="Utrata kafla centrum dowodzenia (miasta) – aneksja całego państwa">★ tutaj</span>
              } @else {
                <button type="button" class="move" [disabled]="!!c.blocked" [title]="c.title" aria-label="Przenieś stolicę tutaj" (click)="moveCapital()">
                  <span class="star">★ tutaj</span> <span class="cost">{{ c.cost }} ●</span>
                </button>
              }
            </dd>
          }
        </dl>
      </section>
    }
  `,
  styles: `
    .summary { display: grid; gap: 8px; padding-bottom: 8px; border-bottom: 1px solid #243847; }
    .resource { display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 10px; }
    .what, .price { display: grid; gap: 1px; }
    .what strong { font-size: 14px; }
    .price { text-align: right; }
    .price strong { color: #f2d47a; font-variant-numeric: tabular-nums; }
    .sub { color: #8ea3b4; font-size: 11px; }
    dl { display: grid; grid-template-columns: auto 1fr; gap: 3px 12px; margin: 0; font-size: 12px; }
    dt { color: #8ea3b4; }
    dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
    .owners { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 4px 10px; }
    .owner { display: inline-flex; align-items: center; gap: 5px; }
    .owner img { border-radius: 2px; }
    .owner b { color: #d8e2ea; font-weight: 600; }
    .stance { display: inline-grid; place-items: center; width: 20px; height: 18px; padding: 0; border-radius: 4px; border: 1px solid #2f7a4a; background: #173524; }
    .stance svg { fill: none; stroke: #5fd08a; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
    .stance.war { border-color: #8f2f2f; background: #3a1717; }
    .stance.war svg { stroke: #ff6b6b; }
    .stance:disabled { opacity: 1; cursor: not-allowed; }
    .nobody { color: #8ea3b4; }
    .capital { color: #f2d47a; }
    /* Przeniesienie stolicy: jak „★ tutaj”, tylko wyszarzone, obok koszt w złocie. */
    .move { padding: 0 4px; font: inherit; color: inherit; background: none; border: 1px solid transparent; border-radius: 4px; cursor: pointer; }
    .move .star { color: #7f8c97; }
    .move .cost { margin-left: 4px; color: #f2d47a; }
    .move:hover:not(:disabled) { border-color: #5c4a24; }
    .move:hover:not(:disabled) .star { color: #f2d47a; }
    .move:disabled { cursor: not-allowed; opacity: 0.6; }
  `,
})
export class ProvinceSummary {
  private readonly session = inject(GameSession);
  private readonly store = inject(MapStore);

  protected toggleStance(player: number, war: boolean): void {
    const id = this.province();
    if (id > 0) this.session.setStance(id, player, war);
  }

  protected moveCapital(): void {
    const id = this.province();
    if (id > 0) this.session.moveCapital(id);
  }
  readonly province = input.required<number>();

  protected readonly view = computed(() => {
    const id = this.province();
    const r = this.session.provinceReport();
    const province = this.store.map()?.provinces[id - 1];
    if (!r || r.province !== id || !province) return null;
    const owners = this.session.owners();
    const shares = owners?.shares[id - 1] ?? [];
    const infra = r.options.find((o) => o.building === 'infrastructure')?.level ?? 0;
    const gold = province.resource === GOLD;
    const owned = r.populationCap > 0;
    const water = [province.coastal && 'nad morzem', province.river && 'rzeka', province.lake && 'jezioro']
      .filter(Boolean)
      .join(', ');
    const unit = gold ? ' złota' : '';
    const city = r.capCity > 0 ? ` + miasto (poziom ${r.capCity}): ${full(r.populationCap - r.capTiles * PEOPLE_PER_TILE)}` : '';
    return {
      resource: province.resource,
      resourceName: RESOURCES[province.resource]?.name ?? '–',
      extraction: r.extraction,
      yield: owned ? `+${amount(r.yieldPerDay)}${unit}/d` : 'bez wydobycia (niczyja)',
      price: gold ? 'skarbiec' : `●${amount(r.price)}`,
      priceTitle: gold
        ? 'Złoto poza rynkiem – wydobycie trafia prosto do skarbca'
        : 'Cena za jednostkę w złocie (rynku jeszcze nie ma – cena bazowa)',
      owners: shares
        .map(([nation, n]) => {
          const player = owners!.nations[nation - 1];
          const name = this.session.playerName(player);
          return {
            name,
            bot: this.session.isBot(player),
            flag: flagDataUrl(nation),
            share: Math.round((n / province.area) * 100),
            stance: stanceView(r.stances.find((s) => s.player === player), name),
          };
        })
        .sort((a, b) => b.share - a.share),
      cap: owned ? `+${short(r.populationCap)}` : '–',
      capTitle: owned
        ? `Kafle (twoje, a gdy ich nie masz – największego udziałowca): ${full(r.capTiles)} × ${PEOPLE_PER_TILE}${city}`
        : 'Niczyja prowincja nie podnosi niczyjego limitu',
      capital: r.capital ? capitalView(r.capital) : null,
      infrastructure: infra,
      baseInfrastructure: r.baseInfrastructure,
      terrain: [`${full(province.area)} kafli`, BIOMES[province.biome]?.name, province.mountains && 'góry', water]
        .filter(Boolean)
        .join(' · '),
    };
  });
}
