//! Rynek w grze (docs/gdd/rynek.md): potrzeby fabryk, pojemność handlu, trasy dostaw i codzienne
//! rozliczenie. Reguły (ceny, dopasowanie, koszty dostawy) są w `market.rs`.

use serde::{Deserialize, Serialize};

use super::{Game, Phase, factory_target};
use crate::{
    buildings::Building,
    economy::{MILLI, NationRates},
    factories::{Input, RECIPES},
    goods::{FERTILIZER, RESOURCE_COUNT, RESOURCE_NAMES, TICKS_PER_DAY},
    market::{
        self, FACTORY_CAPACITY, GROUP, GROUP_COUNT, GROUP_NAMES, Network, Order, PORT_CAPACITY, PPM, Port, Route,
        SHARE_DAYS, TRANSIT_PPM, Tree,
    },
    protocol::PlayerId,
};

/// Trasy liczone z właścicieli i budynków (poza hashem – wynikają ze stanu); przeliczane, gdy zmienią się
/// właściciele albo budynki (`key`).
pub(super) struct RouteCache {
    key: (u32, u32),
    /// Trasy lądem do fabryk importera (`importer[n - 1]`; bez fabryk – do stolicy).
    importer: Vec<Tree>,
    /// Trasy lądem do portów państwa (`from_ports[n - 1]`).
    from_ports: Vec<Tree>,
    ports: Vec<Vec<Port>>,
    /// Dostawa handlarzy spoza mapy do każdego państwa (części na milion wartości; `None` – bez portu).
    trader_ppm: Vec<Option<u64>>,
    diagonal: u64,
}

/// Rynek dla interfejsu (z punktu widzenia gracza).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MarketReport {
    pub groups: Vec<TradeGroupReport>,
    pub resources: Vec<MarketResourceReport>,
    /// Wczorajsze transakcje gracza.
    pub deals: Vec<DealReport>,
    /// Udział w rynku (0–1): wartość eksportu gracza / całego eksportu (średnie z ok. 30 dni).
    pub share: f64,
    /// Dostawa od handlarzy spoza mapy (część wartości, 0–1; `None` – bez portu, bez handlu z nimi).
    pub trader_delivery: Option<f64>,
    /// Przepustowość handlu na dzień (jednostki każdego surowca): porty i fabryki.
    pub capacity: f64,
    pub port_capacity: f64,
    /// Obrót całego rynku wczoraj (złoto z interfejsu).
    pub turnover: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TradeGroupReport {
    pub name: String,
    pub resources: Vec<u8>,
}

/// Surowiec na rynku (ilości w jednostkach, ceny w złocie z interfejsu).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MarketResourceReport {
    pub resource: u8,
    pub name: String,
    pub price: f64,
    pub base_price: f64,
    pub prev_price: f64,
    /// Bilans (% przepustowości: −100 import … +100 eksport).
    pub setting: i8,
    /// Dzienna potrzeba fabryk (informacja) i zapas.
    pub need: f64,
    pub stock: f64,
    /// Średnie zamówienia wszystkich graczy na dzień: popyt i podaż.
    pub demand: f64,
    pub supply: f64,
    /// Udział gracza w eksporcie surowca (0–1, średnie z ok. 30 dni) i wczorajszy obrót surowcem.
    pub share: f64,
    pub turnover: f64,
}

/// Transakcja gracza z wczoraj.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DealReport {
    pub resource: u8,
    pub buy: bool,
    /// Drugi gracz (`None` – handlarze spoza mapy).
    pub partner: Option<PlayerId>,
    pub amount: f64,
    pub value: f64,
    pub delivery: f64,
    /// Gracze, którym zapłacono cło tranzytowe.
    pub transit: Vec<PlayerId>,
}

/// Potrzeba fabryk (informacja w oknie rynku) i przepustowość handlu jednego państwa (tysięczne jednostki
/// na dzień; ta sama dla każdego surowca).
struct Needs {
    need: [u64; RESOURCE_COUNT],
    capacity: u64,
    port_capacity: u64,
}

impl Game {
    /// Bilans surowca (`percent` −100…100) – tylko gracz z państwem.
    pub(super) fn set_trade_resource(&mut self, player: PlayerId, resource: u8, percent: i8) {
        let Some(n) = self.owners.nation_of(player) else { return };
        if (resource as usize) < RESOURCE_COUNT && market::tradable(resource as usize) {
            self.economy[n as usize - 1].trade.settings[resource as usize] = percent.clamp(-100, 100);
        }
    }

