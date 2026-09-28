package com.dengon.app.ble.spike

import java.util.UUID

/**
 * Constantes GATT du spike « hello mesh » (US-103). Reprises telles quelles de
 * `docs/powl/03-network-protocol.md` §2 et §6.1 — c'est la source de vérité du
 * protocole, même si `dengon-core` ne les définit pas encore côté Rust
 * (`protocol::consts` reste un squelette, voir `docs/suivi/02-avancement.md`).
 */
object HelloMeshConstants {
    /** Service GATT `dengon` (`"meng-den gon-v1"`). */
    val SERVICE_UUID: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310000")

    /** Write / write-without-response, pair → nœud. */
    val CHAR_RX_UUID: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310001")

    /** Notify, nœud → pair. */
    val CHAR_TX_UUID: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310002")

    /** UUID standard du Client Characteristic Configuration Descriptor (spec Bluetooth, pas dengon). */
    val CCCD_UUID: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

    /**
     * Payload de test : exactement 20 octets ASCII, comme l'exige le critère
     * d'acceptation US-103 (« deux téléphones échangent 20 octets »).
     */
    val PAYLOAD_20_BYTES: ByteArray = "hello mesh dengon!!!".toByteArray(Charsets.US_ASCII)

    /** ATT_MTU visé, `docs/powl/03-network-protocol.md` §6.3 (retombe à 23 si refusé). */
    const val REQUESTED_MTU = 517
}
