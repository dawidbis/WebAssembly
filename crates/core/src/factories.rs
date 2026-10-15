//! Fabryki i receptury zaopatrzenia (docs/gdd/zaopatrzenie.md).
//!
//! Receptura to lista składników, a składnik – lista wymiennych wejść (surowiec z puli albo zaopatrzenie,
//! np. paliwo w podstawowym cywilnym). Fabryka bierze to wejście, z którego zrobi najwięcej (przy
//! remisie – pierwsze w tabeli), a gdy brakuje składników – produkuje ułamek. Ilości w tysięcznych
//! jednostki na 1 sztukę zaopatrzenia. Rozliczenie (kolejność fabryk, obsada) jest w `game.rs`.

use serde::{Deserialize, Serialize};

use crate::{
    economy::{Economy, MILLI},
    goods::{RESOURCE_NAMES, SUPPLIES, SUPPLY_NAMES, Supply, TICKS_PER_DAY},
};

/// Produkcja za poziom fabryki: 1,0 sztuki na dzień (w tysięcznych).
pub const OUTPUT_PER_LEVEL_PER_DAY: u64 = 1000;
/// Produkcja za poziom na turę (w tysięcznych).
pub const OUTPUT_PER_LEVEL_PER_TICK: u64 = OUTPUT_PER_LEVEL_PER_DAY / TICKS_PER_DAY;
/// Przezbrojenie fabryki na inny rodzaj: tyle tur bez produkcji (5 dni).
pub const RETOOL_TICKS: u32 = 5 * TICKS_PER_DAY as u32;
/// Uniwersytety (suma poziomów w państwie): premia do produkcji fabryk (%) od progu.
pub const UNIVERSITY_BONUS: [(u32, u64); 2] = [(5, 10), (20, 20)];
/// Uniwersytety: paliwo syntetyczne z węgla od tylu poziomów.
pub const SYNTHETIC_FUEL_UNIVERSITIES: u32 = 10;
/// Centrum dowodzenia potrzebne do rodzaju zaopatrzenia (kolejność `Supply`).
pub const COMMAND_LEVEL: [u8; 7] = [1, 2, 4, 1, 1, 1, 1];

/// Wejście receptury: surowiec z puli (numer surowca) albo zaopatrzenie z puli zaopatrzenia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Resource(u8),
    Supply(Supply),
}

/// Wymienne wejście składnika: co, ile na sztukę (w tysięcznych) i od ilu poziomów uniwersytetów.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alternative {
    pub input: Input,
    pub amount: u64,
    pub universities: u32,
}

const fn r(resource: u8, amount: u64) -> Alternative {
    Alternative { input: Input::Resource(resource), amount, universities: 0 }
}

const fn s(supply: Supply, amount: u64) -> Alternative {
    Alternative { input: Input::Supply(supply), amount, universities: 0 }
}

/// Paliwo syntetyczne (dopiero od `SYNTHETIC_FUEL_UNIVERSITIES` poziomów uniwersytetów).
const fn synthetic(resource: u8, amount: u64) -> Alternative {
    Alternative { input: Input::Resource(resource), amount, universities: SYNTHETIC_FUEL_UNIVERSITIES }
}

// Numery surowców (kolejność `RESOURCE_NAMES`; test pilnuje nazw).
const GRAIN: u8 = 0;
const RICE: u8 = 1;
const POTATOES: u8 = 2;
const COTTON: u8 = 3;
const FIBERS: u8 = 4;
const TOBACCO: u8 = 5;
const SUGAR: u8 = 6;
const COFFEE: u8 = 7;
const TEA: u8 = 8;
const RUBBER: u8 = 9;
const CATTLE: u8 = 10;
const HORSES: u8 = 11;
const WOOL: u8 = 12;
const FURS: u8 = 13;
const WOOD: u8 = 14;
const COAL: u8 = 15;
const LIGNITE: u8 = 16;
const OIL: u8 = 17;
const IRON: u8 = 19;
const MANGANESE: u8 = 20;
const CHROME: u8 = 21;
const NICKEL: u8 = 22;
const TUNGSTEN: u8 = 23;
const MOLYBDENUM: u8 = 24;
const COPPER: u8 = 25;
const LEAD: u8 = 26;
const ZINC: u8 = 27;
const ALUMINIUM: u8 = 28;
const TIN: u8 = 29;
const CEMENT: u8 = 32;
const SALTPETER: u8 = 33;
const SULFUR: u8 = 34;