    /// Dzień handlu: zamówienia państw, dopasowanie każdego surowca (rosnąco po numerze), nowe ceny.
    pub(super) fn market_day(&mut self, rates: &[NationRates]) {
        for e in &mut self.economy {
            e.trade.deals.clear();
            e.trade.exports_today = 0;
        }
        if self.economy.is_empty() {
            return;
        }
        self.refresh_routes();
        let needs: Vec<Needs> = (1..=self.economy.len() as u8).map(|n| self.needs(n, &rates[n as usize - 1])).collect();
        let sources = self.sources();
        let mut economy = std::mem::take(&mut self.economy);
        let (mut demand_total, mut supply_total) = ([0; RESOURCE_COUNT], [0; RESOURCE_COUNT]);
        let routes = self.routes.as_ref().expect("trasy policzone");
        for r in (0..RESOURCE_COUNT).filter(|&r| market::tradable(r)) {
            let (mut offers, mut demands) = (Vec::new(), Vec::new());
            for (k, (e, nd)) in economy.iter().zip(&needs).enumerate() {
                let nation = k as u8 + 1;
                // Przepustowość × bilans%, niezależnie od produkcji fabryk (eksport – najwyżej cały zapas).
                let setting = e.trade.setting(r) as i64;
                let amount = nd.capacity * setting.unsigned_abs() / 100;
                if setting > 0 && amount.min(e.stock[r]) > 0 {
                    offers.push(Order { nation, amount: amount.min(e.stock[r]) });
                } else if setting < 0 && amount > 0 {
                    demands.push(Order { nation, amount });
                }
            }
            demand_total[r] = demands.iter().map(|o| o.amount).sum();
            supply_total[r] = offers.iter().map(|o| o.amount).sum();
            if offers.is_empty() && demands.is_empty() {
                continue;
            }
            let start = self.market.day as usize % demands.len().max(1);
            let route = |e: u8, i: u8| self.route(routes, &sources[r], e, i);
            market::match_resource(
                market::Book {
                    resource: r as u8,
                    price: self.market.price[r],
                    offers: &offers,
                    demands: &demands,
                    start,
                },
                &routes.trader_ppm,
                &route,
                &mut economy,
            );
        }
        // Obrót i udział w rynku każdego surowca z dzisiejszych transakcji.
        let mut turnover = [0; RESOURCE_COUNT];
        for e in &mut economy {
            e.trade.exports = market::ema(e.trade.exports, e.trade.exports_today, SHARE_DAYS);
            let mut sold = [0; RESOURCE_COUNT];
            for d in &e.trade.deals {
                if !d.buy {
                    sold[d.resource as usize] += d.value;
                }
                if !d.buy || d.partner == 0 {
                    turnover[d.resource as usize] += d.value;
                }
            }
            for (avg, today) in e.trade.resource_exports.iter_mut().zip(sold) {
                *avg = market::ema(*avg, today, SHARE_DAYS);
            }
        }
        self.market.turnover = turnover;
        self.economy = economy;
        self.market.update(&demand_total, &supply_total);
    }

    /// Potrzeba fabryk (receptury × produkcja na dzień, wejście, które fabryka weźmie) i nawozów pól oraz
    /// przepustowość handlu państwa `n` (porty – znacząco, fabryki).
    fn needs(&self, n: u8, rates: &NationRates) -> Needs {
        let e = &self.economy[n as usize - 1];
        let mut need = [0; RESOURCE_COUNT];
        need[FERTILIZER as usize] += rates.fertilizer_need;
        let (mut factories, mut ports) = (0u64, 0u64);
        for (k, b) in self.buildings.iter().enumerate() {
            let province = k as u16 + 1;
            let level = b.level(Building::Port) as u64;
            if level > 0 && self.controller(province, Building::Port) == Some(n) {
                ports += level;
            }
            let level = b.level(Building::Factory) as u64;
            if level == 0 || self.controller(province, Building::Factory) != Some(n) {
                continue;
            }
            factories += level;
            if b.retool > 0 {
                continue;
            }
            let per_day = factory_target(level, rates) * TICKS_PER_DAY;
            for (i, options) in RECIPES[b.product as usize].iter().enumerate() {
                let Some(k) = b.inputs.resolve(i, options, rates.universities, e) else { continue };
                if let Input::Resource(res) = options[k].input {
                    need[res as usize] += per_day * options[k].amount / MILLI;
                }
            }
        }
        // Porty: `PORT_CAPACITY` jednostek na poziom × pokrycie obsady; fabryki: `FACTORY_CAPACITY` na poziom.
        let port_capacity = PORT_CAPACITY * ports * MILLI * rates.staff_ppm / PPM;
        Needs { need, capacity: port_capacity + FACTORY_CAPACITY * factories * MILLI, port_capacity }
    }

