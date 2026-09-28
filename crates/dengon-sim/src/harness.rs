//! Harness : N nœuds, chacun avec son [`SimTransport`] et un
//! [`Comportement`], avancés pas à pas sur l'horloge virtuelle du réseau.
//!
//! Le comportement est **injecté** : aujourd'hui [`Inondation`] (un relais
//! naïf, suffisant pour exercer le réseau simulé), demain le vrai nœud
//! `dengon-core` quand `sync::routing` (US-209) et la façade `api` (US-301)
//! existeront. Le harness n'a pas à changer pour ça.
//!
//! Ordre d'exécution d'un pas, identique à chaque exécution : les nœuds sont
//! servis par indice croissant, puis l'horloge avance. C'est ce qui rend la
//! trace reproductible.

use std::collections::BTreeSet;

use dengon_ble::{LinkId, Transport, TransportConfig, TransportError, TransportEvent};

use crate::reseau::{Horodate, NoeudId, ReseauPartage, SimTransport};

/// Ce qu'un nœud simulé fait de son transport.
pub trait Comportement {
    /// Un pas de simulation : lire `ctx.transport().poll()` et réagir.
    fn pas(&mut self, ctx: &mut Contexte<'_>);

    /// L'application locale veut émettre `charge`.
    fn emettre(&mut self, charge: &[u8], ctx: &mut Contexte<'_>);
}

/// Ce que le harness prête à un [`Comportement`] le temps d'un appel.
#[derive(Debug)]
pub struct Contexte<'a> {
    noeud: NoeudId,
    maintenant_ms: u64,
    transport: &'a mut SimTransport,
    livres: Vec<Vec<u8>>,
}

impl Contexte<'_> {
    /// L'indice du nœud servi.
    #[must_use]
    pub const fn noeud(&self) -> NoeudId {
        self.noeud
    }

    /// Date virtuelle courante, en ms.
    #[must_use]
    pub const fn maintenant_ms(&self) -> u64 {
        self.maintenant_ms
    }

    /// Le transport du nœud.
    pub fn transport(&mut self) -> &mut SimTransport {
        self.transport
    }

    /// Remet `octets` à l'application du nœud (tracé comme `Livraison`).
    pub fn livrer(&mut self, octets: &[u8]) {
        self.livres.push(octets.to_vec());
    }
}

struct Noeud {
    transport: SimTransport,
    comportement: Box<dyn Comportement>,
    livres: Vec<Vec<u8>>,
}

/// Une simulation : un [`ReseauPartage`] et ses nœuds.
pub struct Simulation {
    reseau: ReseauPartage,
    noeuds: Vec<Noeud>,
}

impl core::fmt::Debug for Simulation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Simulation")
            .field("reseau", &self.reseau)
            .field("noeuds", &self.noeuds.len())
            .finish()
    }
}

impl Simulation {
    /// Une simulation vide dont l'aléa est initialisé par `graine`.
    #[must_use]
    pub fn new(graine: u64) -> Self {
        Self {
            reseau: ReseauPartage::new(graine),
            noeuds: Vec::new(),
        }
    }

    /// Ajoute un nœud, démarre son transport avec `cfg` et rend son indice.
    ///
    /// # Errors
    ///
    /// Celles de [`Transport::start`].
    pub fn ajouter_noeud(
        &mut self,
        comportement: Box<dyn Comportement>,
        cfg: TransportConfig,
    ) -> Result<NoeudId, TransportError> {
        let mut transport = self.reseau.ajouter_noeud();
        transport.start(cfg)?;
        self.noeuds.push(Noeud {
            transport,
            comportement,
            livres: Vec::new(),
        });
        Ok(self.noeuds.len() - 1)
    }

    /// Ajoute `n` nœuds identiques ; le `local_peer_id` de chacun est dérivé
    /// de son indice.
    ///
    /// # Errors
    ///
    /// Celles de [`Transport::start`].
    pub fn ajouter_noeuds(
        &mut self,
        n: usize,
        fabrique: impl Fn() -> Box<dyn Comportement>,
    ) -> Result<(), TransportError> {
        for _ in 0..n {
            let i = self.noeuds.len();
            let cfg = TransportConfig {
                local_peer_id: (i as u64).to_be_bytes(),
                ..TransportConfig::default()
            };
            self.ajouter_noeud(fabrique(), cfg)?;
        }
        Ok(())
    }

    /// Le réseau, pour piloter topologie, partitions et horloge.
    #[must_use]
    pub const fn reseau(&self) -> &ReseauPartage {
        &self.reseau
    }

    /// Nombre de nœuds.
    #[must_use]
    pub fn nb_noeuds(&self) -> usize {
        self.noeuds.len()
    }

    fn servir(&mut self, i: NoeudId, charge: Option<&[u8]>) {
        let maintenant_ms = self.reseau.maintenant_ms();
        let Some(n) = self.noeuds.get_mut(i) else {
            return;
        };
        let mut ctx = Contexte {
            noeud: i,
            maintenant_ms,
            transport: &mut n.transport,
            livres: Vec::new(),
        };
        match charge {
            Some(c) => n.comportement.emettre(c, &mut ctx),
            None => n.comportement.pas(&mut ctx),
        }
        let livres = ctx.livres;
        for l in livres {
            self.reseau.tracer_livraison(i, &l);
            n.livres.push(l);
        }
    }

    /// Fait émettre `charge` par le nœud `noeud` (ignoré s'il n'existe pas).
    pub fn emettre(&mut self, noeud: NoeudId, charge: &[u8]) {
        self.servir(noeud, Some(charge));
    }

