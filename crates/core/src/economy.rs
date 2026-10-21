//! Gospodarka państwa: populacja (wojsko i robotnicy) i złoto – zasoby globalne gracza.
//!
//! Wszystko w liczbach całkowitych (determinizm natywny vs wasm): populacja w tysięcznych częściach
//! osoby (`MILLI`), więc nawet mały przyrost na tick nie ginie w zaokrągleniach.
//!
//! - **Limit populacji:** `PEOPLE_PER_TILE` na kafel państwa. Start: `START_POPULATION` niezależnie
//!   od wielkości prowincji.
//! - **Przyrost na tick** (`growth`) – krzywa S: `szczyt × x · (1 − x) / (1 + 1,25 x) / 0,16`, `x = populacja / limit`.
//!   Mała populacja rośnie wykładniczo, najszybciej przy 40% limitu (`PEAK_GROWTH_PPM` limitu na tick),
//!   potem coraz wolniej dochodzi do limitu. Ponad limitem populacja maleje.
//! - **Podział:** gracz ustawia suwakiem docelowy udział wojska w **całej** populacji (`target_share`);
//!   nowa populacja od razu w tych proporcjach, a istniejąca przechodzi między wojskiem a robotnikami
//!   z bezwładnością (`SHIFT_PPM` populacji na turę) – także gdy populacja stoi na limicie.
//! - **Złoto:** co `TAX_TICKS` tur (30 s = miesiąc gry) podatek `TAX_PER_1000_WORKERS` od każdego tysiąca
//!   **robotników** (wojsko podatków nie płaci). Na start 0.
//! - **Prowiant** (jedzenie, `PROVISIONS_PEOPLE_PER_UNIT` osób na sztukę dziennie): pokrycie mnoży przyrost
//!   (do `GROWTH_FOOD_PCT`% przy pełnym), a brak – głód: ubywa do `STARVE_PPM_PER_DAY` populacji dziennie
//!   (część bez jedzenia).
//! - **Zaopatrzenie cywilne:** populacja zużywa podstawowe (`BASIC_PEOPLE_PER_UNIT`) – średnie pokrycie
//!   z miesiąca ustala podatek od `BASIC_TAX_MIN_PCT`% (bez podstawowego) do 100% (pełne); dopiero przy
//!   pełnym działa luksusowe (`LUXURY_PEOPLE_PER_UNIT`) – do +`LUXURY_TAX_PCT`% ponad to.

use serde::{Deserialize, Serialize};

use crate::{
    goods::{GOLD, RESOURCE_COUNT, START_SUPPLIES, SUPPLY_COUNT, Supply, TICKS_PER_DAY},
    market::Trade,
    protocol::PlayerId,
    war::AttackInfo,
};