    /// Prowincje wydobycia każdego surowca (`sources[surowiec]`: państwo kontrolujące wydobycie, prowincja).
    pub(super) fn sources(&self) -> Vec<Vec<(u8, u16)>> {
        let mut sources = vec![Vec::new(); RESOURCE_COUNT];
        for (k, p) in self.map.provinces.iter().enumerate() {
            if let Some(n) = self.controller(k as u16 + 1, Building::Extraction) {
                sources[p.resource as usize].push((n, k as u16 + 1));
            }
        }
        sources
    }

    /// Najtańsza trasa dostawy od eksportera `e` do importera `i`: z najbliższej jego prowincji wydobycia
    /// (bez wydobycia – ze stolicy) lądem do fabryki importera albo lądem do portu, morzem do portu
    /// importera nad tym samym morzem i lądem do fabryki. `None` – brak trasy.
    pub(super) fn route(&self, routes: &RouteCache, sources: &[(u8, u16)], e: u8, i: u8) -> Option<Route> {
        let mut from: Vec<u16> = sources.iter().filter(|s| s.0 == e).map(|s| s.1).collect();
        if from.is_empty() {
            from.extend(self.capital(e));
        }
        let (imp, by_sea) = (&routes.importer[i as usize - 1], &routes.from_ports[e as usize - 1]);
        let mut best: Option<Route> = None;
        let mut consider = |cost: u64, transit: Vec<u8>| {
            if best.as_ref().is_none_or(|b| cost < b.cost_ppm) {
                best = Some(Route { cost_ppm: cost, transit });
            }
        };
        for &s in &from {
            if let Some((cost, transit)) = imp.leg(s, e, i) {
                consider(cost, transit);
            }
            let k = s as usize - 1;
            if by_sea.land[k] == u64::MAX {
                continue;
            }
            let Some(pe) = routes.ports[e as usize - 1].iter().find(|p| p.province == by_sea.root[k]) else { continue };
            for q in &routes.ports[i as usize - 1] {
                let qk = q.province as usize - 1;
                let Some(sea) = market::sea_leg(pe, q, routes.diagonal) else { continue };
                if imp.land[qk] == u64::MAX {
                    continue;
                }
                let mut transit: Vec<u8> = by_sea.nations[k].iter().chain(&imp.nations[qk]).copied().collect();
                transit.sort_unstable();
                transit.dedup();
                transit.retain(|&t| t != e && t != i);
                let cost = by_sea.land[k] + sea + imp.land[qk] + TRANSIT_PPM * transit.len() as u64;
                consider(cost, transit);
            }
        }
        best
    }

    /// Przelicza trasy, jeśli od ostatniego razu zmienili się właściciele albo budynki.
    pub(super) fn refresh_routes(&mut self) {
        let key = (self.owners.revision(), self.buildings_revision);
        if self.routes.as_ref().is_some_and(|c| c.key == key) {
            return;
        }
        let (w, h) = (self.map.width as i64, self.map.height as i64);
        let diagonal = ((w * w + h * h) as u64).isqrt();
        let count = self.map.provinces.len();
        let network = Network {
            graph: &self.graph,
            centers: self.bounds.iter().map(|b| ((b[0] + b[2]) as i64 / 2, (b[1] + b[3]) as i64 / 2)).collect(),
            owner: (1..=count as u16).map(|p| self.owners.province_owner(p).unwrap_or(0)).collect(),
            infra: self.buildings.iter().map(|b| b.level(Building::Infrastructure)).collect(),
            diagonal,
        };
        let nations = self.economy.len() as u8;
        let ports: Vec<Vec<Port>> = (1..=nations).map(|n| self.ports(n)).collect();
        let mut importer = Vec::new();
        let mut from_ports = Vec::new();
        let mut trader_ppm = Vec::new();
        for n in 1..=nations {
            let mut roots: Vec<u16> = (1..=count as u16)
                .filter(|&p| {
                    self.buildings(p).level(Building::Factory) > 0 && self.controller(p, Building::Factory) == Some(n)
                })
                .collect();
            if roots.is_empty() {
                roots.extend(self.capital(n));
            }
            let tree = network.tree(&roots, n);
            let port_roots: Vec<u16> = ports[n as usize - 1].iter().map(|p| p.province).collect();
            // Handlarze: rejs od krawędzi mapy do naszego portu i lądem do fabryki albo lądem za stałą stawkę.
            // Port bez drogi lądem do fabryk (inny ląd) – transport lądem za pełną stawkę `LAND_PPM`.
            let by_sea = ports[n as usize - 1].iter().map(|p| {
                let land = tree.leg(p.province, 0, n).map_or(market::LAND_PPM, |(cost, _)| cost);
                market::trader_sea_leg(p, w, h, diagonal) + land
            });
            trader_ppm.push(by_sea.min());
            from_ports.push(network.tree(&port_roots, n));
            importer.push(tree);
        }
        self.routes = Some(RouteCache { key, importer, from_ports, ports, trader_ppm, diagonal });
    }

