//! Synchronisation du maillage : ce qui circule entre voisins, et quand.
//!
//! Référence : `docs/synthese/04-architecture.md` §2 (« `sync` : routage
//! (flood + TTL + jitter + clamp densité + quotas), réconciliation par échange
//! d'inventaire, rejeu d'outbox, machine à états des statuts, collecte
//! d'enveloppes »). Répartition dans `docs/suivi/repartition-sprint2.md` §3.
//!
//! | Sous-module | US | Rôle |
//! |---|---|---|
//! | [`routing`] | US-209 | **si**, **quand** et **vers qui** un paquet reçu est relayé |
//! | `inventory` | US-210 | **quoi** échanger avec un pair qui arrive |
//! | [`status`] | US-211 | cycle de vie d'un message émis + outbox persistante |
//! | [`courier`] | US-212 | dépôt / collecte des enveloppes scellées |
//!
//! Tout `sync` reste compilable en `no_std` + `alloc` (cible ESP32) : aucune
//! I/O, ni horloge, ni aléa système. L'appelant fournit l'heure et la graine.

pub mod courier;
pub mod routing;
pub mod status;
