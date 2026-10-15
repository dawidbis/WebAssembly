//! Surowce (docs/gdd/surowce.md): tabela surowców prowincji – budynek wydobycia (dedukowany z
//! surowca) i cena bazowa (pod rynek) – oraz stałe wydobycia.
//!
//! Kolejność surowców jak `RESOURCE_NAMES` w `mapgen/src/resource_table.rs` (numer = `Province::resource`).
//! Ilości surowców w rdzeniu są w tysięcznych częściach jednostki (`MILLI`, jak populacja), żeby
//! wydobycie na turę było liczbą całkowitą.

pub use crate::mapgen::resources::{GOLD, RESOURCE_COUNT, RESOURCE_NAMES};

/// Tur na dzień gry (produkcja i zużycie liczone na dzień, rozliczane co turę). Kalendarz na ekranie
/// dzieli miesiąc (300 tur) na prawdziwą liczbę dni, więc jego dzień to 9,7–10,7 tury – rdzeń trzyma
/// się równych 10.
pub const TICKS_PER_DAY: u64 = 10;
/// Wydobycie prowincji bez budynku wydobycia: 0,10 jednostki na dzień (w tysięcznych).
pub const BASE_YIELD_PER_DAY: u64 = 100;
/// Wydobycie bazowe na turę (w tysięcznych) – 0,01 jednostki.
pub const BASE_YIELD_PER_TICK: u64 = BASE_YIELD_PER_DAY / TICKS_PER_DAY;
/// Surowce nawozowe – premia do pól uprawnych (docs/gdd/surowce.md#nawozy).
pub const FERTILIZER: u8 = 31;
/// Złoto (surowe, w interfejsie ÷10) za jednostkę wydobytego złota – 50 złota z interfejsu.
pub const GOLD_PER_UNIT: u64 = 500;

/// Rodzaj zaopatrzenia (docs/gdd/zaopatrzenie.md) – wytwarzają je fabryki (etap 3) z surowców puli.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Supply {
    LightMilitary,
    StandardMilitary,
    HeavyMilitary,
    Fuel,
    BasicCivil,
    LuxuryCivil,
    EngineeringMaterials,
}

pub const SUPPLY_COUNT: usize = 7;
/// Nazwy rodzajów zaopatrzenia (kolejność `Supply`).
pub const SUPPLY_NAMES: [&str; SUPPLY_COUNT] = [
    "Lekkie wojskowe",
    "Standardowe wojskowe",
    "Ciężkie wojskowe",
    "Paliwo",
    "Podstawowe cywilne",
    "Luksusowe cywilne",
    "Materiały inżynieryjne",
];
/// Materiały inżynieryjne na start (jednostki) – starczą na pierwsze budynki, zanim powstanie fabryka.
pub const START_ENGINEERING_MATERIALS: u64 = 30;

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
}

use Extraction::{Farm, Hunters, Mine, OilWell, Plantation, Quarry, Ranch, Sawmill};

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
        }
    }
}

/// Budynek wydobycia każdego surowca (kolejność `RESOURCE_NAMES`).
#[rustfmt::skip]
pub const EXTRACTION: [Extraction; RESOURCE_COUNT] = [
    Farm, Farm, Farm, Farm, Farm, Farm, Farm, Farm, Farm, // zboża … herbata
    Plantation,                                          // kauczuk
    Ranch, Ranch, Ranch,                                 // bydło, konie, wełna
    Hunters,                                             // futra
    Sawmill,                                             // drewno
    Mine, Mine,                                          // węgiel kamienny, brunatny
    OilWell,                                             // ropa naftowa
    Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, Mine, // uran … cyna
    Mine,                                                // złoto
    Mine,                                                // surowce nawozowe
    Quarry,                                              // cement
    Mine, Mine,                                          // saletra, siarka
];

/// Cena bazowa (złoto z interfejsu za jednostkę) – punkt wyjścia rynku. Złoto: 0 (poza rynkiem);
/// uran: cena przy braku wzbogacania (rośnie z postępem uniwersytetów – docs/gdd/surowce.md).
#[rustfmt::skip]
pub const BASE_PRICE: [u32; RESOURCE_COUNT] = [
    8, 8, 6, 12, 10, 18, 14, 22, 22, 30, // zboża … kauczuk
    12, 20, 12, 24,                      // bydło, konie, wełna, futra
    8,                                   // drewno
    14, 9, 28, 10,                       // węgiel kamienny, brunatny, ropa, uran
    15, 30, 40, 40, 60, 60, 25, 18, 18, 35, 30, // rudy żelaza … cyna
    0,                                   // złoto
    12, 10, 20, 18,                      // nawozy, cement, saletra, siarka
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
        assert_eq!(
            (BASE_PRICE[at("Złoto")], BASE_PRICE[at("Molibden")], BASE_PRICE[at("Węgiel brunatny")]),
            (0, 60, 9)
        );
        assert_eq!(BASE_YIELD_PER_TICK * TICKS_PER_DAY, BASE_YIELD_PER_DAY, "wydobycie bazowe bez reszty");
    }
}