    /// Porty państwa (kontrolowane, z miejscem nad morzem).
    fn ports(&self, n: u8) -> Vec<Port> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        (1..=self.map.provinces.len() as u16)
            .filter_map(|p| {
                let level = self.buildings(p).level(Building::Port);
                if level == 0 || self.controller(p, Building::Port) != Some(n) {
                    return None;
                }
                let [x, y] = self.map.provinces[p as usize - 1].slots[crate::mapgen::slots::SLOT_PORT];
                if [x, y] == crate::mapgen::slots::NO_SLOT {
                    return None;
                }
                let body = self.water.sea_next_to(y as usize * w + x as usize, w, h)?;
                Some(Port { province: p, x: x as i64, y: y as i64, body, level })
            })
            .collect()
    }

    /// Rynek z punktu widzenia gracza `viewer` (`None` – gracz bez państwa albo przed grą właściwą).
    pub fn market_report(&self, viewer: PlayerId) -> Option<MarketReport> {
        let n = self.owners.nation_of(viewer).filter(|_| self.phase == Phase::Playing)?;
        let rates = self.rates();
        let needs = self.needs(n, &rates[n as usize - 1]);
        let e = &self.economy[n as usize - 1];
        let gold = |raw: u64| raw as f64 / (10 * market::PRICE_SCALE) as f64;
        let units = |m: u64| m as f64 / MILLI as f64;
        let player = |k: u8| self.owners.nations()[k as usize - 1];
        let total: u64 = self.economy.iter().map(|e| e.trade.exports).sum();
        let resources = (0..RESOURCE_COUNT)
            .filter(|&r| market::tradable(r))
            .map(|r| MarketResourceReport {
                resource: r as u8,
                name: RESOURCE_NAMES[r].to_string(),
                price: gold(self.market.price[r]),
                base_price: gold(market::base_price(r)),
                prev_price: gold(self.market.prev_price[r]),
                setting: e.trade.setting(r),
                need: units(needs.need[r]),
                stock: units(e.stock[r]),
                demand: units(self.market.demand[r]),
                supply: units(self.market.supply[r]),
                share: {
                    let all: u64 = self.economy.iter().map(|e| e.trade.resource_exports[r]).sum();
                    if all == 0 { 0.0 } else { e.trade.resource_exports[r] as f64 / all as f64 }
                },
                turnover: self.market.turnover[r] as f64 / 10.0,
            })
            .collect();
        Some(MarketReport {
            groups: (0..GROUP_COUNT)
                .map(|g| TradeGroupReport {
                    name: GROUP_NAMES[g].to_string(),
                    resources: (0..RESOURCE_COUNT as u8)
                        .filter(|&r| GROUP[r as usize] as usize == g && market::tradable(r as usize))
                        .collect(),
                })
                .collect(),
            resources,
            deals: e
                .trade
                .deals
                .iter()
                .map(|d| DealReport {
                    resource: d.resource,
                    buy: d.buy,
                    partner: (d.partner > 0).then(|| player(d.partner)),
                    amount: units(d.amount),
                    value: d.value as f64 / 10.0,
                    delivery: d.delivery as f64 / 10.0,
                    transit: d.transit.iter().map(|&t| player(t)).collect(),
                })
                .collect(),
            share: if total == 0 { 0.0 } else { e.trade.exports as f64 / total as f64 },
            turnover: self.market.turnover.iter().sum::<u64>() as f64 / 10.0,
            // Dostawa z tras ostatniego dnia handlu; port zbudowany od tamtej pory – handlarze już dostępni.
            trader_delivery: self
                .routes
                .as_ref()
                .and_then(|r| r.trader_ppm.get(n as usize - 1).copied().flatten())
                .or_else(|| (!self.ports(n).is_empty()).then_some(0))
                .map(|p| p as f64 / PPM as f64),
            capacity: units(needs.capacity),
            port_capacity: units(needs.port_capacity),
        })
    }
}

