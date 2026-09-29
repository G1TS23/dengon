// ---------------------------------------------------------------------------
// dengon_adv — voir l'en-tête.
// ---------------------------------------------------------------------------
#include <string.h>

#include "dengon_adv.h"

void
dengon_adv_build_mfg(const uint8_t local_peer_id[8], uint8_t flags, uint8_t out[DENGON_ADV_MFG_LEN])
{
    /* Le Company ID part en little-endian sur l'air (Core Spec, Vol 3). */
    out[0] = (uint8_t)(DENGON_ADV_COMPANY_ID & 0xFFu);
    out[1] = (uint8_t)(DENGON_ADV_COMPANY_ID >> 8);
    memcpy(&out[2], local_peer_id, DENGON_ADV_PEER_PREFIX_LEN);
    out[6] = flags;
}

bool
dengon_adv_parse_mfg(const uint8_t *data, size_t len, dengon_adv_info_t *out)
{
    uint16_t company;

    /* Plus long que 7 octets : accepté, pour qu'une version future puisse
       ajouter des champs en queue sans rendre les relais existants aveugles.
       6 octets (sans flags) : format de l'app Android, flags à 0 (US-312). */
    if (data == NULL || len < DENGON_ADV_MFG_MIN_LEN) {
        return false;
    }

    company = (uint16_t)(data[0] | (data[1] << 8));
    if (company != DENGON_ADV_COMPANY_ID) {
        return false;
    }

    memcpy(out->peer_prefix, &data[2], DENGON_ADV_PEER_PREFIX_LEN);
    out->flags = len >= DENGON_ADV_MFG_LEN ? data[6] : 0;
    return true;
}

bool
dengon_adv_should_initiate(const uint8_t local_peer_id[8],
                           const uint8_t remote_prefix[DENGON_ADV_PEER_PREFIX_LEN],
                           const uint8_t local_addr[6], const uint8_t remote_addr[6])
{
    int cmp = memcmp(local_peer_id, remote_prefix, DENGON_ADV_PEER_PREFIX_LEN);

    if (cmp == 0) {
        cmp = memcmp(local_addr, remote_addr, 6);
    }
    return cmp < 0;
}

bool
dengon_adv_relay_should_connect(const uint8_t local_peer_id[8], const dengon_adv_info_t *remote,
                                const uint8_t local_addr[6], const uint8_t remote_addr[6])
{
    if ((remote->flags & DENGON_ADV_F_RELAY) == 0) {
        return false; /* un téléphone : c'est lui qui initie */
    }
    return dengon_adv_should_initiate(local_peer_id, remote->peer_prefix, local_addr, remote_addr);
}
