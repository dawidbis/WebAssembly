import { Component, input } from '@angular/core';

/** Ikonka komputera przy nazwie gracza sterowanego przez komputer (bot albo gracz, który wyszedł z gry). */
@Component({
  selector: 'app-bot-icon',
  template: `
    <svg [attr.width]="size()" [attr.height]="size()" viewBox="0 0 16 16" role="img" aria-label="Komputer">
      <title>Steruje komputer</title>
      <rect x="1.5" y="2" width="13" height="9" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.5" />
      <rect x="4" y="4.5" width="8" height="4" rx="0.5" fill="currentColor" opacity="0.35" />
      <path d="M8 11v2.5M4.5 14h7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
  `,
  styles: `:host { display: inline-flex; vertical-align: -0.15em; line-height: 0; margin: 0 0.25em; opacity: 0.85; }`,
})
export class BotIcon {
  readonly size = input(13);
}
