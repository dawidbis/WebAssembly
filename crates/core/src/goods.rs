//! Surowce (docs/gdd/surowce.md): tabela surowców prowincji – budynek wydobycia (dedukowany z
//! surowca) i cena bazowa (pod rynek) – oraz stałe wydobycia.
//!
//! Kolejność surowców jak `RESOURCE_NAMES` w `mapgen/src/resource_table.rs` (numer = `Province::resource`).
//! Ilości surowców w rdzeniu są w tysięcznych częściach jednostki (`MILLI`, jak populacja), żeby
//! wydobycie na turę było liczbą całkowitą.

use serde::{Deserialize, Serialize};

pub use crate::mapgen::resources::{GOLD, RESOURCE_COUNT, RESOURCE_NAMES, id};

/// Tur na dzień gry (produkcja i zużycie liczone na dzień, rozliczane co turę). Kalendarz na ekranie
/// dzieli miesiąc (300 tur) na prawdziwą liczbę dni, więc jego dzień to 9,7–10,7 tury – rdzeń trzyma
/// się równych 10.
pub const TICKS_PER_DAY: u64 = 10;
/// Wydobycie prowincji bez budynku wydobycia: 0,10 jednostki na dzień (w tysięcznych).
pub const BASE_YIELD_PER_DAY: u64 = 100;
/// Wydobycie bazowe na turę (w tysięcznych) – 0,01 jednostki.
pub const BASE_YIELD_PER_TICK: u64 = BASE_YIELD_PER_DAY / TICKS_PER_DAY;
/// Surowce nawozowe – premia do pól uprawnych (docs/gdd/surowce.md#nawozy).
pub const FERTILIZER: u8 = id::FERTILIZER;
/// Złoto (surowe, w interfejsie ÷10) za jednostkę wydobytego złota – 50 złota z interfejsu.
pub const GOLD_PER_UNIT: u64 = 500;

/// Rodzaj zaopatrzenia (docs/gdd/zaopatrzenie.md) – wytwarzają je fabryki z surowców puli
/// (receptury: `factories.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Supply {
    LightMilitary,
    StandardMilitary,
    HeavyMilitary,
    Fuel,
    BasicCivil,
    LuxuryCivil,
    EngineeringMaterials,
    /// Prowiant – jedzenie ludności (fabryki: skrobia + białko + sól).
    Provisions,
}

pub const SUPPLY_COUNT: usize = 8;

/// Wszystkie rodzaje w kolejności indeksów (`Supply as usize`).
pub const SUPPLIES: [Supply; SUPPLY_COUNT] = [
    Supply::LightMilitary,
    Supply::StandardMilitary,
    Supply::HeavyMilitary,
    Supply::Fuel,
    Supply::BasicCivil,
    Supply::LuxuryCivil,
    Supply::EngineeringMaterials,
    Supply::Provisions,
];
/// Nazwy rodzajów zaopatrzenia (kolejność `Supply`).
pub const SUPPLY_NAMES: [&str; SUPPLY_COUNT] = [
    "Lekkie wojskowe",
    "Standardowe wojskowe",
    "Ciężkie wojskowe",
    "Paliwo",
    "Podstawowe cywilne",
    "Luksusowe cywilne",
    "Materiały inżynieryjne",
    "Prowiant",
];
/// Zaopatrzenie na start (jednostki, indeks = `Supply as usize`): materiały inżynieryjne na pierwsze budynki,
/// paliwo, a prowiant i podstawowe cywilne – na pierwsze miesiące gry.
pub const START_SUPPLIES: [u64; SUPPLY_COUNT] = [0, 0, 0, 50, 100, 0, 100, 120];
/// Sól na start (jednostki) – starcza na 300 sztuk prowiantu, zanim gracz zdobędzie złoże albo rynek
/// (sól nie ma zamiennika, a ma ją ~1,6% prowincji).
pub const START_SALT: u64 = 30;

/// Budynek wydobycia – wynika z surowca prowincji (gracz buduje po prostu „wydobycie”).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extraction {
    Farm,
    Plantation,
    Ranch,
    Hunters,
    Sawmill,
    Mine,
    OilWell,
    Quarry,
    Fishery,
}

use Extraction::{Farm, Fishery, Hunters, Mine, OilWell, Plantation, Quarry, Ranch, Sawmill};

