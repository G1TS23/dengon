"""Connexion SQLite et exécution des migrations.

Système volontairement minimal : les migrations sont déclarées dans
``app/migrations.py`` (``MIGRATIONS``), appliquées dans l'ordre de version et
tracées dans la table ``schema_migrations``. Pas d'Alembic — surdimensionné
pour un squelette.

`run_migrations` est **atomique et sûr en concurrence** : chaque migration
s'applique dans une transaction ``BEGIN IMMEDIATE`` (les instructions DDL *et*
l'enregistrement dans ``schema_migrations`` réussissent ou échouent ensemble),
et la présence de la version est revérifiée sous verrou — deux processus qui
démarrent en même temps (``uvicorn --workers N``) n'appliquent pas la migration
deux fois. « Sûr » signifie : jamais appliquée deux fois, jamais à moitié
appliquée — pas « démarre toujours ». Si le verrou reste tenu plus longtemps
que ``busy_timeout`` (5 s), ``BEGIN IMMEDIATE`` lève ``sqlite3.OperationalError``
et le worker échoue à démarrer plutôt que de continuer sur un état incertain ;
la levée n'est pas rattrapée ici, volontairement (retour de revue #59, round 2).

**Vérifié avec de vrais process OS**, pas seulement des threads
(``tests/test_api.py::test_migrations_are_safe_across_processes``, retour de
revue #59, round 4, point d'OswinFreyr : cette affirmation n'était jusque-là
couverte que par un test multi-thread dans un seul process). Ce test a
d'ailleurs révélé un bug réel dans ``connect()`` : voir
``_set_wal_mode_with_retry`` ci-dessous.
"""

from __future__ import annotations

import sqlite3
import threading
import time
from collections.abc import Iterator
from contextlib import contextmanager

from .config import db_path
from .migrations import MIGRATIONS

# Nombre de tentatives pour le passage en WAL sous contention (voir
# `_set_wal_mode_with_retry`) — 20 × 50 ms = 1 s de marge, largement sous
# busy_timeout (5 s) qui couvre le reste des opérations.
_WAL_MODE_MAX_ATTEMPTS = 20
_WAL_MODE_RETRY_DELAY_S = 0.05


def _is_retryable_lock_error(exc: sqlite3.OperationalError) -> bool:
    """Ne re-tente que sur un verrou (`SQLITE_BUSY`/`SQLITE_LOCKED`), pas sur
    n'importe quelle `OperationalError` — un `disk I/O error` ou un `unable
    to open database file` ne se résoudra pas en re-tentant 1 s plus tard, et
    les rattraper masquerait un vrai problème derrière 20 tentatives inutiles
    (retour de revue #59, round 5, point 5 d'OswinFreyr).

    `sqlite_errorcode` est le code **étendu** (ex. `2067` pour
    `SQLITE_CONSTRAINT_UNIQUE`), pas le code de base à comparer directement à
    `SQLITE_BUSY`/`SQLITE_LOCKED` : masqué par `0xFF`, `2067 & 0xFF == 19`
    (`SQLITE_CONSTRAINT`), pas `SQLITE_BUSY`. Sans le masque, le filtre
    laissait passer sans re-tentative `SQLITE_BUSY_RECOVERY` (261),
    `SQLITE_BUSY_SNAPSHOT` (517) ou `SQLITE_LOCKED_SHAREDCACHE` (262) —
    `BUSY_RECOVERY` en particulier est précisément ce que SQLite renvoie
    quand un autre process est en train de récupérer le WAL, le scénario de
    démarrage `--workers N` que ce retry vise (retour de revue #59, round 6,
    point 1 d'OswinFreyr — vérifié en local : une violation `UNIQUE` donne
    `sqlite_errorcode == 2067`, confirmant que le round 5 comparait le
    mauvais niveau de code)."""
    code = getattr(exc, "sqlite_errorcode", None)
    return code is not None and (code & 0xFF) in (sqlite3.SQLITE_BUSY, sqlite3.SQLITE_LOCKED)


