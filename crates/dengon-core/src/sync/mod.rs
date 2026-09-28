//! Synchronisation entre nœuds — `docs/synthese/04-architecture.md` §2.
//!
//! Quatre sous-modules prévus au sprint 2 (répartition dans
//! `docs/suivi/repartition-sprint2.md` §3) :
//!
//! - `routing` — **si** un paquet est relayé (TTL, dedup, anti-inondation) — US-209 ;
//! - `inventory` — **quoi** échanger entre deux pairs qui se rencontrent — US-210 ;
//! - [`status`] — cycle de vie d'un message émis + outbox persistante — US-211 ;
//! - `courier` — dépôt / collecte d'enveloppes scellées — US-212.
//!
//! Tout le module reste compilable en `no_std` + `alloc` (cible ESP32).

pub mod status;