impl Extraction {
    /// Nazwa budynku wydobycia w interfejsie.
    pub fn name(self) -> &'static str {
        match self {
            Farm => "Pola uprawne",
            Plantation => "Plantacja",
            Ranch => "Ranczo",
            Hunters => "Myśliwi",
            Sawmill => "Tartak",
            Mine => "Kopalnia",
            OilWell => "Szyb naftowy",
            Quarry => "Kamieniołom",
            Fishery => "Łowisko",
        }
    }
}

/// Budynek wydobycia każdego surowca (kolejność `RESOURCE_NAMES`).
#[rustfmt::skip]
pub const EXTRACTION: [Extraction; RESOURCE_COUNT] = [
    Farm, Farm, Farm, Farm,                              // zboża, ryż, kukurydza, ziemniaki
    Farm, Farm, Farm, Farm, Farm, Farm,                  // bawełna … herbata
    Plantation,                                          // kauczuk
    Ranch, Ranch, Ranch,                                 // bydło, trzoda chlewna, drób
    Fishery,                                             // ryby
    Ranch, Ranch,                                        // konie, wełna
    Hunters,                                             // futra
    Sawmill,                                             // drewno
    Mine, Mine,                                          // węgiel kamienny, brunatny
    OilWell,                                             // ropa naftowa
    Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, // rudy żelaza … złoto
    Mine,                                                // surowce nawozowe
    Quarry,                                              // cement
    Mine, Mine, Mine,                                    // saletra, siarka, sól
];

impl Extraction {
    /// Rolnictwo, hodowla i połowy – budowa i rozbudowa tylko za złoto (bez materiałów inżynieryjnych).
    pub fn gold_only(self) -> bool {
        matches!(self, Farm | Plantation | Ranch | Fishery)
    }
}

/// Cena bazowa (złoto z interfejsu za jednostkę) – punkt wyjścia rynku. Złoto: 0 (poza rynkiem);
/// rzadkie używki (kawa, herbata) i metale stopowe – drożej.
#[rustfmt::skip]
pub const BASE_PRICE: [u32; RESOURCE_COUNT] = [
    8, 8, 7, 6,                          // zboża, ryż, kukurydza, ziemniaki
    12, 10, 18, 14, 30, 30, 30,          // bawełna … kauczuk
    12, 11, 10, 9, 20, 12, 24,           // bydło, trzoda chlewna, drób, ryby, konie, wełna, futra
    8,                                   // drewno
    14, 9, 28,                           // węgiel kamienny, brunatny, ropa
    15, 30, 40, 40, 60, 25, 18, 35, 30,  // rudy żelaza … cyna
    0,                                   // złoto
    12, 10, 20, 18, 16,                  // nawozy, cement, saletra, siarka, sól
];

/// Nazwa surowca (`Province::resource`).
pub fn name(resource: u8) -> &'static str {
    RESOURCE_NAMES.get(resource as usize).copied().unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_follow_the_resource_order() {
        let at = |n: &str| RESOURCE_NAMES.iter().position(|&x| x == n).unwrap();
        assert_eq!(name(GOLD), "Złoto");
        assert_eq!(name(FERTILIZER), "Surowce nawozowe");
        assert_eq!(EXTRACTION[at("Kauczuk")], Plantation);
        assert_eq!(EXTRACTION[at("Futra")], Hunters);
        assert_eq!(EXTRACTION[at("Drewno")], Sawmill);
        assert_eq!(EXTRACTION[at("Ropa naftowa")], OilWell);
        assert_eq!(EXTRACTION[at("Cement")], Quarry);
        assert_eq!(EXTRACTION[at("Wełna")], Ranch);
        assert_eq!(EXTRACTION[at("Herbata")], Farm);
        assert_eq!(EXTRACTION[at("Siarka")], Mine);
        assert_eq!(EXTRACTION[at("Sól")], Mine);
        assert_eq!(EXTRACTION[at("Ryby")], Fishery);
        assert_eq!((EXTRACTION[at("Trzoda chlewna")], EXTRACTION[at("Kukurydza")]), (Ranch, Farm));
        assert_eq!(
            (
                BASE_PRICE[at("Złoto")],
                BASE_PRICE[at("Wolfram")],
                BASE_PRICE[at("Węgiel brunatny")],
                BASE_PRICE[at("Sól")]
            ),
            (0, 60, 9, 16)
        );
        assert_eq!(BASE_YIELD_PER_TICK * TICKS_PER_DAY, BASE_YIELD_PER_DAY, "wydobycie bazowe bez reszty");
    }
}
