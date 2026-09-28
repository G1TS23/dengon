// ---------------------------------------------------------------------------
// Portage C, cas pour cas, de la suite de conformité `Transport` d'US-105
// (crates/dengon-ble/src/conformance.rs). Mêmes noms, mêmes assertions, mêmes
// messages : c'est la preuve que le transport NimBLE suit la MÊME sémantique
// d'événements que MockTransport, en attendant l'adaptateur Rust d'US-307 qui
// fera tourner la suite Rust elle-même contre ce code.
// ---------------------------------------------------------------------------
#include <stdbool.h>
#include <string.h>

#include "unity.h"

#include "banc.h"
#include "tests.h"

/* Statiques : ~1,5 Ko chacun, trop pour la pile de la tâche principale ESP32. */
static banc_t s_banc;
static lot_t  s_lot;

static const uint8_t X[] = { 'x' };

static bool
lot_contient_trame(const lot_t *lot, dengon_link_id_t lien, const uint8_t *b, size_t n, size_t *rang)
{
    for (size_t i = 0; i < lot->n; i++) {
        const dengon_transport_event_t *e = &lot->ev[i];
        if (e->kind == DENGON_EVT_FRAME_RECEIVED && e->link == lien &&
            e->u.frame.len == n && memcmp(e->u.frame.bytes, b, n) == 0) {
            if (rang != NULL) {
                *rang = i;
            }
            return true;
        }
    }
    return false;
}

static bool
lot_contient_fermeture(const lot_t *lot, dengon_link_id_t lien,
                       const dengon_disconnect_reason_t *motif, size_t *rang)
{
    for (size_t i = 0; i < lot->n; i++) {
        const dengon_transport_event_t *e = &lot->ev[i];
        if (e->kind == DENGON_EVT_PEER_DISCONNECTED && e->link == lien &&
            (motif == NULL || e->u.reason == *motif)) {
            if (rang != NULL) {
                *rang = i;
            }
            return true;
        }
    }
    return false;
}

static void
cas_poll_avant_start_est_vide(void)
{
    banc_nouveau(&s_banc);
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL_MESSAGE(0, s_lot.n,
        "conformité : poll() avant start() doit rendre un Vec vide, pas paniquer");
}

static void
cas_envoi_avant_start_est_refuse(void)
{
    banc_nouveau(&s_banc);
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_NOT_STARTED, banc_send(&s_banc, 0, X, sizeof(X)),
        "conformité : send() avant start() doit rendre NotStarted");
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_NOT_STARTED, banc_broadcast(&s_banc, X, sizeof(X)),
        "conformité : broadcast() avant start() doit rendre NotStarted");
}

static void
cas_double_start_est_refuse(void)
{
    dengon_transport_config_t cfg = banc_config();

    banc_demarre(&s_banc);
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_ALREADY_STARTED,
        dengon_tc_start(&s_banc.tc, &cfg, 3, NULL),
        "conformité : un second start() doit rendre AlreadyStarted");
}

static void
cas_connexion_remonte_un_evenement(void)
{
    dengon_link_id_t lien;
    bool vu = false;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);
    for (size_t i = 0; i < s_lot.n; i++) {
        vu |= s_lot.ev[i].kind == DENGON_EVT_PEER_CONNECTED && s_lot.ev[i].link == lien;
    }
    TEST_ASSERT_TRUE_MESSAGE(vu,
        "conformité : une connexion doit produire un PeerConnected portant son LinkId");
}

static void
cas_poll_consomme_les_evenements(void)
{
    banc_demarre(&s_banc);
    banc_connecter_un_pair(&s_banc);

    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE_MESSAGE(s_lot.n > 0,
        "conformité : le premier poll() doit livrer l'événement de connexion");
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL_MESSAGE(0, s_lot.n,
        "conformité : poll() consomme — le même événement ne doit pas ressortir");
}

static void
cas_trame_recue_remonte_intacte(void)
{
    static const uint8_t charge[] = "dengon-conformite";
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);

    banc_faire_recevoir(&s_banc, lien, charge, sizeof(charge) - 1);
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE_MESSAGE(lot_contient_trame(&s_lot, lien, charge, sizeof(charge) - 1, NULL),
        "conformité : une trame reçue doit remonter intacte, sur le lien qui l'a livrée");
}

static void
cas_envoi_vers_un_pair_connecte_reussit(void)
{
    static const uint8_t charge[] = "charge";
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_OK, banc_send(&s_banc, lien, charge, sizeof(charge) - 1),
        "conformité : send() vers un lien ouvert doit réussir");
}