/// Populacja w tysięcznych częściach osoby.
pub const MILLI: u64 = 1000;
/// Limit populacji na kafel państwa.
pub const PEOPLE_PER_TILE: u64 = 10;
/// Populacja na start (niezależnie od wielkości prowincji startowej).
pub const START_POPULATION: u64 = 10_000;
/// Złoto na start (surowe; w interfejsie ÷10 – 30 000).
pub const START_GOLD: u64 = 300_000;
/// Największy przyrost na tick (przy 40% limitu – `PEAK_FILL_PPM`) – części na milion limitu (640 → 0,064%
/// limitu na turę; mała populacja podwaja się w ok. 20 s).
pub const PEAK_GROWTH_PPM: u64 = 640;
/// Zapełnienie limitu przy szczycie przyrostu (części na milion): `x · (1 − x) / (1 + 1,25 x)` ma maksimum
/// przy 40% (w szczycie 0,16), a przy limicie maleje liniowo – populacja dochodzi do niego jak w logistycznej.
pub const PEAK_FILL_PPM: u64 = 400_000;
/// Domyślny udział wojska w populacji (%).
pub const DEFAULT_TROOP_SHARE: u8 = 50;
/// Bezwładność suwaka: co turę między wojskiem a robotnikami przechodzi najwyżej tyle części na
/// milion populacji (500 → 0,05 punktu procentowego na turę, 0,5 na sekundę – zmiana o 50 punktów
/// trwa ok. 100 s).
pub const SHIFT_PPM: u64 = 500;
/// Podatek co tyle tur (30 s = miesiąc gry – kalendarz na ekranie liczy miesiące tym samym okresem).
pub const TAX_TICKS: u32 = 300;
/// Złota (surowego, w interfejsie ÷10) od tysiąca robotników za jeden podatek przy pełnym podstawowym
/// zaopatrzeniu cywilnym.
pub const TAX_PER_1000_WORKERS: u64 = 5000;
/// Podatek bez podstawowego zaopatrzenia cywilnego: tyle % pełnego (liniowo do 100% przy pełnym pokryciu).
pub const BASIC_TAX_MIN_PCT: u64 = 50;
/// Zaopatrzenie cywilne: osób na 1 sztukę dziennie – podstawowe i luksusowe.
pub const BASIC_PEOPLE_PER_UNIT: u64 = 150_000;
pub const LUXURY_PEOPLE_PER_UNIT: u64 = 300_000;
/// Prowiant: osób na 1 sztukę dziennie; przyrost × pokrycie × `GROWTH_FOOD_PCT`%; głód – ubytek
/// `STARVE_PPM_PER_DAY` populacji (części na milion) dziennie × część bez jedzenia.
pub const PROVISIONS_PEOPLE_PER_UNIT: u64 = 200_000;
pub const GROWTH_FOOD_PCT: u64 = 150;
pub const STARVE_PPM_PER_DAY: u64 = 5_000;
/// Podatek: + tyle % przy pełnym pokryciu luksusowego przez cały miesiąc (tylko przy pełnym podstawowym).
pub const LUXURY_TAX_PCT: u64 = 50;
/// Miękki limit wojska (osoby): `ARMY_BASE` + `ARMY_PER_TILE` na kafel państwa + koszary (`buildings::BARRACKS_ARMY`).
pub const ARMY_BASE: u64 = 5_000;
pub const ARMY_PER_TILE: u64 = 3;
/// Utrzymanie wojska co miesiąc (z podatkiem): `UPKEEP_PER_1000` złota surowego od tysiąca żołnierzy (w kraju
/// i na frontach) – 0,1 złota z interfejsu od żołnierza (robotnik płaci do 0,5 podatku); ponad limitem dochodzi `OVER_UPKEEP` × nadwyżka² / limit
/// (przy 2× limitu każdy kolejny żołnierz kosztuje 5×). Bez złota – płaci, ile ma (bez dezercji).
pub const UPKEEP_PER_1000: u64 = 1000;
pub const OVER_UPKEEP: u64 = 2;
/// Części na milion (pokrycia).
const PPM: u64 = 1_000_000;

/// Księga złota jednego miesiąca (od podatku do podatku, złoto surowe): przychody i wydatki według źródła.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    pub tax: u64,
    pub mining: u64,
    /// Łupy z aneksji.
    pub loot: u64,
    /// Budowy (minus zwroty za anulowanie) i przeniesienia stolicy.
    pub buildings: u64,
    pub capital: u64,
    /// Utrzymanie wojska.
    pub army: u64,
    /// Rynek: sprzedaż, zakupy z dostawą i cło tranzytowe od cudzych dostaw.
    pub sales: u64,
    pub purchases: u64,
    pub duties: u64,
}

impl Ledger {
    fn bytes(&self) -> impl Iterator<Item = u8> {
        [
            self.tax,
            self.mining,
            self.loot,
            self.buildings,
            self.capital,
            self.army,
            self.sales,
            self.purchases,
            self.duties,
        ]
        .into_iter()
        .flat_map(|v| v.to_le_bytes())
    }
}

/// Księga dla interfejsu (złoto z interfejsu, ÷10).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LedgerReport {
    pub tax: f64,
    pub mining: f64,
    pub loot: f64,
    pub buildings: f64,
    pub capital: f64,
    pub army: f64,
    pub sales: f64,
    pub purchases: f64,
    pub duties: f64,
}

impl From<&Ledger> for LedgerReport {
    fn from(l: &Ledger) -> Self {
        let g = |v: u64| (v / 10) as f64;
        LedgerReport {
            tax: g(l.tax),
            mining: g(l.mining),
            loot: g(l.loot),
            buildings: g(l.buildings),
            capital: g(l.capital),
            army: g(l.army),
            sales: g(l.sales),
            purchases: g(l.purchases),
            duties: g(l.duties),
        }
    }
}