/// Receptura każdego rodzaju zaopatrzenia (kolejność `Supply`): składniki, każdy z wymiennymi wejściami.
pub const RECIPES: [&[&[Alternative]]; 7] = [
    // Lekkie wojskowe.
    &[
        &[r(IRON, 500), r(COPPER, 500)],
        &[r(COTTON, 300), r(WOOL, 300), r(FIBERS, 300)],
        &[r(SALTPETER, 200), r(SULFUR, 200)],
        &[r(LEAD, 100), r(ZINC, 100)],
    ],
    // Standardowe wojskowe.
    &[
        &[r(IRON, 600)],
        &[r(MANGANESE, 200), r(CHROME, 200), r(NICKEL, 200)],
        &[r(SALTPETER, 300), r(SULFUR, 300)],
        &[r(LEAD, 200), r(ZINC, 200)],
        &[r(COAL, 200), r(LIGNITE, 300)],
    ],
    // Ciężkie wojskowe (koks hutniczy – tylko węgiel kamienny).
    &[
        &[r(IRON, 800)],
        &[r(CHROME, 300), r(NICKEL, 300), r(TUNGSTEN, 300), r(MOLYBDENUM, 300)],
        &[r(COAL, 300)],
        &[r(RUBBER, 200)],
        &[r(ALUMINIUM, 200)],
    ],
    // Paliwo (z uniwersytetem także syntetyczne z węgla).
    &[&[r(OIL, 1000), synthetic(COAL, 2000), synthetic(LIGNITE, 3000)]],
    // Podstawowe cywilne.
    &[
        &[r(GRAIN, 600), r(RICE, 600), r(POTATOES, 600), r(CATTLE, 600)],
        &[r(COTTON, 200), r(WOOL, 200), r(FIBERS, 200)],
        &[r(WOOD, 200)],
        &[r(COAL, 200), r(LIGNITE, 300)],
        &[s(Supply::Fuel, 100), r(HORSES, 200)],
    ],
    // Luksusowe cywilne.
    &[
        &[r(COFFEE, 300), r(TEA, 300), r(TOBACCO, 300), r(SUGAR, 300)],
        &[r(FURS, 200), r(WOOL, 200)],
        &[r(COPPER, 100), r(TIN, 100)],
    ],
    // Materiały inżynieryjne.
    &[&[r(CEMENT, 500)], &[r(WOOD, 300)], &[r(IRON, 200)], &[s(Supply::Fuel, 100), r(HORSES, 200)]],
];

/// Brak składnika ograniczającego produkcję (`Production::short`).
pub const NO_SHORTAGE: u8 = u8::MAX;
/// Części na milion.
const PPM: u64 = 1_000_000;

/// Wynik tury jednej fabryki.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Production {
    /// Wyprodukowane (w tysięcznych sztuki).
    pub produced: u64,
    /// Numer składnika, którego zabrakło (`NO_SHORTAGE` – pełna produkcja).
    pub short: u8,
}

/// Zapas wejścia w puli państwa (w tysięcznych).
pub fn available(input: Input, e: &Economy) -> u64 {
    match input {
        Input::Resource(r) => e.stock[r as usize],
        Input::Supply(s) => e.supplies[s as usize],
    }
}

/// Najwięcej składników w recepturze.
pub const MAX_INGREDIENTS: usize = 5;
/// `Inputs::choice`: wejście jeszcze niewybrane.
pub const UNSET: u8 = u8::MAX;

/// Wybrane wejście każdego składnika w fabryce. Automatycznie fabryka wybiera wejście, z którego zrobi
/// najwięcej, i **trzyma się go**, aż jego zapas się skończy; wybór ręczny (`manual`, bit na składnik)
/// obowiązuje, dopóki gracz go nie zmieni.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub choice: [u8; MAX_INGREDIENTS],
    pub manual: u8,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs { choice: [UNSET; MAX_INGREDIENTS], manual: 0 }
    }
}

impl Inputs {
    pub fn is_manual(&self, ingredient: usize) -> bool {
        self.manual & (1 << ingredient) != 0
    }

