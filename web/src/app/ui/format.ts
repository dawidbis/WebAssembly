/**
 * Krótki zapis liczby w interfejsie (mniej miejsca): do tysiąca w całości (850), dalej tysiące „k”
 * (25,3k, 380k) i miliony „m” (1,2m, 45m) – do 100 z jednym miejscem po przecinku.
 */
export function short(n: number): string {
  const sign = n < 0 ? '−' : '';
  const a = Math.abs(n);
  if (a < 1000) return sign + Math.round(a);
  if (a < 999_950) return sign + scaled(a / 1000) + 'k';
  return sign + scaled(a / 1_000_000) + 'm';
}

function scaled(v: number): string {
  if (v >= 99.95) return String(Math.round(v));
  return v.toFixed(1).replace(/\.0$/, '').replace('.', ',');
}

const whole = new Intl.NumberFormat('pl-PL', { maximumFractionDigits: 0 });

/** Pełna liczba po polsku (25 340) – do podpowiedzi przy skróconym zapisie. */
export function full(n: number): string {
  return whole.format(n);
}

/** Mała ilość (zapasy surowców): do 100 z jednym miejscem po przecinku (0,3 · 12,5), dalej jak `short`. */
export function amount(n: number): string {
  if (Math.abs(n) >= 99.95) return short(n);
  return n.toFixed(1).replace('.', ',');
}