def _set_wal_mode_with_retry(conn: sqlite3.Connection) -> None:
    """Bascule en WAL avec re-tentatives manuelles.

    `busy_timeout` ne protège PAS ce PRAGMA de façon fiable : passer en WAL
    prend un verrou distinct du verrou d'écriture habituel, et plusieurs
    process qui ouvrent le même fichier neuf en même temps (``uvicorn
    --workers N`` au tout premier démarrage) peuvent chacun lever
    ``sqlite3.OperationalError: database is locked`` ici, y compris avec
    `busy_timeout` déjà réglé — reproduit de façon fiable avec 5 process
    lancés en même temps sur un fichier neuf (découvert en écrivant
    `test_migrations_are_safe_across_processes`, retour de revue #59, round
    4, point d'OswinFreyr sur la sûreté multi-process : la docstring du
    module affirmait cette sûreté sans qu'aucun test multi-process ne
    l'exerce). Piège SQLite connu, pas un bug applicatif : la solution usuelle
    est une re-tentative manuelle courte, `busy_timeout` couvrant le reste.
    """
    for attempt in range(1, _WAL_MODE_MAX_ATTEMPTS + 1):
        try:
            conn.execute("PRAGMA journal_mode = WAL")
            return
        except sqlite3.OperationalError as exc:
            if attempt == _WAL_MODE_MAX_ATTEMPTS or not _is_retryable_lock_error(exc):
                raise
            time.sleep(_WAL_MODE_RETRY_DELAY_S)


def connect() -> sqlite3.Connection:
    """Ouvre une connexion sur la base configurée.

    ``autocommit`` désactivé (``isolation_level = None``) : les transactions
    sont **explicites** (``BEGIN`` / ``COMMIT``), comportement identique de
    Python 3.11 à 3.13 et seul moyen de rendre le DDL transactionnel.

    ``check_same_thread=False`` : la connexion ouverte au démarrage de l'app
    est réutilisée pour les écritures depuis le threadpool
    (``run_in_threadpool``), donc depuis un thread différent de celui qui l'a
    ouverte — ``sqlite3.Connection`` n'est pas sûre en usage concurrent non
    protégé, même avec ce réglage. C'est pour ça que `connect()` seule ne
    suffit pas côté appelant : voir `LockedConnection` ci-dessous, qui couple
    la connexion à son verrou plutôt que de compter sur la discipline de
    l'appelant.
    """
    conn = sqlite3.connect(db_path(), isolation_level=None, check_same_thread=False)
    try:
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA busy_timeout = 5000")  # attendre un verrou plutôt qu'échouer aussitôt
        _set_wal_mode_with_retry(conn)  # lecteurs et écrivain ne se bloquent pas
        conn.execute("PRAGMA foreign_keys = ON")
    except BaseException:
        # `sqlite3.connect()` a déjà ouvert le fichier : si une étape
        # suivante échoue (WAL épuise ses tentatives, par ex.), personne
        # d'autre ne fermera cette connexion — `lifespan` garde `conn =
        # connect()` hors de son `try` (retour de revue #59, round 5, point 4
        # d'OswinFreyr : même classe de fuite que le point 2 du round 4,
        # déplacée d'un cran).
        conn.close()
        raise
    return conn


class LockedConnection:
    """Couple une connexion SQLite à son verrou.

    Avant : ``app.state.db_conn`` (la connexion) et ``app.state.db_lock`` (le
    verrou) étaient deux attributs séparés, et l'invariant « jamais l'un sans
    l'autre » n'existait qu'en commentaire dans ``main.py`` — une future route
    (ex. US-217) aurait pu appeler ``request.app.state.db_conn.execute(...)``
    directement, en oubliant le verrou, et rien ne l'en aurait empêché (retour
    de revue #59, round 4, point d'OswinFreyr). ``execute()`` est la seule
    façon d'utiliser la connexion depuis l'extérieur de ce module : le verrou
    est tenu pour toute la durée de l'appel, structurellement, pas par
    convention.
    """

    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn
        self._lock = threading.Lock()

    def execute(self, sql: str, parameters: tuple = ()) -> sqlite3.Cursor:
        """Pour un `INSERT`/`UPDATE`/`DELETE` isolé : le verrou n'est tenu
        que le temps de l'instruction. **Ne pas appeler `.fetchall()` sur le
        `Cursor` renvoyé** — il n'est plus protégé une fois le verrou relâché
        (retour de revue #59, round 5, point 2 d'OswinFreyr). Pour un
        `SELECT` dont on lit les lignes, ou pour plusieurs instructions liées
        (ex. `BEGIN`/`COMMIT`), utiliser [`locked`] à la place.
        """
        with self._lock:
            return self._conn.execute(sql, parameters)

    @contextmanager
    def locked(self) -> Iterator[sqlite3.Connection]:
        """Tient le verrou pour toute la durée du bloc `with` — pour un
        `SELECT` suivi d'un `fetchall()`, ou une séquence d'instructions liées
        (la connexion est en autocommit, donc un `BEGIN`/`COMMIT` à plusieurs
        étapes serait sinon entrelacé entre threads).

        Si le bloc lève entre un `BEGIN` et un `COMMIT`, la transaction est
        annulée avant de propager l'exception : sans ça, elle resterait
        ouverte sur la connexion **partagée** — les écritures suivantes
        (ex. `_store_raw_batch`) s'y agrégeraient sans jamais être commitées,
        et le prochain `BEGIN` lèverait `cannot start a transaction within a
        transaction` (retour de revue #59, round 6, point 2 d'OswinFreyr :
        piège qu'un futur appelant, US-217, aurait rencontré).
        """
        with self._lock:
            try:
                yield self._conn
            except BaseException:
                if self._conn.in_transaction:
                    self._conn.rollback()
                raise

    def close(self) -> None:
        with self._lock:
            self._conn.close()