/// Port na mapie (kliknięcie ikony portu): zasięg desantu i handel morski.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PortReport {
    pub province: u16,
    /// Kafel portu (miejsce na mapie).
    pub x: u32,
    pub y: u32,
    pub level: u8,
    /// Gracz kontrolujący port (`None` – niczyj).
    pub owner: Option<PlayerId>,
    /// Zasięg desantu (kafle drogi po morzu; `None` – cała mapa).
    pub range: Option<u32>,
    /// Porty innych graczy nad tym samym morzem – partnerzy handlu morskiego.
    pub partners: Vec<PortPartner>,
}

/// Port partnera: koszt dostawy z tego portu do jego fabryk (część wartości: rejs z opłatami portowymi,
/// ląd do fabryk i cło) i gracze, którym płaci się cło tranzytowe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PortPartner {
    pub province: u16,
    pub player: PlayerId,
    pub x: u32,
    pub y: u32,
    pub delivery: f64,
    pub transit: Vec<PlayerId>,
}

impl Game {
    /// Port w prowincji `province` dla mapy (`None` – bez portu).
    pub fn port_report(&self, province: u16) -> Option<PortReport> {
        if province == 0 || province as usize > self.map.provinces.len() {
            return None;
        }
        let level = self.buildings(province).level(Building::Port);
        let owner = self.controller(province, Building::Port);
        let w = self.map.width as usize;
        let [x, y] = self.map.provinces[province as usize - 1].slots[crate::mapgen::slots::SLOT_PORT];
        if level == 0 || [x, y] == crate::mapgen::slots::NO_SLOT {
            return None;
        }
        let ours = Port {
            province,
            x: x as i64,
            y: y as i64,
            body: self.water.sea_next_to(y as usize * w + x as usize, w, self.map.height as usize)?,
            level,
        };
        let diagonal = ((self.map.width as u64).pow(2) + (self.map.height as u64).pow(2)).isqrt();
        let player = |k: u8| self.owners.nations()[k as usize - 1];
        let mut partners = Vec::new();
        for n in 1..=self.economy.len() as u8 {
            if Some(n) == owner {
                continue;
            }
            for q in self.ports(n) {
                let Some(sea) = market::sea_leg(&ours, &q, diagonal) else { continue };
                // Ląd od portu partnera do jego fabryk (trasy z ostatniego dnia handlu).
                let leg = self
                    .routes
                    .as_ref()
                    .and_then(|r| r.importer.get(n as usize - 1)?.leg(q.province, owner.unwrap_or(0), n));
                let (land, transit) = leg.unwrap_or((0, Vec::new()));
                partners.push(PortPartner {
                    province: q.province,
                    player: player(n),
                    x: q.x as u32,
                    y: q.y as u32,
                    delivery: (sea + land) as f64 / PPM as f64,
                    transit: transit.into_iter().map(player).collect(),
                });
            }
        }
        let range = crate::war::PORT_LANDING_RANGE[level.min(3) as usize];
        Some(PortReport {
            province,
            x: x as u32,
            y: y as u32,
            level,
            owner: owner.map(player),
            range: (range != u32::MAX).then_some(range),
            partners,
        })
    }

    /// Kafle morza w zasięgu desantu z portu w prowincji (pusto – bez portu).
    pub fn port_reach(&self, province: u16) -> Vec<u32> {
        let Some(report) = self.port_report(province) else { return Vec::new() };
        let w = self.map.width as usize;
        let [x, y] = self.map.provinces[province as usize - 1].slots[crate::mapgen::slots::SLOT_PORT];
        self.sea_reach(y as usize * w + x as usize, report.range.unwrap_or(u32::MAX))
    }
}