static void
cas_broadcast_sans_pair_reussit(void)
{
    static const uint8_t charge[] = "personne";

    banc_demarre(&s_banc);
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_OK, banc_broadcast(&s_banc, charge, sizeof(charge) - 1),
        "conformité : broadcast() sans pair connecté est un succès, pas une erreur");
}

static void
cas_deconnexion_brutale(void)
{
    static const uint8_t tard[] = "trop tard";
    static const uint8_t fantome[] = "fantome";
    const dengon_disconnect_reason_t brutale = DENGON_DISC_BRUTALE;
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);

    banc_couper(&s_banc, lien, DENGON_DISC_BRUTALE);
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE_MESSAGE(lot_contient_fermeture(&s_lot, lien, &brutale, NULL),
        "conformité : une coupure brutale doit produire un PeerDisconnected "
        "portant DisconnectReason::Brutale");

    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_UNKNOWN_PEER, banc_send(&s_banc, lien, tard, sizeof(tard) - 1),
        "conformité : send() sur un lien mort doit rendre UnknownPeer, sans paniquer");

    banc_faire_recevoir(&s_banc, lien, fantome, sizeof(fantome) - 1);
    banc_poll(&s_banc, &s_lot);
    for (size_t i = 0; i < s_lot.n; i++) {
        TEST_ASSERT_FALSE_MESSAGE(s_lot.ev[i].link == lien,
            "conformité : plus aucun événement ne doit porter un LinkId fermé");
    }
}

static void
cas_trame_recue_avant_coupure_est_livree(void)
{
    static const uint8_t charge[] = "avant-la-coupure";
    dengon_link_id_t lien;
    size_t rang_trame = 0, rang_fermeture = 0;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);

    banc_faire_recevoir(&s_banc, lien, charge, sizeof(charge) - 1);
    banc_couper(&s_banc, lien, DENGON_DISC_BRUTALE);

    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE_MESSAGE(lot_contient_trame(&s_lot, lien, charge, sizeof(charge) - 1, &rang_trame),
        "conformité : une trame reçue avant la coupure ne doit pas être perdue — "
        "elle était complète et valide");
    TEST_ASSERT_TRUE_MESSAGE(lot_contient_fermeture(&s_lot, lien, NULL, &rang_fermeture),
        "conformité : une coupure brutale doit produire un PeerDisconnected");
    TEST_ASSERT_TRUE_MESSAGE(rang_trame < rang_fermeture,
        "conformité : la trame reçue avant la coupure doit être livrée *avant* "
        "l'événement de fermeture du lien");
}

static void
cas_deconnexion_propre_est_distinguee(void)
{
    const dengon_disconnect_reason_t propre = DENGON_DISC_PROPRE;
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);

    banc_couper(&s_banc, lien, DENGON_DISC_PROPRE);
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE_MESSAGE(lot_contient_fermeture(&s_lot, lien, &propre, NULL),
        "conformité : une déconnexion propre doit être signalée comme telle");
}

static void
cas_link_id_jamais_reutilise(void)
{
    dengon_link_id_t premier, second;

    banc_demarre(&s_banc);
    premier = banc_connecter_un_pair(&s_banc);
    banc_couper(&s_banc, premier, DENGON_DISC_BRUTALE);
    banc_poll(&s_banc, &s_lot);

    second = banc_connecter_un_pair(&s_banc);
    /* Renfort propre au C : le banc réutilise le même conn_handle, comme
       NimBLE. Le LinkId, lui, doit changer quand même. */
    TEST_ASSERT_EQUAL(banc_conn_de(&s_banc, premier), banc_conn_de(&s_banc, second));
    TEST_ASSERT_NOT_EQUAL_MESSAGE(premier, second,
        "conformité : réutiliser un LinkId attribuerait des trames au mauvais pair");
}

void
run_conformite(void)
{
    /* Sans cela, Unity attribue chaque cas au fichier de UNITY_BEGIN. */
    UnitySetTestFile(__FILE__);
    RUN_TEST(cas_poll_avant_start_est_vide);
    RUN_TEST(cas_envoi_avant_start_est_refuse);
    RUN_TEST(cas_double_start_est_refuse);
    RUN_TEST(cas_connexion_remonte_un_evenement);
    RUN_TEST(cas_poll_consomme_les_evenements);
    RUN_TEST(cas_trame_recue_remonte_intacte);
    RUN_TEST(cas_envoi_vers_un_pair_connecte_reussit);
    RUN_TEST(cas_broadcast_sans_pair_reussit);
    RUN_TEST(cas_deconnexion_brutale);
    RUN_TEST(cas_trame_recue_avant_coupure_est_livree);
    RUN_TEST(cas_deconnexion_propre_est_distinguee);
    RUN_TEST(cas_link_id_jamais_reutilise);
    lot_liberer(&s_lot);
}
