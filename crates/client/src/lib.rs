//! Fenêtre publique de `polygone-client`.
//!
//! Le binaire `polygone` (`src/main.rs`) est l'interface utilisateur ; ce qui
//! est exposé **ici** est ce qu'un autre programme peut réutiliser.
//!
//! - [`blocks`] — le magasin de blocs adressé par BLAKE3 (incrément 2b-12b), le
//!   premier lien pair-à-pair direct du projet.
//! - [`mesh`] — la découverte des pairs du LAN (UDP `7642`). Monté ici à
//!   l'incrément 2b-13 parce que `node status` doit **mesurer** le mesh, pas le
//!   supposer : le binaire s'y branche par `use polygone_client::mesh;`.
//! - [`node`] — le service de blocs qui obtient un bail du daemon, sert sous
//!   ce bail et s'arrête sur signal, perte du daemon ou retrait de politique.
//! - [`calc`] — le cache de calcul : la clé porte la **version du calculateur**
//!   (`BLAKE3(version ‖ entrée)`), le résultat est un bloc vérifié par son hash.
//!
//! Le reste du client (net, tui, hide, exec…) vit encore dans le binaire ; l'y
//! déplacer serait un incrément à part entière, pas une conséquence de celui-ci.

pub mod blocks;
pub mod calc;
pub mod mesh;
pub mod node;
pub mod node_isolation;