/// Stan gospodarki jednego państwa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    /// Wojsko w kraju (broni) i robotnicy (w `MILLI`).
    pub troops: u64,
    pub workers: u64,
    /// Wojsko w atakach (w `MILLI`) – liczy się do populacji i udziału wojska, ale nie broni
    /// i nie przechodzi do robotników; wraca do `troops`, gdy atak się kończy.
    pub deployed: u64,
    /// Złoto w skarbcu (w interfejsie dzielone przez 10).
    pub gold: u64,
    /// Docelowy udział wojska w całej populacji (%, suwak gracza) – rzeczywisty idzie za nim
    /// z bezwładnością (`SHIFT_PPM`).
    pub target_share: u8,
    /// Przyrost w ostatniej turze (w `MILLI`, ujemny ponad limitem).
    pub last_growth: i64,
    /// Zapas surowców w globalnej puli (w tysięcznych jednostki, indeks = numer surowca; złoto zawsze 0 –
    /// trafia od razu do skarbca).
    pub stock: [u64; RESOURCE_COUNT],
    /// Zapas zaopatrzenia (w tysięcznych, indeks = `Supply as usize`); na start materiały inżynieryjne.
    pub supplies: [u64; SUPPLY_COUNT],
    /// Ostatnia tura (w tysięcznych): zaopatrzenie wyprodukowane przez fabryki i surowce zużyte
    /// (fabryki, nawozy) – do raportu „na dzień”.
    pub supply_output: [u64; SUPPLY_COUNT],
    pub resource_use: [u64; RESOURCE_COUNT],
    /// Zaopatrzenie zużyte w ostatniej turze (składniki fabryk, ataki, ludność).
    pub supply_use: [u64; SUPPLY_COUNT],
    /// Pokrycie potrzeby ludności w ostatniej turze (części na milion): podstawowe i luksusowe (0, gdy
    /// podstawowego nie starczyło – luksusowe wtedy nie działa).
    pub basic_ppm: u64,
    pub luxury_ppm: u64,
    /// Pokrycie potrzeby prowiantu w ostatniej turze (części na milion).
    pub food_ppm: u64,
    /// Od ostatniego podatku: tury i sumy pokrycia podstawowego i luksusowego (średnie – mnożnik podatku).
    pub civil_ticks: u32,
    pub luxury_sum: u64,
    pub basic_sum: u64,
    /// Księga złota: bieżący i poprzedni miesiąc.
    pub month: Ledger,
    pub last_month: Ledger,
    /// Rozbicie zmiany populacji w ostatniej turze (`MILLI`): przyrost logistyczny przed prowiantem i głód.
    pub last_base_growth: i64,
    pub last_starve: i64,
    /// Rynek: ustawienia suwaków, udział w rynku, wczorajsze transakcje.
    pub trade: Trade,
}

impl Default for Economy {
    fn default() -> Self {
        let half = START_POPULATION * MILLI / 2;
        Economy {
            troops: half,
            workers: START_POPULATION * MILLI - half,
            deployed: 0,
            gold: START_GOLD,
            target_share: DEFAULT_TROOP_SHARE,
            last_growth: 0,
            stock: [0; RESOURCE_COUNT],
            supplies: START_SUPPLIES.map(|u| u * MILLI),
            supply_output: [0; SUPPLY_COUNT],
            resource_use: [0; RESOURCE_COUNT],
            supply_use: [0; SUPPLY_COUNT],
            basic_ppm: PPM,
            luxury_ppm: 0,
            food_ppm: PPM,
            civil_ticks: 0,
            luxury_sum: 0,
            basic_sum: 0,
            month: Ledger::default(),
            last_month: Ledger::default(),
            last_base_growth: 0,
            last_starve: 0,
            trade: Trade::default(),
        }
    }
}

/// Przyrost populacji na tick (w `MILLI`) przy populacji `pop` i limicie `cap` (oba w `MILLI`).
pub fn growth(pop: u64, cap: u64) -> i64 {
    let (pop, cap) = (pop as i128, cap as i128);
    if pop == 0 {
        return 0;
    }
    // szczyt × x(1 − x) / (1 + 1,25x) / 0,16 = szczyt × 25 · P(K − P) / (K · (4K + 5P)) · K – po skróceniu jak niżej;
    // ponad limitem (i bez kafli) ujemny – ludność odpływa.
    let d = PEAK_GROWTH_PPM as i128 * 25 * pop * (cap - pop) / (1_000_000 * (4 * cap + 5 * pop));
    if pop > cap {
        return d.min(-1) as i64;
    }
    d as i64
}

impl Economy {
    /// Cała populacja (w `MILLI`).
    pub fn population(&self) -> u64 {
        self.troops + self.workers + self.deployed
    }

    /// Całe wojsko: w kraju i w atakach (w `MILLI`).
    pub fn army(&self) -> u64 {
        self.troops + self.deployed
    }

    /// Podział populacji od razu według celu suwaka – bez bezwładności (przed startem gry).
    pub fn split_now(&mut self) {
        let pop = self.troops + self.workers;
        self.troops = (pop as u128 * self.target_share as u128 / 100) as u64;
        self.workers = pop - self.troops;
    }

    /// Rzeczywisty udział wojska w populacji (%, zaokrąglony).
    pub fn troop_share(&self) -> u8 {
        let pop = self.population();
        if pop == 0 { self.target_share } else { ((self.army() as u128 * 100 + pop as u128 / 2) / pop as u128) as u8 }
    }

