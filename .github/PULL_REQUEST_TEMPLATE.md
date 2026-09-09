<!--
  Modèle de PR — dengon.
  Référence : docs/olivier/proposition-organisation-github.md §7.1 (DoD globale)
  et §7.2 (DoD par type d'US).
  Les commentaires HTML ne s'affichent pas dans la PR : les laisser en place.
-->

Closes #

<!-- Une seule US par PR : elle sera squashée en un commit. Si la PR ne clôt
     pas l'issue (critère non réalisable ici, droit manquant…), remplacer par
     « Refs # » et dire pourquoi dans « Ce qui reste ouvert ». -->

## Ce que fait cette PR

<!-- Deux à cinq lignes. Le quoi et le pourquoi, pas la liste des fichiers :
     le diff la donne déjà. -->

## Ce que la revue doit regarder en priorité

<!-- Le passage le plus discutable, celui où un deuxième regard sert vraiment.
     Une PR sans ce champ fait perdre 20 minutes au relecteur. -->

## Critères d'acceptation de l'issue

<!-- Recopier les cases de l'issue et les cocher, avec la preuve à côté :
     sortie de commande, numéro de test, capture. « Vérifié » sans preuve ne
     compte pas (DoD §7.1 point 1 : « démontrés dans la PR »). -->

- [ ]

## Definition of Done globale — §7.1

- [ ] **1.** Critères d'acceptation **tous** vérifiés, et démontrés ci-dessus.
- [ ] **2.** Tests écrits **au niveau annoncé en DoR** (unitaire / property / sim / e2e), et verts.
- [ ] **3.** CI verte sur les jobs concernés.
- [ ] **4.** **Revue demandée à une personne d'une autre `area:`** (voir plus bas).
- [ ] **5.** `docs/suivi/` à jour : entrée dans `00-journal.md`, `01-etat-du-code.md` rafraîchi, fiche `modules/` créée ou mise à jour.
- [ ] **6.** Écart vs conception consigné dans `docs/suivi/03-ecarts-conception.md` — ou « aucun écart », dit explicitement.
- [ ] **7.** Commits **Conventional Commits** (hook `.githooks/commit-msg`), PR à squasher, branche supprimée après merge.
- [ ] **8.** Aucun TODO bloquant laissé : soit fait, soit une issue de suite créée et liée ici.

## Definition of Done spécifique — §7.2

**Type d'US :** <!-- spike / contrat / module dengon-core / transport / app Android / firmware / dashboard API / dashboard web / doc -->

<details>
<summary>Ajouts par type — ne cocher que la ligne qui s'applique</summary>

- **Spike** — timebox respectée · livrable = une **décision écrite** (`docs/suivi/` + `docs/synthese/01-sujets-a-trancher.md`) · réponse oui/non · code jetable jeté.
- **Contrat** — artefact **compilable** livré (trait + mock, UDL + bouchon, schéma + fixtures, types + vecteurs) · label `contract` posé · gel annoncé en point d'équipe.
- **Module `dengon-core`** — tests unitaires **+ property tests** là où `synthese/10` §4.2 les exige · `clippy -D warnings` · couverture ≥ 85 % sur le module · compile en `no_std` si embarqué (`protocol`, `sync`, `ledger`).
- **Transport** (Android / NimBLE / btleplug) — suite de conformité `Transport` passée · testé sur ≥ 2 appareils ou 2 cartes · comportement documenté en déconnexion brutale.
- **App Android** — `assembleDebug` + tests unitaires verts · matrice d'appareils · pas de régression du service de fond (relais écran éteint ≥ 5 min).
- **Firmware** — `idf.py build` propre · tests host de `libdengon_core` verts · tests Unity cible · **vecteurs de conformité identiques au core** · survit à `esp_restart()` sans rupture de chaîne de journal.
- **Dashboard API** — `pytest` vert sur SQLite éphémère · **test négatif de redaction** (aucun `msg_uuid`, texte ou destinataire en clair stocké ni renvoyé) · idempotence sur `event_id` prouvée.
- **Dashboard web** — rendu correct sur mobile · les écrans tiennent avec des données partielles.
- **Doc / rapport** — relu par une autre personne · aucun renvoi vers un fichier inexistant · cohérent avec la table de décisions de `synthese/00`.

</details>

## Vérification

```
<!-- Les commandes RÉELLEMENT exécutées et leur résultat. Si une étape n'a pas
     pu être vérifiée, l'écrire ici plutôt que de la cocher plus haut :
     docs/suivi/README.md, règle n°7. -->
```

## Ce qui reste ouvert

<!-- TODO reportés (avec le numéro de l'issue de suite), étapes non
     vérifiables ici, dette assumée. « Rien » est une réponse valide. -->

---

<!--
  Revue croisée — §7.1 point 4 et §10.3.
  .github/CODEOWNERS demande automatiquement la bonne personne selon les
  chemins touchés, et GitHub interdit à l'auteur de s'auto-approuver au titre
  de CODEOWNERS. Tant que la protection de `main` n'est pas active (US-113),
  ce n'est qu'une suggestion : rien n'empêche techniquement de merger sans
  relecture. C'est à nous de ne pas le faire.
-->
