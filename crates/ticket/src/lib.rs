//! Bilety dołączenia do pokoju (docs/adr/0004): JWT podpisany Ed25519.
//!
//! Wystawia je meta-serwer (lobby), sprawdza game-server kluczem publicznym. Bilet niesie
//! wszystko, czego game-server potrzebuje, żeby utworzyć pokój – bez dostępu do bazy.
//!
//! Klucze w formacie PEM (PKCS#8 / SPKI), np. z OpenSSL:
//!
//!   openssl genpkey -algorithm ed25519 -out ticket.pem
//!   openssl pkey -in ticket.pem -pubout -out ticket.pub.pem

use game_core::protocol::GameConfig;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub use jsonwebtoken::errors::Error;

/// Domyślny czas życia biletu: tyle, ile trwa przejście z lobby do połączenia z grą.
pub const TTL_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TicketClaims {
    /// ID pokoju (nadaje meta-serwer).
    pub room: String,
    /// Nazwa gracza (do logów; później konto).
    pub name: String,
    /// Konfiguracja gry pokoju – z niej game-server tworzy pokój.
    pub config: GameConfig,
    /// Unikalne ID biletu – game-server przyjmuje bilet tylko raz.
    pub jti: String,
    /// Czas wystawienia i wygaśnięcia (sekundy uniksowe).
    pub iat: u64,
    pub exp: u64,
}

pub struct Signer {
    key: EncodingKey,
}

impl Signer {
    pub fn from_pem(pem: &str) -> Result<Self, Error> {
        Ok(Self { key: EncodingKey::from_ed_pem(pem.as_bytes())? })
    }

    pub fn sign(&self, claims: &TicketClaims) -> Result<String, Error> {
        jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), claims, &self.key)
    }
}

pub struct Verifier {
    key: DecodingKey,
    validation: Validation,
}

impl Verifier {
    pub fn from_pem(pem: &str) -> Result<Self, Error> {
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_required_spec_claims(&["exp"]);
        validation.leeway = 5;
        Ok(Self { key: DecodingKey::from_ed_pem(pem.as_bytes())?, validation })
    }

    /// Sprawdza podpis, algorytm i ważność (`exp`).
    pub fn verify(&self, token: &str) -> Result<TicketClaims, Error> {
        Ok(jsonwebtoken::decode::<TicketClaims>(token, &self.key, &self.validation)?.claims)
    }
}

/// Bieżący czas w sekundach uniksowych.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Klucze tylko do testów (tu i w testach game-servera – cecha `test-keys`). Generowane
/// z deterministycznego ziarna zamiast trzymać klucz prywatny PEM w repo.
#[cfg(any(test, feature = "test-keys"))]
pub mod test_keys {
    use ed25519_dalek::{
        SigningKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding},
    };

    pub struct KeyPair {
        pub private: String,
        pub public: String,
    }

    /// Para kluczy Ed25519 (PEM) z ziarna – różne ziarna = różne klucze.
    pub fn pair(seed: u8) -> KeyPair {
        let key = SigningKey::from_bytes(&[seed; 32]);
        KeyPair {
            private: key.to_pkcs8_pem(LineEnding::LF).expect("PKCS#8").to_string(),
            public: key.verifying_key().to_public_key_pem(LineEnding::LF).expect("SPKI"),
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::mapgen::{GENERATOR_VERSION, MapGenParams};

    use super::*;

    fn claims(exp: u64) -> TicketClaims {
        TicketClaims {
            room: "r1".into(),
            name: "Ala".into(),
            config: GameConfig { generator_version: GENERATOR_VERSION, map: MapGenParams { seed: 7, ..Default::default() } },
            jti: "t1".into(),
            iat: now_secs(),
            exp,
        }
    }

    #[test]
    fn signed_ticket_verifies() {
        let signer = Signer::from_pem(&test_keys::pair(1).private).unwrap();
        let verifier = Verifier::from_pem(&test_keys::pair(1).public).unwrap();
        let c = claims(now_secs() + TTL_SECS);
        assert_eq!(verifier.verify(&signer.sign(&c).unwrap()).unwrap(), c);
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let signer = Signer::from_pem(&test_keys::pair(1).private).unwrap();
        let verifier = Verifier::from_pem(&test_keys::pair(1).public).unwrap();
        let token = signer.sign(&claims(now_secs() - 60)).unwrap();
        assert!(verifier.verify(&token).is_err());
    }

    #[test]
    fn wrong_key_and_tampering_are_rejected() {
        let signer = Signer::from_pem(&test_keys::pair(1).private).unwrap();
        let token = signer.sign(&claims(now_secs() + TTL_SECS)).unwrap();
        assert!(Verifier::from_pem(&test_keys::pair(2).public).unwrap().verify(&token).is_err());

        // Podmiana treści (inny pokój) przy starym podpisie.
        let mut parts: Vec<String> = token.split('.').map(String::from).collect();
        let other = signer.sign(&TicketClaims { room: "r2".into(), ..claims(now_secs() + TTL_SECS) }).unwrap();
        parts[1] = other.split('.').nth(1).unwrap().to_string();
        assert!(Verifier::from_pem(&test_keys::pair(1).public).unwrap().verify(&parts.join(".")).is_err());
    }
}