    /// Utrzymanie wojska za miesiąc (złoto surowe) przy miękkim limicie `limit` (osoby): `UPKEEP_PER_1000` od
    /// tysiąca żołnierzy + kara `OVER_UPKEEP` × nadwyżka² / limit.
    pub fn upkeep(&self, limit: u64) -> u64 {
        let troops = self.army() / MILLI;
        let over = troops.saturating_sub(limit);
        let weighted = troops as u128 + OVER_UPKEEP as u128 * (over as u128).pow(2) / limit.max(1) as u128;
        (weighted * UPKEEP_PER_1000 as u128 / 1000) as u64
    }

    /// Jedna tura gry właściwej przy limicie populacji `cap` i limicie wojska `army_limit` (osoby); `tax` – tura
    /// podatku i utrzymania wojska (płaci, ile ma w skarbcu).
    pub fn tick(&mut self, cap: u64, army_limit: u64, tax: bool) {
        self.consume_civil();
        let pop = self.population();
        let mut d = growth(pop, cap * MILLI);
        self.last_base_growth = d;
        if d > 0 {
            // Prowiant: przyrost × pokrycie × `GROWTH_FOOD_PCT`% (ubytek ponad limitem – bez zmian).
            let scaled = d as i128 * (GROWTH_FOOD_PCT * self.food_ppm) as i128 / (100 * PPM) as i128;
            d = if self.food_ppm > 0 { (scaled as i64).max(1) } else { 0 };
        }
        // Głód: część populacji bez jedzenia ubywa.
        let starve = pop as u128 * STARVE_PPM_PER_DAY as u128 * (PPM - self.food_ppm) as u128
            / (PPM as u128 * PPM as u128 * TICKS_PER_DAY as u128);
        d -= starve as i64;
        self.last_starve = starve as i64;
        self.last_growth = d;
        if d >= 0 {
            // Nowa populacja od razu w docelowych proporcjach.
            let to_troops = d as u64 * self.target_share as u64 / 100;
            self.troops += to_troops;
            self.workers += d as u64 - to_troops;
        } else {
            // Ubytek dzieli się po równo z obecnym składem (wojsko w atakach nie ubywa).
            let home = self.troops + self.workers;
            let loss = (-d as u64).min(home);
            let from_troops = (loss as u128 * self.troops as u128 / home.max(1) as u128) as u64;
            self.troops -= from_troops.min(self.troops);
            self.workers -= (loss - from_troops).min(self.workers);
        }
        self.shift();
        self.civil_ticks += 1;
        self.luxury_sum += self.luxury_ppm;
        self.basic_sum += self.basic_ppm;
        if tax {
            let t = self.next_tax();
            self.gold += t;
            self.month.tax += t;
            let upkeep = self.upkeep(army_limit).min(self.gold);
            self.gold -= upkeep;
            self.month.army += upkeep;
            self.last_month = std::mem::take(&mut self.month);
            (self.civil_ticks, self.luxury_sum, self.basic_sum) = (0, 0, 0);
        }
    }

    /// Podatek, gdyby był teraz: robotnicy × `TAX_PER_1000_WORKERS` / 1000 × mnożnik z podstawowego
    /// (`BASIC_TAX_MIN_PCT`–100%) + premia z luksusowego – średnie pokrycia z miesiąca.
    pub fn next_tax(&self) -> u64 {
        let base = self.workers / MILLI * TAX_PER_1000_WORKERS / 1000;
        base * self.basic_tax_ppm() / PPM + base * LUXURY_TAX_PCT * self.tax_bonus_ppm() / (100 * PPM)
    }

    /// Mnożnik podatku z podstawowego zaopatrzenia (części na milion): `BASIC_TAX_MIN_PCT`% bez niego, 100%
    /// przy pełnym pokryciu przez cały miesiąc (na początku miesiąca – pokrycie z ostatniej tury).
    pub fn basic_tax_ppm(&self) -> u64 {
        let basic = self.basic_sum.checked_div(self.civil_ticks as u64).unwrap_or(self.basic_ppm);
        let min = BASIC_TAX_MIN_PCT * PPM / 100;
        min + (PPM - min) * basic.min(PPM) / PPM
    }

    /// Średnie pokrycie luksusowego od ostatniego podatku (części na milion).
    pub fn tax_bonus_ppm(&self) -> u64 {
        self.luxury_sum.checked_div(self.civil_ticks as u64).unwrap_or(0)
    }

