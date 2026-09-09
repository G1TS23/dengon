-- Migration initiale du dashboard dengon.
--
-- Squelette volontairement minimal (US-110) : une seule table d'atterrissage
-- pour les batchs bruts. Aucune validation, aucune normalisation ici.
--
-- La validation de schéma + signature Ed25519 (US-216), la table `events`
-- normalisée et les projections `messages` / `nodes` / `links` / `message_hops`
-- (US-217) arrivent en sprint 2.
-- Réf : docs/synthese/09-dashboard-et-donnees.md §3 (ingestion) et §11.2 (schéma cible).

CREATE TABLE raw_batches (
    batch_id     TEXT    PRIMARY KEY,     -- uuid4 généré à la réception
    received_ms  INTEGER NOT NULL,        -- horodatage serveur, ms depuis epoch
    remote_addr  TEXT,                    -- IP source (debug uniquement)
    content_type TEXT,                    -- en-tête Content-Type reçu
    event_count  INTEGER,                 -- nb d'événements deviné, NULL si indéterminé
    body         TEXT    NOT NULL         -- corps JSON, tel que reçu
);

CREATE INDEX idx_raw_batches_received ON raw_batches (received_ms DESC);
