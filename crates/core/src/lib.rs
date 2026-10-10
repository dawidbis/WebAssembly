//! Deterministyczny rdzeń gry. Zasady (sprawdzane w code review):
//! - bez `std::collections::HashMap/HashSet` (losowa kolejność iteracji) – używaj Vec/BTreeMap,
//! - bez zegara i bez losowości spoza seeda,
//! - funkcje przestępne (exp, ln, pow) tylko z crate'a `libm`.

pub mod game;
pub mod lobby;
pub mod protocol;

pub use game_mapgen as mapgen;