    /// Wejście składnika na tę turę: ręczne – zawsze to; automatyczne – dotychczasowe, póki ma zapas,
    /// inaczej nowe z `pick`.
    pub fn resolve(&self, ingredient: usize, options: &[Alternative], universities: u32, e: &Economy) -> Option<usize> {
        let k = self.choice[ingredient] as usize;
        let valid = k < options.len() && options[k].universities <= universities;
        if valid && (self.is_manual(ingredient) || available(options[k].input, e) > 0) {
            return Some(k);
        }
        pick(options, universities, e)
    }
}

/// Które wejście składnika weźmie fabryka: to, z którego zrobi najwięcej sztuk (remis – pierwsze),
/// spośród dostępnych przy `universities` poziomach uniwersytetów (indeks w `options`).
pub fn pick(options: &[Alternative], universities: u32, e: &Economy) -> Option<usize> {
    let mut best: Option<(usize, u64)> = None;
    for (k, o) in options.iter().enumerate().filter(|(_, o)| o.universities <= universities) {
        let units = available(o.input, e) * MILLI / o.amount;
        if best.is_none_or(|(_, b)| units > b) {
            best = Some((k, units));
        }
    }
    best.map(|(k, _)| k)
}

/// Tura fabryki: chce wyprodukować `target` (w tysięcznych) zaopatrzenia `supply`. Z każdego składnika
/// bierze wejście z `inputs` (`Inputs::resolve`, wybór zapamiętany), produkuje tyle, na ile starcza
/// najrzadszego składnika, zużywa składniki z puli (zaokrąglając w górę, najwyżej cały zapas; zapisuje
/// zużycie w `resource_use` / `supply_use`). Wynik trafia do `supplies[supply]`.
pub fn produce(supply: Supply, target: u64, universities: u32, inputs: &mut Inputs, e: &mut Economy) -> Production {
    if target == 0 {
        return Production { produced: 0, short: NO_SHORTAGE };
    }
    let recipe = RECIPES[supply as usize];
    let mut picks = Vec::with_capacity(recipe.len());
    let (mut fraction, mut short) = (PPM, NO_SHORTAGE);
    for (i, options) in recipe.iter().enumerate() {
        let Some(k) = inputs.resolve(i, options, universities, e) else { continue };
        inputs.choice[i] = k as u8;
        let o = options[k];
        let need = target * o.amount / MILLI;
        let f = (available(o.input, e) * PPM).checked_div(need).map_or(PPM, |f| f.min(PPM));
        if f < fraction {
            (fraction, short) = (f, i as u8);
        }
        picks.push(o);
    }
    let produced = target * fraction / PPM;
    for o in picks {
        let take = (produced * o.amount).div_ceil(MILLI).min(available(o.input, e));
        match o.input {
            Input::Resource(r) => {
                e.stock[r as usize] -= take;
                e.resource_use[r as usize] += take;
            }
            Input::Supply(s) => {
                e.supplies[s as usize] -= take;
                e.supply_use[s as usize] += take;
            }
        }
    }
    e.supplies[supply as usize] += produced;
    e.supply_output[supply as usize] += produced;
    Production { produced, short: if fraction < PPM { short } else { NO_SHORTAGE } }
}

/// Wejście receptury dla interfejsu: surowiec albo zaopatrzenie, ilość na sztukę.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InputReport {
    pub resource: Option<u8>,
    pub supply: Option<Supply>,
    /// Jednostki na 1 sztukę zaopatrzenia.
    pub amount: f64,
    /// Wymagane poziomy uniwersytetów (0 – od startu).
    pub universities: u32,
}

/// Składnik receptury: wymienne wejścia i to, które fabryka bierze teraz (indeks w `options`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IngredientReport {
    pub options: Vec<InputReport>,
    pub used: Option<u8>,
    /// Wejście wybrane ręcznie przez gracza (inaczej – automatycznie, do wyczerpania zapasu).
    pub manual: bool,
}

/// Rodzaj zaopatrzenia w drzewku receptur.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SupplyRule {
    pub supply: Supply,
    pub name: String,
    /// Poziom centrum dowodzenia, od którego fabryki mogą go produkować.
    pub command_level: u8,
    pub ingredients: Vec<IngredientReport>,
}