    /// Zużycie przez ludność na tę turę: prowiant, podstawowe cywilne, a przy pełnym – luksusowe.
    fn consume_civil(&mut self) {
        let pop = self.population();
        let take = |e: &mut Self, s: Supply, people: u64| {
            let need = pop / (people * TICKS_PER_DAY);
            if need == 0 {
                return PPM;
            }
            let got = e.supplies[s as usize].min(need);
            e.supplies[s as usize] -= got;
            e.supply_use[s as usize] += got;
            got * PPM / need
        };
        self.food_ppm = take(self, Supply::Provisions, PROVISIONS_PEOPLE_PER_UNIT);
        self.basic_ppm = take(self, Supply::BasicCivil, BASIC_PEOPLE_PER_UNIT);
        self.luxury_ppm =
            if self.basic_ppm >= PPM { take(self, Supply::LuxuryCivil, LUXURY_PEOPLE_PER_UNIT) } else { 0 };
    }

    /// Przesunięcie ludzi między wojskiem a robotnikami w stronę celu suwaka – najwyżej `SHIFT_PPM`
    /// populacji na turę (także gdy populacja stoi na limicie).
    fn shift(&mut self) {
        let pop = self.population();
        let wanted = (pop as u128 * self.target_share as u128 / 100) as u64;
        let step = (pop as u128 * SHIFT_PPM as u128 / 1_000_000).max(1) as u64;
        let army = self.army();
        if wanted > army {
            let k = (wanted - army).min(step).min(self.workers);
            self.troops += k;
            self.workers -= k;
        } else {
            // Do robotników wracają tylko żołnierze z kraju.
            let k = (army - wanted).min(step).min(self.troops);
            self.troops -= k;
            self.workers += k;
        }
    }

    /// Bajty do hasha stanu.
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut bytes = Vec::with_capacity(25 + 8 * (RESOURCE_COUNT + SUPPLY_COUNT));
        bytes.extend(self.troops.to_le_bytes());
        bytes.extend(self.workers.to_le_bytes());
        bytes.extend(self.deployed.to_le_bytes());
        bytes.extend(self.gold.to_le_bytes());
        bytes.push(self.target_share);
        bytes.extend(self.stock.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supplies.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supply_output.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.resource_use.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supply_use.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.basic_ppm.to_le_bytes());
        bytes.extend(self.luxury_ppm.to_le_bytes());
        bytes.extend(self.food_ppm.to_le_bytes());
        bytes.extend(self.civil_ticks.to_le_bytes());
        bytes.extend(self.luxury_sum.to_le_bytes());
        bytes.extend(self.basic_sum.to_le_bytes());
        bytes.extend(self.month.bytes());
        bytes.extend(self.last_month.bytes());
        bytes.extend(self.trade.hash_bytes());
        bytes.into_iter()
    }
}

/// Wielkości państwa liczone z mapy i budynków na początku tury (`Game::rates`) – wspólne dla
/// rozliczenia tury i raportu dla interfejsu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NationRates {
    /// Limit populacji (osoby): kafle × `PEOPLE_PER_TILE` + miasta (`cap_cities`).
    pub cap: u64,
    pub cap_cities: u64,
    /// Robotników potrzebnych do pełnej obsady budynków (osoby).
    pub staff: u64,
    /// Pokrycie obsady (części na milion): min(1, robotnicy / obsada) – mnoży produkcję budynków.
    pub staff_ppm: u64,
    /// Dzienna potrzeba nawozów pól uprawnych (w tysięcznych) i jej pokrycie z puli (części na milion).
    pub fertilizer_need: u64,
    pub fertilizer_ppm: u64,
    /// Wydobycie na dzień (w tysięcznych, indeks = numer surowca; złoto – w surowym złocie).
    pub production: [u64; RESOURCE_COUNT],
    /// Suma poziomów uniwersytetów w państwie (progi technologii, `factories::university_bonus`).
    pub universities: u32,
    /// Poziom centrum dowodzenia (`Game::command_level`).
    pub command_level: u8,
    /// Miękki limit wojska (osoby): baza + kafle + koszary.
    pub army_limit: u64,
}

