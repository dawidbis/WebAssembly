import { Component, computed, inject, signal } from '@angular/core';

import type { AttackBlock } from '../../generated/AttackBlock';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { full, short } from './format';

/** Dlaczego nie można atakować. */
const BLOCKS: Record<AttackBlock, string> = {
  notPlaying: 'Atakować można po wyborze startu.',
  tunnel: 'Prowincja za tunelem – tunele w przyszłości (centrum dowodzenia 3).',
  owned: 'Cała prowincja jest już twoja.',
  noContact: 'Brak wspólnej granicy (przez rzekę – od centrum dowodzenia 2).',
  limit: 'Za dużo ataków naraz – więcej od wyższego centrum dowodzenia.',
  noTroops: 'Za mało wojska w kraju.',
};
/** Zapamiętany udział wojska w ataku (%) – tylko wygoda. */
const SHARE_KEY = 'mapa.attackShare';
const DEFAULT_SHARE = 50;

function loadShare(): number {
  try {
    const v = Number(localStorage.getItem(SHARE_KEY));
    return v >= 1 && v <= 100 ? v : DEFAULT_SHARE;
  } catch {
    return DEFAULT_SHARE;
  }
}

/**
 * Panel ataku na dole ekranu: pokazuje się po zaznaczeniu cudzej (albo nie w całości naszej) prowincji,
 * a przypięty – zawsze. Suwak wojska, kafle do zajęcia, szacunek kosztu, atak / dosłanie wojska,
 * trwający atak z postępem i odwołaniem. Zasady i koszty liczy rdzeń (`Game::attack_report`).
 */
@Component({
  selector: 'app-attack-panel',
  templateUrl: './attack-panel.html',
  styleUrls: ['./ui.css', './attack-panel.css'],
})
export class AttackPanel {
  private readonly session = inject(GameSession);
  protected readonly store = inject(MapStore);
  protected readonly share = signal(loadShare());

  protected readonly view = computed(() => {
    if (this.session.phase() !== 'playing' || !this.session.myNation()) return null;
    const id = this.store.selectedProvince();
    const r = this.session.provinceReport();
    const a = r && r.province === id ? r.attack : null;
    // Bez celu (albo cel w całości nasz, bez ataku) – tylko przypięty panel.
    const target = a && !(a.blocked === 'owned' && a.troops === null) ? a : null;
    if (!target && !this.store.attackPinned()) return null;
    const available = this.session.myEconomy()?.troops ?? 0;
    const send = Math.floor((available * this.share()) / 100);
    if (!target) return { target: null, send: short(send), sendTitle: `${full(send)} z ${full(available)} wojska w kraju` };
    const left = target.neutralTiles + target.enemyTiles;
    const estimate = target.neutralTiles * target.neutralCost + target.enemyTiles * target.enemyCost;
    const name = this.store.map()?.provinces[id - 1]?.name || `#${id}`;
    return {
      target: {
        name,
        active:
          target.troops !== null
            ? { troops: short(target.troops), taken: target.taken, progress: (target.taken / Math.max(1, target.taken + left)) * 100 }
            : null,
        blocked: target.blocked ? BLOCKS[target.blocked] : '',
        tiles: `${full(target.neutralTiles)} niczyich` + (target.enemyTiles > 0 ? ` · ${full(target.enemyTiles)} cudzych` : ''),
        costTitle:
          `Kafel niczyj: ${target.neutralCost} wojska na nizinie (las ×1,25, wyżyny ×1,5, wyżyny z lasem ×1,9)` +
          (target.enemyTiles > 0 ? `\nKafel cudzy: ok. ${target.enemyCost} wojska (gęstość wojska obrońcy i zaopatrzenie obu stron)` : ''),
        estimate: short(estimate),
        enough: send >= estimate,
        label: target.troops !== null ? 'Dośli wojsko' : target.enemyTiles > 0 ? 'Atakuj' : 'Zajmij',
      },
      send: short(send),
      sendTitle: `${full(send)} z ${full(available)} wojska w kraju`,
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

  protected togglePin(): void {
    const pinned = !this.store.attackPinned();
    this.store.attackPinned.set(pinned);
    try {
      localStorage.setItem('mapa.attackPinned', pinned ? '1' : '0');
    } catch {
      // Bez storage – tylko na tę sesję.
    }
  }

  protected attack(): void {
    this.session.attack(this.store.selectedProvince(), this.share());
  }

  protected cancel(): void {
    this.session.cancelAttack(this.store.selectedProvince());
  }
}
