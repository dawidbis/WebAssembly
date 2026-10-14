type Rgb = readonly [number, number, number];

/**
 * Kolory państw (numer państwa `n` → `NATION_COLORS[(n - 1) % długość]`). Nasycone, żeby odcinały się
 * od szarych prowincji niczyich (`POLITICAL_UNOWNED` w `render/provinces.ts`). Flagi (`render/flags.ts`)
 * mają kolor państwa jako dominujący.
 */
export const NATION_COLORS: readonly Rgb[] = [
  [206, 62, 54], // czerwony
  [52, 108, 196], // niebieski
  [58, 152, 76], // zielony
  [232, 168, 36], // bursztynowy
  [138, 78, 178], // fioletowy
  [36, 164, 168], // morski
  [226, 108, 168], // różowy
  [128, 88, 46], // brązowy
  [244, 124, 36], // pomarańczowy
  [84, 86, 160], // indygo
  [146, 186, 46], // limonkowy
  [176, 38, 104], // amarantowy
];

export function nationColor(nation: number): Rgb {
  return NATION_COLORS[(nation - 1) % NATION_COLORS.length];
}

/** Kolor CSS państwa (ramka prowincji, panel). */
export function nationCss(nation: number): string {
  const [r, g, b] = nationColor(nation);
  return `rgb(${r}, ${g}, ${b})`;
}