/// Gospodarka państwa dla interfejsu (osoby i złoto w całych jednostkach; liczby jako `f64` dla TS).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NationReport {
    /// Gracz kierujący państwem.
    pub player: PlayerId,
    pub population: f64,
    /// Limit populacji (kafle × `PEOPLE_PER_TILE` + miasta).
    pub max_population: f64,
    pub troops: f64,
    pub workers: f64,
    /// Złoto w skarbcu (surowe – interfejs dzieli przez 10).
    pub gold: f64,
    pub troop_share: u8,
    pub target_share: u8,
    /// Przyrost populacji na sekundę (osoby, przy 10 turach/s).
    pub growth_per_second: f64,
    /// Ile złota przyniesie najbliższy podatek (surowe).
    pub next_tax: f64,
    /// Zapas surowców (jednostki, indeks = numer surowca) i wydobycie na dzień.
    pub stock: Vec<f64>,
    pub production: Vec<f64>,
    /// Złoto (surowe) na dzień z wydobycia złota.
    pub gold_mining: f64,
    /// Zapas zaopatrzenia (jednostki, indeks = `Supply as usize`).
    pub supplies: Vec<f64>,
    /// Obsada budynków: potrzebni robotnicy (osoby) i pokrycie (0–1).
    pub staff: f64,
    pub staff_coverage: f64,
    /// Nawozy: potrzeba na dzień (jednostki) i pokrycie (0–1).
    pub fertilizer_need: f64,
    pub fertilizer_coverage: f64,
    /// Na dzień (jednostki, z ostatniej tury): zaopatrzenie z fabryk (indeks = `Supply as usize`)
    /// i zużycie surowców przez fabryki i nawożenie (indeks = numer surowca).
    pub supply_production: Vec<f64>,
    pub resource_use: Vec<f64>,
    /// Zużycie zaopatrzenia na dzień (składniki fabryk, ataki).
    pub supply_use: Vec<f64>,
    /// Poziom centrum dowodzenia.
    pub command_level: u8,
    /// Suma poziomów uniwersytetów w państwie.
    pub universities: u32,
    /// Pokrycie potrzeby ludności (0–1): podstawowe (przyrost) i luksusowe (premia do podatku) z ostatniej
    /// tury, średnia luksusowego od ostatniego podatku i potrzeba na dzień (sztuki) obu rodzajów.
    pub basic_coverage: f64,
    pub luxury_coverage: f64,
    /// Prowiant: pokrycie (0–1) z ostatniej tury i potrzeba na dzień (sztuki).
    pub food_coverage: f64,
    pub food_need: f64,
    pub tax_bonus: f64,
    /// Mnożnik podatku z podstawowego zaopatrzenia (`BASIC_TAX_MIN_PCT`% – 1).
    pub basic_tax: f64,
    /// Podatek bez mnożników (złoto surowe: robotnicy × `TAX_PER_1000_WORKERS` / 1000) i stawka od 1 000.
    pub tax_base: f64,
    pub tax_per_1000: f64,
    /// Księga złota: bieżący i poprzedni miesiąc (złoto z interfejsu).
    pub month: LedgerReport,
    pub last_month: LedgerReport,
    /// Ludność na dzień (osoby): przyrost logistyczny przed prowiantem, mnożnik prowiantu, głód.
    pub growth_base_per_day: f64,
    pub growth_food_pct: f64,
    pub starve_per_day: f64,
    /// Limit populacji: z kafli i z miast (osoby).
    pub cap_tiles: f64,
    pub cap_cities: f64,
    pub basic_need: f64,
    pub luxury_need: f64,
    /// Wojsko w atakach (osoby; `troops` – tylko w kraju) i trwające ataki.
    pub deployed: f64,
    pub attacks: Vec<AttackInfo>,
    /// Państwo przyłączone (stolica zdobyta) – gracz zdobywcy; `None` – w grze.
    pub annexed_by: Option<PlayerId>,
    /// Miękki limit wojska (osoby) i utrzymanie wojska za najbliższy miesiąc (złoto surowe).
    pub army_limit: f64,
    pub upkeep: f64,
    /// Państwem kieruje komputer (bot albo gracz, który wyszedł z gry).
    pub bot: bool,
}

