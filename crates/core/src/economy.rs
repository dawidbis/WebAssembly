//! Gospodarka państwa: populacja (wojsko i robotnicy) i złoto – zasoby globalne gracza.
//!
//! Wszystko w liczbach całkowitych (determinizm natywny vs wasm): populacja w tysięcznych częściach
//! osoby (`MILLI`), więc nawet mały przyrost na tick nie ginie w zaokrągleniach.
//!
//! - **Limit populacji:** `PEOPLE_PER_TILE` na kafel państwa. Start: `START_POPULATION` niezależnie
//!   od wielkości prowincji.
//! - **Przyrost na tick** (`growth`) zależy od zapełnienia `x = populacja / limit`: do `PEAK_PPM` (55%)
//!   rośnie razem z populacją (przyrost przyspiesza), dalej maleje do zera przy limicie (zwalnia) –
//!   proporcjonalnie do pierwiastka z brakującej części, więc populacja dochodzi do limitu w skończonym
//!   czasie (bez długiego „pełzania” po kilka osób); najszybszy jest przy 55% – `PEAK_GROWTH_PPM`
//!   limitu na tick. Ponad limitem populacja maleje.
//! - **Podział:** nowa populacja idzie do wojska (`troop_share`, %) i robotników (reszta). Gracz
//!   ustawia cel (`target_share`), a podział dochodzi do niego o 1 punkt co `SHARE_STEP_TICKS` tur.
//! - **Złoto:** co `TAX_TICKS` tur (30 s = miesiąc gry) podatek `GOLD_PER_PERSON` od każdego
//!   **robotnika** (wojsko podatków nie płaci). Na start 0.

use serde::{Deserialize, Serialize};

use crate::protocol::PlayerId;

/// Populacja w tysięcznych częściach osoby.
pub const MILLI: u64 = 1000;
/// Limit populacji na kafel państwa.
pub const PEOPLE_PER_TILE: u64 = 100;
/// Populacja na start (niezależnie od wielkości prowincji startowej).
pub const START_POPULATION: u64 = 25_000;
/// Zapełnienie (części na milion), przy którym przyrost jest największy.
pub const PEAK_PPM: u64 = 550_000;
/// Przyrost na tick przy zapełnieniu `PEAK_PPM` – części na milion limitu (640 → ok. 0,064% limitu
/// na turę; poniżej szczytu populacja podwaja się w ok. 60 s, od 55% do pełna dochodzi w ok. 140 s).
pub const PEAK_GROWTH_PPM: u64 = 640;
/// Domyślny udział wojska w nowej populacji (%).
pub const DEFAULT_TROOP_SHARE: u8 = 50;
/// Bezwładność suwaka: rzeczywisty podział zmienia się o 1 punkt procentowy co tyle tur (2 s –
/// zmiana o 50 punktów trwa ok. 100 s).
pub const SHARE_STEP_TICKS: u32 = 20;
/// Podatek co tyle tur (30 s = miesiąc gry – kalendarz na ekranie liczy miesiące tym samym okresem).
pub const TAX_TICKS: u32 = 300;
/// Złota od robotnika za jeden podatek.
pub const GOLD_PER_PERSON: u64 = 1;

/// Stan gospodarki jednego państwa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    /// Wojsko i robotnicy (w `MILLI`).
    pub troops: u64,
    pub workers: u64,
    /// Złoto w skarbcu (w interfejsie dzielone przez 100).
    pub gold: u64,
    /// Rzeczywisty udział wojska w nowej populacji (%) – idzie za `target_share`.
    pub troop_share: u8,
    /// Udział wybrany przez gracza (suwak).
    pub target_share: u8,
    /// Przyrost w ostatniej turze (w `MILLI`, ujemny ponad limitem).
    pub last_growth: i64,
}