/// Składniki receptury; z wyborem fabryki i gospodarką państwa – zaznaczone wejście, które fabryka
/// bierze teraz (albo weźmie w następnej turze).
pub fn ingredients(supply: Supply, universities: u32, state: Option<(&Inputs, &Economy)>) -> Vec<IngredientReport> {
    RECIPES[supply as usize]
        .iter()
        .enumerate()
        .map(|(i, options)| IngredientReport {
            options: options
                .iter()
                .map(|o| InputReport {
                    resource: match o.input {
                        Input::Resource(r) => Some(r),
                        Input::Supply(_) => None,
                    },
                    supply: match o.input {
                        Input::Supply(s) => Some(s),
                        Input::Resource(_) => None,
                    },
                    amount: o.amount as f64 / MILLI as f64,
                    universities: o.universities,
                })
                .collect(),
            used: state.and_then(|(inputs, e)| inputs.resolve(i, options, universities, e)).map(|k| k as u8),
            manual: state.is_some_and(|(inputs, _)| inputs.is_manual(i)),
        })
        .collect()
}

/// Wszystkie receptury (drzewko zaopatrzenia w oknie zapasów).
pub fn rules() -> Vec<SupplyRule> {
    SUPPLIES
        .iter()
        .map(|&s| SupplyRule {
            supply: s,
            name: SUPPLY_NAMES[s as usize].to_string(),
            command_level: COMMAND_LEVEL[s as usize],
            ingredients: ingredients(s, u32::MAX, None),
        })
        .collect()
}

/// Nazwa wejścia receptury.
pub fn input_name(input: Input) -> &'static str {
    match input {
        Input::Resource(r) => RESOURCE_NAMES[r as usize],
        Input::Supply(s) => SUPPLY_NAMES[s as usize],
    }
}

/// Premia uniwersytetów do produkcji fabryk (%) przy sumie poziomów `universities`.
pub fn university_bonus(universities: u32) -> u64 {
    UNIVERSITY_BONUS.iter().filter(|&&(at, _)| universities >= at).map(|&(_, pct)| pct).max().unwrap_or(0)
}

