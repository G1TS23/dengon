# docs/synthese — Contexte global du projet

Ce dossier **regroupe en une source unique** l'intégralité de la recherche et de
la conception produites séparément dans [`../powl/`](../powl/),
[`../oswin/`](../oswin/) et [`../olivier/`](../olivier/). But : qu'une nouvelle
conversation (ou un nouvel arrivant) ait **tout le contexte** sans relire les
34 fichiers d'origine, et connaisse les **décisions déjà prises**.

**Point d'entrée : [`00-contexte-global.md`](00-contexte-global.md)** (résumé
exécutif + table des décisions + guide de lecture).

| Fichier | Contenu |
| --- | --- |
| [`00-contexte-global.md`](00-contexte-global.md) | Point d'entrée : résumé exécutif, **table des décisions de conception** (A/B/C), principes directeurs, origine des sources, guide de lecture. |
| [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md) | **Transversal.** Chaque décision avec ses options, arguments et statut ; les points encore ouverts. La mémoire du « pourquoi ». |
| [`02-probleme-et-besoins.md`](02-probleme-et-besoins.md) | Concept, besoins métier, cas d'usage, périmètre MVP ; analyse de besoins (10 axes) ; points produit / UX. |
| [`03-etat-de-lart.md`](03-etat-de-lart.md) | Recherche : DTN / RFC 9171, routage, Bitchat, Bridgefy, Bluetooth Mesh, analyse « blockchain ». |
| [`04-architecture.md`](04-architecture.md) | Modules `dengon-core`, trait `Transport`, couches, découpage du dépôt, déploiement. |
| [`05-protocole-et-trame.md`](05-protocole-et-trame.md) | Couches L1–L4, constantes, format de paquet octet par octet, types de paquets, fragmentation, routage, comportement MVP. |
| [`06-securite.md`](06-securite.md) | Menace, identité & clés, Noise `XX`/`X`, `recipient_tag`, journal chaîné signé, chiffrement base locale, primitives. |
| [`07-cycle-de-vie-et-statuts.md`](07-cycle-de-vie-et-statuts.md) | Statuts, machine à états, émission/réception, multi-saut, reconnexion, expiration. |
| [`08-relais-esp32.md`](08-relais-esp32.md) | Matériel WROOM, firmware, budgets mémoire, connexion HTTPS au VPS, pannes, journalisation. |
| [`09-dashboard-et-donnees.md`](09-dashboard-et-donnees.md) | Dashboard FastAPI + SSE + SQLite, ingestion HTTPS, écrans, API, catalogue d'événements, schémas de données. |
| [`10-benchmarks-mvp-tests.md`](10-benchmarks-mvp-tests.md) | Décisions techniques, comparatifs, DoD, lots de livraison, risques, feuilles de route, tests & CI. |
| [`11-glossaire-biblio-annexes.md`](11-glossaire-biblio-annexes.md) | Glossaire, bibliographie, correspondance fichier ↔ sources, annexes « pour mémoire » (format `olivier` v0.1, dashboard `olivier` v0.2). |

## Périmètre

- **Recherche & conception uniquement.** L'état réel du code et le méta-projet
  (échéance, équipe, conventions de commit) ne sont **pas** dans ce dossier :
  ils restent dans [`../suivi/`](../suivi/) et `CLAUDE.md`.

## Hiérarchie des sources

- [`../powl/`](../powl/), [`../oswin/`](../oswin/) et [`../olivier/`](../olivier/)
  restent la **matière première** (recherche sourcée, benchmarks, brouillons
  v0.1–v0.3) et ne sont **pas modifiés**. Le rapport / la soutenance peuvent
  s'y appuyer.
- **Les décisions vivent maintenant ici** : la table de `00-contexte-global.md`
  et le corps des fichiers `02`–`11` pour la conception retenue,
  `01-sujets-a-trancher.md` pour les options, arguments et alternatives
  écartées.

## Règle de mise à jour

Quand une **décision d'équipe** modifie un point :

1. mettre à jour la fiche correspondante de `01-sujets-a-trancher.md` (statut,
   décision) ;
2. répercuter dans le fichier thématique concerné (`02`–`11`) — le texte doit
   énoncer la conception **décidée**, pas une option ouverte ;
3. mettre à jour la table des décisions de `00-contexte-global.md`.

Quand `powl/`, `oswin/` ou `olivier/` évoluent (nouvelle recherche), répercuter
dans la section thématique concernée (la correspondance fichier ↔ sources est
en `11-glossaire-biblio-annexes.md`).

Une fois le code commencé, le suivi de ce qui est **réellement implémenté** se
fait dans [`../suivi/`](../suivi/), pas ici.