    /// Un pas : chaque nœud est servi une fois, puis l'horloge avance de
    /// `duree_ms`.
    pub fn pas(&mut self, duree_ms: u64) {
        for i in 0..self.noeuds.len() {
            self.servir(i, None);
        }
        self.reseau.avancer(duree_ms);
    }

    /// Enchaîne des pas de `pas_ms` jusqu'à atteindre `t_ms`.
    pub fn executer_jusqu_a(&mut self, t_ms: u64, pas_ms: u64) {
        let pas_ms = pas_ms.max(1);
        while self.reseau.maintenant_ms() < t_ms {
            self.pas(pas_ms);
        }
    }

    /// Ce que l'application du nœud `noeud` a reçu, dans l'ordre.
    #[must_use]
    pub fn livres(&self, noeud: NoeudId) -> &[Vec<u8>] {
        self.noeuds.get(noeud).map_or(&[], |n| &n.livres)
    }

    /// La trace complète de l'exécution.
    #[must_use]
    pub fn trace(&self) -> Vec<Horodate> {
        self.reseau.trace()
    }
}

/// Relais par **inondation** : le comportement de démonstration.
///
/// - Tout message jamais vu est livré à l'application et renvoyé à tous les
///   autres voisins (dédup par contenu exact).
/// - À chaque nouvelle connexion, tout ce qui est connu est poussé au nouveau
///   voisin : une réconciliation naïve, qui suffit à faire converger deux
///   moitiés de réseau après une partition.
///
/// C'est **un bouchon de protocole**, pas `dengon-core` : ni TTL, ni
/// signature, ni inventaire. Il sera remplacé par le vrai nœud quand
/// `sync::routing` (US-209) existera — voir `03-ecarts-conception.md`.
#[derive(Debug, Default)]
pub struct Inondation {
    connus: BTreeSet<Vec<u8>>,
    liens: BTreeSet<LinkId>,
}

impl Comportement for Inondation {
    fn pas(&mut self, ctx: &mut Contexte<'_>) {
        for ev in ctx.transport().poll() {
            match ev {
                TransportEvent::PeerConnected { peer_link_id, .. } => {
                    self.liens.insert(peer_link_id);
                    for m in &self.connus {
                        let _ = ctx.transport().send(peer_link_id, m);
                    }
                }
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    self.liens.remove(&peer_link_id);
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => {
                    if self.connus.insert(bytes.clone()) {
                        ctx.livrer(&bytes);
                        for l in self.liens.iter().filter(|l| **l != peer_link_id) {
                            let _ = ctx.transport().send(*l, &bytes);
                        }
                    }
                }
            }
        }
    }

    fn emettre(&mut self, charge: &[u8], ctx: &mut Contexte<'_>) {
        if self.connus.insert(charge.to_vec()) {
            let _ = ctx.transport().broadcast(charge);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reseau::ParametresLien;

    fn chaine(n: usize, params: ParametresLien) -> Simulation {
        let mut sim = Simulation::new(7);
        sim.ajouter_noeuds(n, || Box::new(Inondation::default()))
            .unwrap();
        for i in 1..n {
            sim.reseau().relier(i - 1, i, params);
        }
        sim
    }

    #[test]
    fn multi_saut_sur_une_chaine() {
        let mut sim = chaine(
            4,
            ParametresLien {
                latence_ms: 20,
                ..ParametresLien::default()
            },
        );
        sim.pas(10);
        sim.emettre(0, b"bonjour");
        sim.executer_jusqu_a(500, 10);
        for n in 1..4 {
            assert_eq!(sim.livres(n), [b"bonjour".to_vec()], "nœud {n}");
        }
        assert!(sim.livres(0).is_empty(), "l'émetteur ne se livre pas");
        assert_eq!(sim.nb_noeuds(), 4);
        assert!(sim.livres(99).is_empty());
    }

    #[test]
    fn la_partition_puis_la_reunion_font_converger() {
        let mut sim = chaine(4, ParametresLien::default());
        sim.pas(10);
        sim.reseau().partitionner(&[vec![0, 1], vec![2, 3]]);
        sim.emettre(0, b"gauche");
        sim.emettre(3, b"droite");
        sim.executer_jusqu_a(200, 10);
        assert!(sim.livres(3).iter().all(|m| m != b"gauche"));

        sim.reseau().reunir();
        sim.executer_jusqu_a(400, 10);
        for n in 0..4 {
            let vus: BTreeSet<_> = sim.livres(n).iter().cloned().collect();
            let attendus: BTreeSet<Vec<u8>> = [b"gauche".to_vec(), b"droite".to_vec()]
                .into_iter()
                .filter(|m| !((n == 0 && m == b"gauche") || (n == 3 && m == b"droite")))
                .collect();
            assert_eq!(vus, attendus, "nœud {n}");
        }
    }

    #[test]
    fn meme_graine_meme_trace() {
        let run = || {
            let mut sim = chaine(
                5,
                ParametresLien {
                    latence_ms: 5,
                    gigue_ms: 30,
                    perte_pour_mille: 300,
                },
            );
            sim.pas(1);
            for i in 0u8..20 {
                sim.emettre(usize::from(i % 5), &[i]);
                sim.pas(7);
            }
            sim.executer_jusqu_a(2000, 5);
            sim.trace()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn le_debug_de_la_simulation_ne_panique_pas() {
        let sim = Simulation::new(0);
        assert!(format!("{sim:?}").contains("Simulation"));
        sim.reseau().avancer(1);
        let mut sim = sim;
        sim.emettre(3, b"personne"); // nœud inexistant : ignoré
        sim.executer_jusqu_a(5, 0); // pas nul ramené à 1
        assert_eq!(sim.reseau().maintenant_ms(), 5);
    }
}