/// Receptura czytelnie, np. „0,5 Cement + 0,3 Drewno + 0,1 Paliwo | 0,2 Konie” (wejścia dostępne przy
/// `universities` poziomach uniwersytetów).
pub fn recipe_text(supply: Supply, universities: u32) -> String {
    let qty = |m: u64| {
        let s = format!("{}", m as f64 / 1000.0);
        s.replace('.', ",")
    };
    RECIPES[supply as usize]
        .iter()
        .map(|options| {
            options
                .iter()
                .filter(|o| o.universities <= universities)
                .map(|o| format!("{} {}", qty(o.amount), input_name(o.input)))
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_numbers_match_names() {
        let at = |n: &str| RESOURCE_NAMES.iter().position(|&x| x == n).unwrap() as u8;
        assert_eq!((GRAIN, CATTLE, HORSES, WOOD), (at("Zboża"), at("Bydło"), at("Konie"), at("Drewno")));
        assert_eq!(
            (COAL, LIGNITE, OIL, IRON),
            (at("Węgiel kamienny"), at("Węgiel brunatny"), at("Ropa naftowa"), at("Rudy żelaza"))
        );
        assert_eq!((MOLYBDENUM, COPPER, LEAD, ZINC), (at("Molibden"), at("Miedź"), at("Ołów"), at("Cynk")));
        assert_eq!((ALUMINIUM, TIN, CEMENT, SULFUR), (at("Aluminium"), at("Cyna"), at("Cement"), at("Siarka")));
        assert_eq!(
            (FIBERS, SUGAR, TEA, FURS),
            (at("Włókna roślinne"), at("Trzcina cukrowa"), at("Herbata"), at("Futra"))
        );
        assert_eq!((RICE, POTATOES, COTTON, TOBACCO), (at("Ryż"), at("Ziemniaki"), at("Bawełna"), at("Tytoń")));
        assert_eq!((COFFEE, RUBBER, WOOL, MANGANESE), (at("Kawa"), at("Kauczuk"), at("Wełna"), at("Mangan")));
        assert_eq!((CHROME, NICKEL, TUNGSTEN, SALTPETER), (at("Chrom"), at("Nikiel"), at("Wolfram"), at("Saletra")));
    }

    #[test]
    fn produce_picks_the_best_substitute_and_stops_at_the_scarcest_ingredient() {
        let mut e = Economy { supplies: [0; crate::goods::SUPPLY_COUNT], ..Default::default() };
        // 1 sztuka materiałów: 0,5 cementu, 0,3 drewna, 0,2 żelaza, 0,1 paliwa | 0,2 koni.
        e.stock[CEMENT as usize] = 250; // starczy na pół sztuki
        e.stock[WOOD as usize] = 10_000;
        e.stock[IRON as usize] = 10_000;
        e.stock[HORSES as usize] = 10_000;
        let mut inputs = Inputs::default();
        let p = produce(Supply::EngineeringMaterials, 1000, 0, &mut inputs, &mut e);
        assert_eq!(p, Production { produced: 500, short: 0 }, "cement ogranicza");
        assert_eq!(inputs.choice[3], 1, "bez paliwa – konie (zapamiętane)");
        assert_eq!(e.supplies[Supply::EngineeringMaterials as usize], 500);
        assert_eq!((e.stock[CEMENT as usize], e.stock[WOOD as usize], e.stock[HORSES as usize]), (0, 9_850, 9_900));
        assert_eq!(e.resource_use[HORSES as usize], 100, "bez paliwa – konie");
        // Paliwa na 100 sztuk, koni na 49,5: nowa fabryka wzięłaby paliwo, ale ta trzyma się koni,
        // póki ich starcza (wybór automatyczny nie skacze co turę).
        e.stock[CEMENT as usize] = 10_000;
        e.supplies[Supply::Fuel as usize] = 10_000;
        assert_eq!(pick(RECIPES[Supply::EngineeringMaterials as usize][3], 0, &e), Some(0));
        let p = produce(Supply::EngineeringMaterials, 1000, 0, &mut inputs, &mut e);
        assert_eq!(p.short, NO_SHORTAGE);
        assert_eq!((e.supplies[Supply::Fuel as usize], e.stock[HORSES as usize]), (10_000, 9_700));
        // Ręcznie: paliwo – zostaje, choćby się skończyło.
        inputs.choice[3] = 0;
        inputs.manual = 1 << 3;
        produce(Supply::EngineeringMaterials, 1000, 0, &mut inputs, &mut e);
        assert_eq!((e.supplies[Supply::Fuel as usize], e.supply_use[Supply::Fuel as usize]), (9_900, 100));
        e.supplies[Supply::Fuel as usize] = 0;
        let p = produce(Supply::EngineeringMaterials, 1000, 0, &mut inputs, &mut e);
        assert_eq!((p.produced, p.short, inputs.choice[3]), (0, 3, 0), "ręczny wybór bez zapasu – stoi");
        // Paliwo syntetyczne dopiero z uniwersytetami.
        e.stock[COAL as usize] = 4_000;
        let mut fuel = Inputs::default();
        let p = produce(Supply::Fuel, 1000, 0, &mut fuel, &mut e);
        assert_eq!(p.produced, 0, "bez ropy i bez uniwersytetów");
        let p = produce(Supply::Fuel, 1000, 10, &mut fuel, &mut e);
        assert_eq!((p.produced, e.stock[COAL as usize]), (1000, 2_000));
    }

    #[test]
    fn recipes_read_like_the_gdd() {
        assert_eq!(
            recipe_text(Supply::EngineeringMaterials, 0),
            "0,5 Cement + 0,3 Drewno + 0,2 Rudy żelaza + 0,1 Paliwo | 0,2 Konie"
        );
        assert_eq!(recipe_text(Supply::Fuel, 0), "1 Ropa naftowa");
        assert_eq!(recipe_text(Supply::Fuel, 10), "1 Ropa naftowa | 2 Węgiel kamienny | 3 Węgiel brunatny");
        assert_eq!((university_bonus(4), university_bonus(5), university_bonus(25)), (0, 10, 20));
        assert_eq!(OUTPUT_PER_LEVEL_PER_TICK * TICKS_PER_DAY, OUTPUT_PER_LEVEL_PER_DAY);
    }
}
