import { Component, computed, inject, input, signal } from '@angular/core';

import type { AttackBlock } from '../../generated/AttackBlock';
import { GameSession } from '../game/game-session';
import { full, short } from './format';

/** Dlaczego nie można atakować (pokazywane przy zaznaczonej prowincji). */
const BLOCKS: Record<AttackBlock, string> = {
  notPlaying: 'Atakować można po wyborze startu.',
  tunnel: 'Prowincja za tunelem – tunele od centrum dowodzenia 3.',
  owned: '',
  noContact: 'Brak wspólnej granicy (przez rzekę – od centrum dowodzenia 2).',
  limit: 'Trwa inny atak – więcej naraz od centrum dowodzenia 2.',
  noTroops: 'Za mało wojska w kraju.',
};
/** Domyślny i zapamiętany udział wojska w ataku (%) – tylko wygoda. */
const SHARE_KEY = 'mapa.attackShare';
const DEFAULT_SHARE = 50;

function loadShare(): number {
  try {
    const v = Number(localStorage.getItem(SHARE_KEY));
    return v >= 10 && v <= 100 ? v : DEFAULT_SHARE;
  } catch {
    return DEFAULT_SHARE;
  }
}

/**
 * Atak na prowincję w ramce prowincji (tylko w grze, prowincja nie w całości nasza): trwający atak
 * (wojsko, zajęte kafle, odwołanie), a przy zaznaczonej prowincji – suwak wojska, szacunek kosztu
 * i przycisk ataku. Zasady i koszty liczy rdzeń (`Game::attack_report`).
 */
@Component({
  selector: 'app-province-attack',
  templateUrl: './province-attack.html',
  styleUrls: ['./ui.css', './province-attack.css'],
})
export class ProvinceAttack {
  private readonly session = inject(GameSession);
  readonly province = input.required<number>();
  readonly selected = input(false);
  /** Ile procent wojska z kraju wysłać. */
  protected readonly share = signal(loadShare());

  protected readonly view = computed(() => {
    const r = this.session.provinceReport();
    if (!r || r.province !== this.province()) return null;
    const a = r.attack;
    if (a.blocked === 'owned' && a.troops === null) return null;
    const send = Math.floor((a.available * this.share()) / 100);
    const left = a.neutralTiles + a.enemyTiles;
    const estimate = a.neutralTiles * a.neutralCost + a.enemyTiles * a.enemyCost;
    return {
      active: a.troops !== null ? { troops: short(a.troops), taken: a.taken, progress: (a.taken / Math.max(1, a.taken + left)) * 100 } : null,
      canAct: this.selected() && a.blocked === null,
      blocked: this.selected() && a.blocked ? BLOCKS[a.blocked] : '',
      send: short(send),
      sendTitle: `${full(send)} z ${full(a.available)} wojska w kraju`,
      tiles: `${full(a.neutralTiles)} niczyich` + (a.enemyTiles > 0 ? ` · ${full(a.enemyTiles)} cudzych` : ''),
      costTitle:
        `Kafel niczyj: ${a.neutralCost} wojska (wyżyny ×1,5)` +
        (a.enemyTiles > 0 ? `\nKafel cudzy: ok. ${a.enemyCost} wojska (gęstość wojska obrońcy i zaopatrzenie obu stron)` : ''),
      estimate: short(estimate),
      enough: send >= estimate,
      label: a.troops !== null ? 'Dośli wojsko' : a.enemyTiles > 0 ? 'Atakuj' : 'Zajmij',
    };
  });

  protected setShare(event: Event): void {
    const v = Number((event.target as HTMLInputElement).value);
    this.share.set(v);
    try {
      localStorage.setItem(SHARE_KEY, String(v));
    } catch {
      // Bez storage – tylko na tę sesję.
    }
  }

  protected attack(): void {
    this.session.attack(this.province(), this.share());
  }

  protected cancel(): void {
    this.session.cancelAttack(this.province());
  }
}