impl Default for Economy {
    fn default() -> Self {
        let half = START_POPULATION * MILLI / 2;
        Economy {
            troops: half,
            workers: START_POPULATION * MILLI - half,
            gold: 0,
            troop_share: DEFAULT_TROOP_SHARE,
            target_share: DEFAULT_TROOP_SHARE,
            last_growth: 0,
        }
    }
}

/// Przyrost populacji na tick (w `MILLI`) przy populacji `pop` i limicie `cap` (oba w `MILLI`).
pub fn growth(pop: u64, cap: u64) -> i64 {
    let (pop, cap) = (pop as i128, cap as i128);
    if pop == 0 {
        return 0;
    }
    if cap == 0 {
        // Państwo bez kafli – ludność odpływa w tempie jak ponad limitem.
        return -((pop * PEAK_GROWTH_PPM as i128) / (1_000_000 - PEAK_PPM as i128)).max(1) as i64;
    }
    let peak = cap * PEAK_GROWTH_PPM as i128 / 1_000_000;
    if pop * 1_000_000 <= cap * PEAK_PPM as i128 {
        // x ≤ 55%: przyrost ∝ x (przyspiesza); przy x = 55% – szczyt.
        return (pop * PEAK_GROWTH_PPM as i128 / PEAK_PPM as i128) as i64;
    }
    if pop > cap {
        // Ponad limitem (np. po utracie ziemi): ubytek ∝ nadwyżce.
        return -((pop - cap) * PEAK_GROWTH_PPM as i128 / (1_000_000 - PEAK_PPM as i128)).max(1) as i64;
    }
    // x > 55%: przyrost = szczyt × √(brakująca część / brakująca część przy 55%) – przy 55% szczyt,
    // przy limicie zero, ale do limitu dochodzi w skończonym czasie. Pierwiastek całkowity (`isqrt`)
    // – ten sam wynik natywnie i w wasm. Ostatni krok kończy dokładnie na limicie.
    let room = cap - pop;
    let room_at_peak = cap * (1_000_000 - PEAK_PPM as i128) / 1_000_000;
    let ratio_ppm = (room * 1_000_000 / room_at_peak.max(1)) as u128;
    let sqrt_ppm = (ratio_ppm * 1_000_000).isqrt() as i128;
    let d = (peak * sqrt_ppm / 1_000_000).max(MILLI as i128);
    d.min(room) as i64
}

impl Economy {
    /// Cała populacja (w `MILLI`).
    pub fn population(&self) -> u64 {
        self.troops + self.workers
    }

    /// Jedna tura gry właściwej przy `area` kaflach państwa; `tax` – tura podatku.
    pub fn tick(&mut self, area: u32, tick: u32, tax: bool) {
        if tick.is_multiple_of(SHARE_STEP_TICKS) {
            self.troop_share = match self.troop_share.cmp(&self.target_share) {
                std::cmp::Ordering::Less => self.troop_share + 1,
                std::cmp::Ordering::Greater => self.troop_share - 1,
                std::cmp::Ordering::Equal => self.troop_share,
            };
        }
        let pop = self.population();
        let d = growth(pop, area as u64 * PEOPLE_PER_TILE * MILLI);
        self.last_growth = d;
        if d >= 0 {
            let to_troops = d as u64 * self.troop_share as u64 / 100;
            self.troops += to_troops;
            self.workers += d as u64 - to_troops;
        } else {
            // Ubytek dzieli się po równo z obecnym składem.
            let loss = (-d as u64).min(pop);
            let from_troops = (loss as u128 * self.troops as u128 / pop.max(1) as u128) as u64;
            self.troops -= from_troops.min(self.troops);
            self.workers -= (loss - from_troops).min(self.workers);
        }
        if tax {
            self.gold += self.workers / MILLI * GOLD_PER_PERSON;
        }
    }

    /// Bajty do hasha stanu.
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut bytes = Vec::with_capacity(27);
        bytes.extend(self.troops.to_le_bytes());
        bytes.extend(self.workers.to_le_bytes());
        bytes.extend(self.gold.to_le_bytes());
        bytes.extend([self.troop_share, self.target_share]);
        bytes.into_iter()
    }
}