def _ensure_schema_migrations(conn: sqlite3.Connection) -> None:
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations ("
        "  version    INTEGER PRIMARY KEY,"
        "  name       TEXT    NOT NULL,"
        "  applied_at TEXT    NOT NULL DEFAULT (datetime('now'))"
        ")"
    )


def _applied_versions(conn: sqlite3.Connection) -> set[int]:
    return {row["version"] for row in conn.execute("SELECT version FROM schema_migrations")}


def run_migrations(conn: sqlite3.Connection) -> list[int]:
    """Applique les migrations manquantes. Retourne les versions nouvellement appliquées."""
    _ensure_schema_migrations(conn)
    newly_applied: list[int] = []

    # Calculée une fois : re-sélectionner à chaque itération serait un
    # SELECT par migration, y compris pour celles déjà appliquées qu'on ne
    # fait que sauter (retour de revue #59, point 5) — coût O(n) au
    # démarrage dès que MIGRATIONS grossit. Seule la revérification sous
    # verrou ci-dessous a besoin d'une lecture fraîche.
    deja_appliquees = _applied_versions(conn)

    for version, name, statements in sorted(MIGRATIONS, key=lambda m: m[0]):
        if version in deja_appliquees:
            continue

        # BEGIN IMMEDIATE : prend le verrou d'écriture tout de suite, donc un
        # second processus attend ici puis reverra la version comme appliquée.
        # Volontairement HORS du try/except ci-dessous : si le verrou n'est
        # pas obtenu avant busy_timeout, aucune transaction n'est ouverte, donc
        # rien à ROLLBACK — l'y inclure lèverait une seconde OperationalError
        # ("no transaction is active") qui masquerait la vraie cause. Dans ce
        # cas (verrou tenu > 5 s, deux workers démarrés en même temps par
        # exemple), le worker échoue à démarrer plutôt que de continuer sur un
        # état incertain — comportement voulu, pas un bug (retour de revue
        # #59, round 2 : la levée n'était pas documentée comme volontaire).
        conn.execute("BEGIN IMMEDIATE")

        try:
            if version in _applied_versions(conn):  # revérification sous verrou
                conn.execute("ROLLBACK")
                continue
            for statement in statements:
                conn.execute(statement)
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
                (version, name),
            )
            conn.execute("COMMIT")
        except Exception:
            # SQLite annule lui-même la transaction sur certaines erreurs
            # (`SQLITE_FULL`, `SQLITE_IOERR`, `SQLITE_NOMEM`) : un `ROLLBACK`
            # explicite sans garde lèverait alors `OperationalError: cannot
            # rollback - no transaction is active`, qui remonterait à la
            # place de l'erreur d'origine (« database or disk is full »),
            # reléguée dans `__context__`. Même garde que
            # `LockedConnection.locked()` (retour de revue #59, round 8,
            # point 2 d'OswinFreyr).
            if conn.in_transaction:
                conn.execute("ROLLBACK")
            raise

        newly_applied.append(version)

    return newly_applied
