//! Contrat de protocole : ce que Petals envoie, le daemon doit l'accepter.
//!
//! Le client Petals et le daemon parlaient deux langages différents. Le client
//! envoyait `AllocateResources` / `GetSystemResources` / `RegisterModel` — que
//! le daemon ne connaît pas — sans le champ `v` que le protocole v1 exige sur
//! chaque message, vers `/tmp/polygoned.sock` alors que le daemon écoute
//! `~/.polygone/daemon.sock`. Aucun de ses messages n'était accepté.
//!
//! Ce test est la garantie que ça ne peut plus arriver : chaque requête
//! produite par le client passe par le **décodeur réel du daemon**, et chaque
//! réponse réelle du daemon est traduite par le client.

use polygone_petals::daemon::{
    alloc_request, allocation_from_reply, release_request, status_request, ResourceRequest,
};
use polygoned::socket::{decode, encode, encode_request, Reply, Request};

fn requirements() -> ResourceRequest {
    ResourceRequest {
        cpu_cores: 2,
        ram_gb: 4.0,
        gpu_vram_gb: None,
        concurrent_requests: 1,
        model_names: vec!["qwen3-8b".to_string()],
    }
}

#[test]
fn l_allocation_du_client_est_acceptee_par_le_daemon() {
    let line = encode_request(&alloc_request(&requirements()));

    // Le champ `v` est posé par l'encodeur du daemon : un seul exemplaire,
    // pour les deux directions.
    assert!(line.contains("\"v\":1"), "le champ v manque : {line}");

    let request = decode(line.trim_end()).expect("le daemon doit accepter la requête du client");
    match request {
        Request::Alloc {
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            client,
            storage_gb,
            ..
        } => {
            assert_eq!(cpu_cores, 2);
            assert_eq!(ram_gb, 4.0);
            assert_eq!(gpu_vram_gb, None);
            assert_eq!(storage_gb, None);
            assert_eq!(client.as_deref(), Some("petals"));
        }
        other => panic!("attendu Alloc, obtenu {other:?}"),
    }
}

#[test]
fn la_liberation_du_client_est_acceptee_par_le_daemon() {
    let line = encode_request(&release_request("alloc-42"));

    let request = decode(line.trim_end()).expect("le daemon doit accepter la libération");
    match request {
        Request::Release { allocation_id, .. } => assert_eq!(allocation_id, "alloc-42"),
        other => panic!("attendu Release, obtenu {other:?}"),
    }
}

#[test]
fn la_requete_d_etat_du_client_est_acceptee_par_le_daemon() {
    let line = encode_request(&status_request());

    let request = decode(line.trim_end()).expect("le daemon doit accepter la requête d'état");
    assert!(matches!(request, Request::Status { .. }));
}

#[test]
fn le_client_sait_lire_une_reponse_reelle_du_daemon() {
    // Une réponse fabriquée par le daemon, encodée par le daemon.
    let reply = Reply::AllocGranted {
        id: None,
        allocation_id: "alloc-1".to_string(),
        cpu_cores: 2,
        ram_gb: 4.0,
        gpu_vram_gb: None,
        storage_gb: None,
        expires_at: None,
    };
    let line = encode(&reply);

    let decoded = polygoned::socket::decode_reply(line.trim_end())
        .expect("le client doit savoir lire la réponse du daemon");
    let allocation =
        allocation_from_reply(&decoded).expect("le client doit traduire la réponse en allocation");

    assert_eq!(allocation.id, "alloc-1");
    assert_eq!(allocation.cpu_cores, 2);
    assert_eq!(allocation.ram_gb, 4.0);
    // Le daemon ne fournit pas de chemin de cgroup : on ne l'invente pas.
    assert_eq!(allocation.cpu_cgroup, None);
    assert_eq!(allocation.memory_cgroup, None);
}

#[test]
fn une_fiche_de_demande_du_client_ne_contient_que_des_champs_connus() {
    // Le protocole v1 n'a que quatre types de requete. Si le client en
    // inventait un, le daemon le refuserait avec `UnknownType` : ce test le
    // prouve avant qu'il n'arrive en production.
    let line = encode_request(&alloc_request(&requirements()));
    let value: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();

    let known = ["status_request", "ping", "alloc_request", "release"];
    let kind = value.get("type").and_then(|t| t.as_str()).unwrap();
    assert!(known.contains(&kind), "type de requête inconnu : {kind}");
}