/// Gospodarka państwa dla interfejsu (osoby i złoto w całych jednostkach; liczby jako `f64` dla TS).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NationReport {
    /// Gracz kierujący państwem.
    pub player: PlayerId,
    pub population: f64,
    /// Limit populacji (kafle × `PEOPLE_PER_TILE`).
    pub max_population: f64,
    pub troops: f64,
    pub workers: f64,
    /// Złoto w skarbcu (surowe – interfejs dzieli przez 100).
    pub gold: f64,
    pub troop_share: u8,
    pub target_share: u8,
    /// Przyrost populacji na sekundę (osoby, przy 10 turach/s).
    pub growth_per_second: f64,
    /// Ile złota przyniesie najbliższy podatek (surowe).
    pub next_tax: f64,
}

impl NationReport {
    pub fn new(player: PlayerId, e: &Economy, area: u32) -> Self {
        let people = |m: u64| (m / MILLI) as f64;
        NationReport {
            player,
            population: people(e.population()),
            max_population: (area as u64 * PEOPLE_PER_TILE) as f64,
            troops: people(e.troops),
            workers: people(e.workers),
            gold: e.gold as f64,
            troop_share: e.troop_share,
            target_share: e.target_share,
            growth_per_second: e.last_growth as f64 / MILLI as f64 * 10.0,
            next_tax: (e.workers / MILLI * GOLD_PER_PERSON) as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_speeds_up_until_55_percent_then_slows_down() {
        let cap = 60_000 * MILLI;
        let at = |pct: u64| growth(cap * pct / 100, cap);
        assert!(at(10) < at(30) && at(30) < at(50) && at(50) < at(55), "przyspiesza do 55%");
        assert!(at(55) > at(70) && at(70) > at(90) && at(90) > at(99), "potem zwalnia");
        assert_eq!(at(100), 0);
        assert!(at(120) < 0, "ponad limitem maleje");
        // Szczyt: PEAK_GROWTH_PPM limitu na turę.
        assert_eq!(at(55), (cap * PEAK_GROWTH_PPM / 1_000_000) as i64);
    }

    #[test]
    fn population_approaches_the_cap_and_tax_is_paid() {
        let mut e = Economy::default();
        let area = 600; // limit 60 000
        let cap = 60_000 * MILLI;
        let mut full_at = None;
        for tick in 1..=3000u32 {
            e.tick(area, tick, tick % TAX_TICKS == 0);
            if full_at.is_none() && e.population() == cap {
                full_at = Some(tick);
            }
        }
        // Dochodzi do limitu w skończonym czasie (ok. 20 s do 55% + ok. 140 s do pełna) i na nim stoi.
        let full_at = full_at.expect("populacja dochodzi do limitu");
        assert!((1200..=2200).contains(&full_at), "pełna po {full_at} turach");
        assert_eq!(e.population(), cap);
        // Podatek tylko od robotników (połowa populacji przy suwaku 50%).
        assert!(e.gold > 10 * 12_500 && e.gold < 10 * 30_000, "10 podatków od robotników: {}", e.gold);
        // Domyślnie pół na pół.
        assert!(e.troops.abs_diff(e.workers) <= 10 * MILLI, "zaokrąglenia: najwyżej kilka osób różnicy");
    }

    #[test]
    fn share_follows_the_target_with_inertia() {
        let mut e = Economy { target_share: 70, ..Default::default() };
        for tick in 1..=SHARE_STEP_TICKS * 10 {
            e.tick(600, tick, false);
        }
        assert_eq!(e.troop_share, 60, "10 kroków po 1 punkcie");
        for tick in 1..=SHARE_STEP_TICKS * 20 {
            e.tick(600, tick, false);
        }
        assert_eq!(e.troop_share, 70, "dochodzi do celu i stoi");
        assert!(e.troops > e.workers, "nowa populacja głównie do wojska");
    }
}
