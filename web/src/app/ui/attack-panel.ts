import { Component, computed, inject } from '@angular/core';

import type { AttackBlock } from '../../generated/AttackBlock';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { full, short } from './format';
import { COMMAND_LIMITS } from './unlocks';

/** Dlaczego nie można atakować. */
export const ATTACK_BLOCKS: Record<AttackBlock, string> = {
  notPlaying: 'Atakować można po wyborze startu.',
  tunnel: 'Prowincja za tunelem – budowa tuneli w przyszłości.',
  owned: 'Cała prowincja jest już twoja.',
  noContact: 'Brak styku – ani granicy, ani rzeki, jeziora czy morza nad twoim brzegiem.',
  limit: 'Za dużo frontów naraz – więcej od wyższego centrum dowodzenia.',
  provinces: 'Za dużo prowincji na jednym froncie – więcej od wyższego centrum dowodzenia.',
  landing: 'Desant morski – od centrum dowodzenia 2 (jeden naraz), od 3 bez limitu.',
  otherFront: 'Zaznaczone prowincje są na różnych frontach.',
  noTroops: 'Za mało wojska w kraju.',
};

/**
 * Panel ataku na dole ekranu: pokazuje się po zaznaczeniu cudzej (albo nie w całości naszej) prowincji
 * – z Ctrl można zaznaczyć kilka na jeden front – a przypięty zawsze. Suwak wojska (ten sam co przy
 * dwukliku), kafle do zajęcia, szacunek kosztu, atak / dosłanie wojska, trwający front z postępem
 * i odwołaniem. Zasady liczy rdzeń (`Game::attack_report`, `Game::check_attack`).
 */
@Component({
  selector: 'app-attack-panel',
  templateUrl: './attack-panel.html',
  styleUrls: ['./ui.css', './attack-panel.css'],
})
export class AttackPanel {
  private readonly session = inject(GameSession);
  protected readonly store = inject(MapStore);

  protected readonly view = computed(() => {
    if (this.session.phase() !== 'playing' || !this.session.myNation()) return null;
    const id = this.store.selectedProvince();
    const targets = this.store.attackTargets();
    const r = this.session.provinceReport();
    const a = r && r.province === id ? r.attack : null;
    const many = targets.length > 1;
    // Bez celu (albo cel w całości nasz, bez frontu) – tylko przypięty panel.
    const target = many || (a && !(a.blocked === 'owned' && a.troops === null)) ? a : null;
    const available = this.session.myEconomy()?.troops ?? 0;
    const share = this.store.attackShare();
    const send = Math.floor((available * share) / 100);
    const base = { send: short(send), sendTitle: `${full(send)} z ${full(available)} wojska w kraju`, share };
    if (!target && !many) return this.store.attackPinned() ? { ...base, target: null } : null;
    const names = this.store.map()?.provinces;
    const name = (p: number) => names?.[p - 1]?.name || `#${p}`;
    const command = this.session.myEconomy()?.commandLevel ?? 1;
    const limit = COMMAND_LIMITS[Math.min(6, Math.max(1, command))].provinces;
    // Kilka celów: sprawdzenie zbioru z workera (`attackCheck`), szacunek tylko dla zaznaczonej.
    const check = this.session.attackCheck();
    const blocked = many ? (check?.blocked ?? null) : (target?.blocked ?? null);
    const left = target ? target.neutralTiles + target.enemyTiles : 0;
    const estimate = target ? target.neutralTiles * target.neutralCost + target.enemyTiles * target.enemyCost : 0;
    const active = target && target.troops !== null ? target : null;
    return {
      ...base,
      target: {
        name: many ? targets.map(name).join(', ') : name(id),
        count: many ? `${targets.length}/${limit === Infinity ? '∞' : limit}` : '',
        front: active && active.front.length > 1 ? active.front.map(name).join(', ') : '',
        active: active
          ? { troops: short(active.troops ?? 0), taken: active.taken, progress: (active.taken / Math.max(1, active.taken + left)) * 100 }
          : null,
        blocked: blocked ? ATTACK_BLOCKS[blocked] : '',
        landing: many ? !!check?.landing : false,
        tiles: target
          ? `${full(target.neutralTiles)} niczyich` + (target.enemyTiles > 0 ? ` · ${full(target.enemyTiles)} cudzych` : '')
          : '',
        costTitle: target
          ? `Kafel niczyj: ok. ${target.neutralCost} wojska na nizinie – walka z armią tubylczą (zostało ${full(target.natives)});` +
            ' las ×1,25, wyżyny ×1,5, wyżyny z lasem ×1,9, desant ×1,5' +
            (target.enemyTiles > 0 ? `\nKafel cudzy: ok. ${target.enemyCost} wojska (gęstość wojska obrońcy i zaopatrzenie obu stron)` : '')
          : '',
        estimate: target ? short(estimate) : '',
        enough: send >= estimate,
        label: active ? 'Dośli wojsko' : target && target.enemyTiles > 0 ? 'Atakuj' : 'Zajmij',
      },
    };
  });

  protected setShare(event: Event): void {
    this.store.setAttackShare(Number((event.target as HTMLInputElement).value));
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
    const targets = this.store.attackTargets();
    this.session.attack(targets.length > 0 ? targets : [this.store.selectedProvince()], this.store.attackShare());
  }

  protected cancel(): void {
    this.session.cancelAttack(this.store.selectedProvince());
  }
}