impl NationReport {
    pub fn new(player: PlayerId, e: &Economy, rates: &NationRates, attacks: Vec<AttackInfo>) -> Self {
        let production = &rates.production;
        let people = |m: u64| (m / MILLI) as f64;
        // Z ostatniej tury (w tysięcznych) na dzień, w jednostkach.
        let per_day = |m: u64| (m * TICKS_PER_DAY) as f64 / MILLI as f64;
        NationReport {
            player,
            population: people(e.population()),
            max_population: rates.cap as f64,
            troops: people(e.troops),
            workers: people(e.workers),
            gold: e.gold as f64,
            troop_share: e.troop_share(),
            target_share: e.target_share,
            growth_per_second: e.last_growth as f64 / MILLI as f64 * 10.0,
            next_tax: e.next_tax() as f64,
            stock: e.stock.iter().map(|&s| s as f64 / MILLI as f64).collect(),
            production: production
                .iter()
                .enumerate()
                .map(|(r, &p)| if r == GOLD as usize { 0.0 } else { p as f64 / MILLI as f64 })
                .collect(),
            gold_mining: production[GOLD as usize] as f64,
            supplies: e.supplies.iter().map(|&s| s as f64 / MILLI as f64).collect(),
            staff: rates.staff as f64,
            staff_coverage: rates.staff_ppm as f64 / 1e6,
            fertilizer_need: rates.fertilizer_need as f64 / MILLI as f64,
            fertilizer_coverage: rates.fertilizer_ppm as f64 / 1e6,
            supply_production: e.supply_output.iter().map(|&s| per_day(s)).collect(),
            resource_use: e.resource_use.iter().map(|&s| per_day(s)).collect(),
            supply_use: e.supply_use.iter().map(|&s| per_day(s)).collect(),
            command_level: rates.command_level,
            universities: rates.universities,
            basic_coverage: e.basic_ppm as f64 / 1e6,
            food_coverage: e.food_ppm as f64 / 1e6,
            food_need: people(e.population()) / PROVISIONS_PEOPLE_PER_UNIT as f64,
            luxury_coverage: e.luxury_ppm as f64 / 1e6,
            tax_bonus: e.tax_bonus_ppm() as f64 / 1e6 * LUXURY_TAX_PCT as f64 / 100.0,
            basic_tax: e.basic_tax_ppm() as f64 / 1e6,
            tax_base: (e.workers / MILLI * TAX_PER_1000_WORKERS / 1000) as f64,
            tax_per_1000: TAX_PER_1000_WORKERS as f64,
            month: (&e.month).into(),
            last_month: (&e.last_month).into(),
            growth_base_per_day: e.last_base_growth as f64 * TICKS_PER_DAY as f64 / MILLI as f64,
            growth_food_pct: (GROWTH_FOOD_PCT * e.food_ppm) as f64 / (100 * PPM) as f64,
            starve_per_day: e.last_starve as f64 * TICKS_PER_DAY as f64 / MILLI as f64,
            cap_tiles: (rates.cap - rates.cap_cities) as f64,
            cap_cities: rates.cap_cities as f64,
            basic_need: people(e.population()) / BASIC_PEOPLE_PER_UNIT as f64,
            luxury_need: people(e.population()) / LUXURY_PEOPLE_PER_UNIT as f64,
            deployed: people(e.deployed),
            attacks,
            annexed_by: None,
            army_limit: rates.army_limit as f64,
            upkeep: e.upkeep(rates.army_limit) as f64,
            bot: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_is_an_s_curve_peaking_at_40_percent() {
        let cap = 60_000 * MILLI;
        let at = |pct: u64| growth(cap * pct / 100, cap);
        assert!(at(10) < at(30) && at(30) < at(38) && at(38) <= at(40), "przyspiesza do 40%");
        assert!(
            at(40) >= at(42) && at(42) > at(50) && at(50) > at(70) && at(70) > at(90) && at(90) > at(99),
            "potem zwalnia"
        );
        assert_eq!(at(100), 0);
        assert!(at(120) < 0, "ponad limitem maleje");
        // Szczyt: PEAK_GROWTH_PPM limitu na turę.
        assert_eq!(at(40), (cap * PEAK_GROWTH_PPM / 1_000_000) as i64);
        assert_eq!(PEAK_FILL_PPM, 400_000);
    }

    #[test]
    fn population_approaches_the_cap_and_tax_is_paid() {
        // Pod dostatkiem prowiantu (przyrost 150%), bez luksusowego (podatek bez premii).
        let mut supplies = [0; SUPPLY_COUNT];
        supplies[Supply::Provisions as usize] = 100_000 * MILLI;
        let mut e = Economy { supplies, ..Default::default() };
        let limit = 60_000;
        let cap = 60_000 * MILLI;
        let mut full_at = None;
        let mut upkeep = 0;
        for tick in 1..=3000u32 {
            e.tick(limit, ARMY_BASE, tick % TAX_TICKS == 0);
            if tick % TAX_TICKS == 0 {
                upkeep += e.last_month.army;
            }
            if full_at.is_none() && e.population() == cap {
                full_at = Some(tick);
            }
        }
        // Krzywa S: dochodzi pod sam limit (ostatnie osoby – zaokrąglenia) i tam stoi.
        let _ = full_at;
        assert!(e.population() > cap - cap / 1000 && e.population() <= cap, "pod limitem: {}", e.population());
        // Podatek tylko od robotników (połowa populacji przy suwaku 50%): 2,5 złota surowego od osoby (bez utrzymania
        // wojska, które płaci się z podatkiem).
        assert!(upkeep > 0, "utrzymanie wojska co miesiąc");
        let gold = e.gold + upkeep - START_GOLD;
        assert!(gold > 10 * 30_000 && gold < 10 * 75_000, "10 podatków od robotników: {gold}");
        // Domyślnie pół na pół.
        assert!(e.troops.abs_diff(e.workers) <= 10 * MILLI, "zaokrąglenia: najwyżej kilka osób różnicy");
    }

    #[test]
    fn army_upkeep_grows_fast_over_the_soft_limit() {
        let army = |troops: u64| Economy { troops: troops * MILLI, workers: 0, ..Default::default() };
        let per = UPKEEP_PER_1000 / 1000;
        assert_eq!(army(10_000).upkeep(10_000), 10_000 * per, "w limicie – liniowo");
        // 2× limitu: 2L + 2·L²/L = 4L, a każdy kolejny żołnierz kosztuje 5×.
        assert_eq!(army(20_000).upkeep(10_000), 40_000 * per);
        assert_eq!(army(20_001).upkeep(10_000) - army(20_000).upkeep(10_000), 5 * per);
        // Bez złota – płaci, ile ma (skarbiec nie schodzi poniżej zera).
        let mut poor = Economy { gold: 100, ..army(50_000) };
        poor.tick(60_000, 1_000, true);
        assert_eq!(poor.gold, 0);
    }

    #[test]
    fn provisions_feed_growth_and_luxury_raises_taxes() {
        let fed = |food: u64, basic: u64, luxury: u64| {
            let mut s = [0; SUPPLY_COUNT];
            s[Supply::Provisions as usize] = food * MILLI;
            s[Supply::BasicCivil as usize] = basic * MILLI;
            s[Supply::LuxuryCivil as usize] = luxury * MILLI;
            Economy { supplies: s, ..Default::default() }
        };
        let start = fed(0, 0, 0).population();
        let (mut hungry, mut fed_up, mut basic_only) = (fed(0, 0, 0), fed(1000, 0, 0), fed(1000, 1000, 0));
        for _ in 0..50 {
            hungry.tick(60_000, ARMY_BASE, false);
            fed_up.tick(60_000, ARMY_BASE, false);
            basic_only.tick(60_000, ARMY_BASE, false);
        }
        assert!(hungry.population() < start, "bez prowiantu populacji ubywa");
        assert!(fed_up.population() > start, "z prowiantem rośnie");
        assert_eq!(basic_only.population(), fed_up.population(), "podstawowe cywilne nie zmienia przyrostu");
        assert!(fed_up.supplies[Supply::Provisions as usize] < 1000 * MILLI, "ludność je prowiant");
        assert!(basic_only.supplies[Supply::BasicCivil as usize] < 1000 * MILLI, "i zużywa podstawowe");
        let fed = |basic: u64, luxury: u64| fed(1000, basic, luxury);
        // Podatek: luksusowe daje premię tylko przy pełnym podstawowym.
        let tax = |mut e: Economy| {
            for t in 1..=TAX_TICKS {
                e.tick(60_000, ARMY_BASE, t == TAX_TICKS);
            }
            (e.gold + e.last_month.army - START_GOLD, e.workers / MILLI * TAX_PER_1000_WORKERS / 1000)
        };
        let (gold, base) = tax(fed(1000, 1000));
        assert!(gold > base * 14 / 10, "luksusowe: +50% podatku ({gold} przy {base})");
        let (gold, base) = tax(fed(0, 1000));
        assert!(gold <= base / 2 + 1, "bez podstawowego połowa podatku, luksusowe nie działa ({gold} przy {base})");
        let (gold, base) = tax(fed(1000, 0));
        assert!(gold.abs_diff(base) <= 1, "pełne podstawowe – pełny podatek ({gold} przy {base})");
    }

    #[test]
    fn share_follows_the_target_with_inertia() {
        // Populacja na limicie (bez przyrostu) – podział i tak idzie za suwakiem.
        let cap = 60_000 * MILLI;
        let mut supplies = [0; SUPPLY_COUNT];
        supplies[Supply::Provisions as usize] = 100_000 * MILLI;
        let mut e = Economy { troops: cap / 2, workers: cap / 2, target_share: 70, supplies, ..Default::default() };
        for _ in 0..100 {
            e.tick(60_000, ARMY_BASE, false);
        }
        assert_eq!(e.population(), cap);
        assert_eq!(e.troop_share(), 55, "100 tur po 0,05 punktu");
        for _ in 0..400 {
            e.tick(60_000, ARMY_BASE, false);
        }
        assert_eq!(e.troop_share(), 70, "dochodzi do celu i stoi");
        e.target_share = 0;
        for _ in 0..2000 {
            e.tick(60_000, ARMY_BASE, false);
        }
        assert_eq!((e.troops, e.workers), (0, cap), "w obie strony, aż do zera");
    }
}
