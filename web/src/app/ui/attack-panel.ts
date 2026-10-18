import { Component, computed, inject } from '@angular/core';

import type { AttackBlock } from '../../generated/AttackBlock';
import type { AttackReport } from '../../generated/AttackReport';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import type { LandingReport } from '../../generated/LandingReport';
import { amount, full, short } from './format';
import { COMMAND_LIMITS } from './unlocks';

/** Dlaczego nie można atakować. */
export const ATTACK_BLOCKS: Record<AttackBlock, string> = {
  notPlaying: 'Atakować można po wyborze startu.',
  tunnel: 'Prowincja za tunelem – budowa tuneli w przyszłości.',
  owned: 'Cała prowincja jest już twoja.',
  noContact: 'Brak styku – ani granicy, ani rzeki, jeziora czy morza nad twoim brzegiem.',
  limit: 'Za dużo frontów naraz – więcej od wyższego centrum dowodzenia.',
  provinces: 'Za dużo prowincji na jednym froncie – więcej od wyższego centrum dowodzenia.',
  landing: 'Desant morski – do centrum dowodzenia 2 jeden naraz, od 3 bez limitu.',
  noPort: 'Desant morski wypływa z portu – zbuduj port nad tym samym morzem.',
  tooFar: 'Za daleko na desant – przy centrum dowodzenia 1 tylko krótki rejs (do 100 kafli), dalej od 2.',
  noFuel: 'Za mało paliwa na rejs desantu.',
  otherFront: 'Zaznaczone prowincje są na różnych frontach.',
  noTroops: 'Za mało wojska w kraju.',
};

/** Kafle do zajęcia, np. „120 niczyich · 30 cudzych”. */
function tilesText(t: AttackReport): string {
  const enemy = t.enemyTiles > 0 ? ` · ${full(t.enemyTiles)} cudzych` : '';
  return `${full(t.neutralTiles)} niczyich${enemy}`;
}

/** Podpowiedź kosztu kafla: niczyj (tubylcy, teren) i cudzy. */
function costTitle(t: AttackReport): string {
  const forts = t.fortifications > 0 ? `, umocnienia poziom ${t.fortifications}: obrona ×${(1 + t.fortifications * 0.5).toString().replace('.', ',')}` : '';
  const enemy = t.enemyTiles > 0 ? `\nKafel cudzy: ok. ${t.enemyCost} wojska (gęstość wojska obrońcy i zaopatrzenie obu stron${forts})` : '';
  return (
    `Kafel niczyj: ok. ${t.neutralCost} wojska na nizinie – walka z armią tubylczą (zostało ${full(t.natives)});` +
    ' las ×1,25, wyżyny ×1,5, wyżyny z lasem ×1,9, desant ×1,5' +
    enemy
  );
}

/** Trwający front na cel: prowincje frontu (gdy kilka), wojsko, zajęte kafle i postęp. */
function activeView(active: AttackReport | null, name: (p: number) => string) {
  if (!active) return { front: '', active: null };
  const left = active.neutralTiles + active.enemyTiles;
  return {
    front: active.front.length > 1 ? active.front.map(name).join(', ') : '',
    active: { troops: short(active.troops ?? 0), taken: active.taken, progress: (active.taken / Math.max(1, active.taken + left)) * 100 },
  };
}

/** Napis przycisku: dosłanie do trwającego frontu, atak na gracza albo zajęcie ziemi niczyjej. */
function actionLabel(active: AttackReport | null, target: AttackReport | null): string {
  if (active) return 'Dośli wojsko';
  return target && target.enemyTiles > 0 ? 'Atakuj' : 'Zajmij';
}

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
    return { ...base, target: this.targetView(target, targets, id, send) };
  });

  /** Cel panelu: zaznaczona prowincja albo kilka (Ctrl + klik – sprawdzenie zbioru z workera). */
  private targetView(target: AttackReport | null, targets: number[], id: number, send: number) {
    const many = targets.length > 1;
    const names = this.store.map()?.provinces;
    const name = (p: number) => names?.[p - 1]?.name || `#${p}`;
    // Kilka celów: sprawdzenie zbioru z workera (`attackCheck`), szacunek tylko dla zaznaczonej.
    const check = many ? this.session.attackCheck() : target;
    const blocked = check?.blocked ?? null;
    const estimate = target ? target.neutralTiles * target.neutralCost + target.enemyTiles * target.enemyCost : 0;
    const active = target?.troops == null ? null : target;
    return {
      name: many ? targets.map(name).join(', ') : name(id),
      count: many ? this.count(targets.length) : '',
      ...activeView(active, name),
      blocked: blocked ? ATTACK_BLOCKS[blocked] : '',
      landing: this.landing(check?.landing ?? null, send),
      tiles: target ? tilesText(target) : '',
      costTitle: target ? costTitle(target) : '',
      estimate: target ? short(estimate) : '',
      enough: send >= estimate,
      label: actionLabel(active, target),
    };
  }

  /** Liczba celów frontu i limit centrum dowodzenia, np. „2/3”. */
  private count(targets: number): string {
    const command = this.session.myEconomy()?.commandLevel ?? 1;
    const limit = COMMAND_LIMITS[Math.min(6, Math.max(1, command))].provinces;
    return `${targets}/${limit === Infinity ? '∞' : limit}`;
  }

  /** Desant: port, rejs, paliwo z góry, ile wojska popłynie (limit centrum dowodzenia i paliwa). */
  private landing(l: LandingReport | null, send: number): { text: string; title: string } | null {
    if (!l) return null;
    const port = this.store.map()?.provinces[l.port - 1]?.name || `#${l.port}`;
    const troops = Math.min(send, l.maxTroops, l.fuelTroops);
    return {
      text: `⚓ z ${port} · ${amount(l.days)} d · paliwo ${amount((troops / 1000) * l.fuelPer1000)}`,
      title: `Desant morski z portu ${port}: rejs ${amount(l.days)} dni, paliwo z góry ${amount(l.fuelPer1000)} na 1 000 wojska.\nPopłynie ${full(troops)} wojska (limit centrum dowodzenia ${full(l.maxTroops)}, paliwo starczy na ${full(l.fuelTroops)}).`,
    };
  }

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
