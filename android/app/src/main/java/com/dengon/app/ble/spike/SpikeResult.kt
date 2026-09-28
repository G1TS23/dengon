package com.dengon.app.ble.spike

/**
 * Chiffres mesurés par un échange « hello mesh » (US-103, critères
 * d'acceptation) : MTU négocié, temps d'établissement, appareil testé.
 */
data class SpikeResult(
    val role: String,
    val mtu: Int?,
    val scanToConnectMs: Long,
    val connectToExchangeMs: Long,
    val deviceModel: String,
    val androidVersion: String,
)
