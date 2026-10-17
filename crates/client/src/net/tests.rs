use super::*;
use crate::identity::LocalIdentity;

/// Build a signed KEM envelope exactly like `send_network` does.
fn signed_kem_json(
    session: &str,
    sender: &LocalIdentity,
    to: &str,
    ts: u64,
    output: &crate::msg::SendOutput,
    file_name: Option<&str>,
) -> String {
    let from = node_id(sender);
    let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
    let sig = sender.sign_signer().unwrap().sign(&canonical_bytes(
        session,
        &from,
        to,
        ts,
        output.kem_ct.as_bytes(),
        &ciphertext,
    ));
    let name_ct = match (&output.session_key, file_name) {
        (Some(k), Some(n)) => Some(symmetric::encrypt(n.as_bytes(), k).unwrap()),
        _ => None,
    };
    let env = NetEnvelope {
        kind: "fragment".into(),
        from,
        to: to.into(),
        session: session.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: Some(sig.as_bytes().to_vec()),
        signer: Some(sender.sign_pk_hex.clone()),
        name_ct,
        ts: Some(ts),
    };
    serde_json::to_string(&env).unwrap()
}

fn frag_json(session: &str, from: &str, to: &str, idx: u8, payload: &[u8]) -> String {
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: from.into(),
        to: to.into(),
        session: session.into(),
        seq: idx as u64,
        typ: "frag".into(),
        idx,
        threshold: 4,
        total: 7,
        payload: payload.to_vec(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };
    serde_json::to_string(&env).unwrap()
}

/// Fixed clock for tests: passes the freshness window.
fn test_now() -> u64 {
    1_800_000_000
}

static PEER_TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn peer_test_path(label: &str) -> std::path::PathBuf {
    let id = PEER_TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "polygone-peers-{label}-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("peers.json")
}

fn remove_peer_test_path(path: &std::path::Path) {
    #[cfg(not(unix))]
    let _ = std::fs::remove_file(peer_lock_path(path));
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
#[cfg(windows)]
#[test]
fn peers_store_first_launch_creates_missing_parent() {
    let seed = peer_test_path("windows-first-launch");
    let root = seed.parent().unwrap().to_path_buf();
    std::fs::remove_dir(&root).unwrap();
    let path = root.join(".polygone").join("peers.json");
    assert!(!path.parent().unwrap().exists());

    initialize_peers_at(&path).unwrap();
    assert!(path.parent().unwrap().is_dir());
    assert!(load_peers_at(&path).unwrap().is_empty());

    remove_peer_test_path(&path);
    let _ = std::fs::remove_dir(root);
}

#[test]
fn peers_store_bootstrap_is_explicit_and_later_disappearance_fails_closed() {
    let path = peer_test_path("bootstrap");
    let error = load_peers_at(&path).unwrap_err();
    assert!(error.to_string().contains("initialiser-pairs"));
    assert!(
        !path.exists(),
        "un chargement absent ne crée jamais de map vide"
    );

    initialize_peers_at(&path).unwrap();
    assert!(load_peers_at(&path).unwrap().is_empty());
    std::fs::remove_file(&path).unwrap();

    let error = load_peers_at(&path).unwrap_err();
    assert!(error.to_string().contains("disparu"));
    assert!(
        !path.exists(),
        "une disparition ultérieure ne doit pas réinitialiser le store"
    );
    remove_peer_test_path(&path);
}

#[test]
fn valid_legacy_store_is_migrated_without_silent_reinitialization() {
    let path = peer_test_path("migration");
    let expected = HashMap::from([(String::from("legacy-node"), String::from("legacy-key"))]);
    std::fs::write(&path, serde_json::to_vec(&expected).unwrap()).unwrap();

    assert_eq!(load_peers_at(&path).unwrap(), expected);
    assert!(
        peer_marker_path(&path).exists(),
        "la migration durable un marqueur"
    );
    std::fs::remove_file(&path).unwrap();
    let error = load_peers_at(&path).unwrap_err();
    assert!(error.to_string().contains("disparu"));
    remove_peer_test_path(&path);
}

#[test]
fn corrupt_json_and_invalid_utf8_are_errors_not_empty_trust() {
    let path = peer_test_path("corrupt-json");
    initialize_peers_at(&path).unwrap();
    std::fs::write(&path, b"{broken").unwrap();
    assert!(load_peers_at(&path).is_err());
    std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
    assert!(load_peers_at(&path).is_err());
    remove_peer_test_path(&path);
}

#[cfg(unix)]
#[test]
fn denied_store_read_is_propagated() {
    use std::os::unix::fs::PermissionsExt;

    let path = peer_test_path("read-denied");
    initialize_peers_at(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let error = load_peers_at(&path).unwrap_err();
    assert!(error.to_string().contains("Permission denied"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    remove_peer_test_path(&path);
}

#[cfg(unix)]
#[test]
fn peer_store_writes_are_private_atomic_and_preserve_old_bytes_on_failure() {
    use std::os::unix::fs::PermissionsExt;

    let path = peer_test_path("atomic-write");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([(String::from("trusted"), String::from("key-v1"))]);
    save_peers_at(&path, &original).unwrap();
    let old_bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let injected = atomic_write_bytes_with(&path, b"{\"trusted\":\"partial\"}", |file, bytes| {
        std::io::Write::write_all(file, &bytes[..5])?;
        Err(std::io::Error::other(
            "erreur d’écriture injectée après préfixe",
        ))
    });
    assert!(injected.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old_bytes);
    assert_eq!(
        serde_json::from_slice::<HashMap<String, String>>(&old_bytes).unwrap(),
        original
    );

    let replacement = HashMap::from([(String::from("trusted"), String::from("key-v2"))]);
    let parent = path.parent().unwrap();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o500)).unwrap();
    let denied = save_peers_at(&path, &replacement);
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(denied.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old_bytes);

    save_peers_at(&path, &replacement).unwrap();
    let final_bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        serde_json::from_slice::<HashMap<String, String>>(&final_bytes).unwrap(),
        replacement
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(std::fs::read_dir(parent).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")
    }));
    remove_peer_test_path(&path);
}

#[test]
fn persisted_pin_is_reloaded_and_replacement_signer_is_rejected() {
    let path = peer_test_path("pin-replacement");
    initialize_peers_at(&path).unwrap();
    let alice = LocalIdentity::generate();
    let eve = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let alice_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let mut peers = load_peers_at(&path).unwrap();
    peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());
    save_peers_at(&path, &peers).unwrap();
    let mut reloaded = load_peers_at(&path).unwrap();
    assert_eq!(reloaded.get(&alice_node), Some(&alice.sign_pk_hex));

    let output =
        crate::msg::send("tentative d’usurpation", &bob.kem_public_key().unwrap()).unwrap();
    let session = "persisted-pin-replacement";
    let ciphertext = output.ciphertext.as_deref().unwrap();
    let signature = eve.sign_signer().unwrap().sign(&canonical_bytes(
        session,
        &alice_node,
        &bob_node,
        test_now(),
        output.kem_ct.as_bytes(),
        ciphertext,
    ));
    let forged = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node.clone(),
        to: bob_node,
        session: session.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: Some(signature.as_bytes().to_vec()),
        signer: Some(eve.sign_pk_hex),
        name_ct: None,
        ts: Some(test_now()),
    };
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    let unchanged = reloaded.clone();
    assert!(process_line(
        &serde_json::to_string(&forged).unwrap(),
        &bob,
        &mut reloaded,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap()
    .is_none());
    assert_eq!(reloaded, unchanged);
    assert!(sessions.is_empty());
    remove_peer_test_path(&path);
}

#[cfg(unix)]
#[test]
fn new_pin_is_not_delivered_or_kept_in_memory_when_persistence_fails() {
    use std::os::unix::fs::PermissionsExt;

    let path = peer_test_path("pin-write-failure");
    initialize_peers_at(&path).unwrap();
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send("ne pas livrer", &bob.kem_public_key().unwrap()).unwrap();
    let alice_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let session = "pin-must-persist";
    let mut peers = load_peers_at(&path).unwrap();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    assert!(process_line_with_persistence(
        &signed_kem_json(session, &alice, &bob_node, test_now(), &output, None),
        &bob,
        &mut peers,
        test_now(),
        &mut completed,
        &mut sessions,
        &path,
    )
    .unwrap()
    .is_none());
    for fragment in output.fragments.iter().take(3) {
        assert!(process_line_with_persistence(
            &frag_json(
                session,
                &alice_node,
                &bob_node,
                fragment.index,
                &fragment.share
            ),
            &bob,
            &mut peers,
            test_now(),
            &mut completed,
            &mut sessions,
            &path,
        )
        .unwrap()
        .is_none());
    }

    let parent = path.parent().unwrap();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o500)).unwrap();
    let last = &output.fragments[3];
    let result = process_line_with_persistence(
        &frag_json(session, &alice_node, &bob_node, last.index, &last.share),
        &bob,
        &mut peers,
        test_now(),
        &mut completed,
        &mut sessions,
        &path,
    );
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert!(
        result.is_err(),
        "le résultat vérifié ne doit pas être livré si le pin ne peut être écrit"
    );
    assert!(
        peers.is_empty(),
        "le nouveau pin est retiré de l’état en mémoire"
    );
    assert!(
        load_peers_at(&path).unwrap().is_empty(),
        "le store précédent reste intact"
    );
    remove_peer_test_path(&path);
}

fn res_request(session: &str) -> NetEnvelope {
    NetEnvelope {
        kind: "fragment".into(),
        from: "borrower-node".into(),
        to: "ghost-node".into(),
        session: session.into(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: serde_json::json!({"action": "compute", "task": "echo test"})
            .to_string()
            .into_bytes(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    }
}

fn signed_and_verified_grant_body(
    req: &NetEnvelope,
    ghost: &LocalIdentity,
    body: serde_json::Value,
) -> serde_json::Value {
    let grant = grant_for(req, ghost, body);
    let ghost_node = grant.from.clone();
    let signer = grant.signer.clone().expect("signed grant has a signer");
    let mut known_peers = HashMap::new();
    assert!(verify_grant(&grant, &mut known_peers, &ghost_node));
    assert_eq!(known_peers.get(&ghost_node), Some(&signer));
    serde_json::from_slice(&grant.payload).expect("grant payload is JSON")
}

fn assert_invalid_grant_does_not_learn_tofu(grant: &NetEnvelope, ghost_node: &str) {
    let mut known_peers = HashMap::new();
    assert!(!verify_grant(grant, &mut known_peers, ghost_node));
    assert!(
        known_peers.is_empty(),
        "un grant invalide ne doit pas apprendre une clé TOFU"
    );
}

#[test]
fn canonical_bytes_are_deterministic() {
    let a = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 5]);
    let b = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 5]);
    assert_eq!(a, b);
    let c = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 6]);
    assert_ne!(a, c, "different ciphertext must change the bytes");
    let d = canonical_bytes("s1", "alice", "bob", 124, &[1, 2, 3], &[4, 5]);
    assert_ne!(a, d, "different timestamp must change the bytes");
}

#[test]
fn node_id_is_stable_and_derived_from_the_kem_public_key() {
    let identity = LocalIdentity::generate();
    let expected = &identity.kem_pk_hex[..16];
    assert_eq!(node_id(&identity), expected);
    assert_eq!(node_id(&identity), node_id(&identity));
}

#[test]
fn oversized_wire_envelopes_are_rejected_before_connecting() {
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "alice".into(),
        to: "bob".into(),
        session: "large-block".into(),
        seq: 1,
        typ: "frag".into(),
        idx: 1,
        threshold: 4,
        total: 7,
        payload: vec![255; MAX_RELAY_LINE_BYTES],
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };

    let err = serialize_wire_line(&env).unwrap_err();
    assert!(err.to_string().contains("limite"));
}

#[tokio::test]
async fn inbound_wire_line_is_rejected_with_a_bounded_buffer() {
    let bytes = vec![b'x'; MAX_RELAY_LINE_BYTES + 4096];
    let mut reader = BufReader::new(std::io::Cursor::new(bytes));
    let mut line = Vec::new();

    let error = read_line_capped(&mut reader, &mut line, MAX_RELAY_LINE_BYTES)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(line.len() <= MAX_RELAY_LINE_BYTES + 1);
}

#[tokio::test]
async fn inbound_wire_line_accepts_the_exact_relay_limit() {
    let mut bytes = vec![b'x'; MAX_RELAY_LINE_BYTES];
    bytes.push(b'\n');
    let mut reader = BufReader::new(std::io::Cursor::new(bytes));
    let mut line = Vec::new();

    let read = read_line_capped(&mut reader, &mut line, MAX_RELAY_LINE_BYTES)
        .await
        .unwrap();
    assert_eq!(read, MAX_RELAY_LINE_BYTES + 1);
    assert_eq!(line.len(), MAX_RELAY_LINE_BYTES + 1);
    assert_eq!(line.last(), Some(&b'\n'));
}

#[test]
fn drive_chunks_fit_the_relay_line_limit() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let recipient_pk = bob.kem_public_key().unwrap();
    let data = vec![255; 12 * 1024];
    let outputs = crate::drive::DriveFile::prepare_send(&data, "chunk.bin", &recipient_pk).unwrap();
    assert_eq!(outputs.len(), 1);
    let bob_node = node_id(&bob);

    let kem_line = signed_kem_json(
        "drive-size-check",
        &alice,
        &bob_node,
        test_now(),
        &outputs[0],
        Some("chunk.bin"),
    );
    let kem_env: NetEnvelope = serde_json::from_str(&kem_line).unwrap();
    serialize_wire_line(&kem_env).unwrap();

    for fragment in &outputs[0].fragments {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: node_id(&alice),
            to: bob_node.clone(),
            session: "drive-size-check".into(),
            seq: u64::from(fragment.index),
            typ: "frag".into(),
            idx: fragment.index,
            threshold: 4,
            total: 7,
            payload: fragment.share.clone(),
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };
        serialize_wire_line(&env).unwrap();
    }
}

#[test]
fn full_pipeline_signed_message_round_trip() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message réseau test", &pk).unwrap();
    let session = "test-session-1";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    let r = process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(r.is_none(), "kem alone must not complete a session");

    for (i, frag) in output.fragments.iter().enumerate() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        if i == 3 {
            // The 4th fragment completes the session.
            let (sid, recv) = r.expect("4/7 must complete");
            assert_eq!(sid, session);
            match recv {
                Received::Message(text) => assert_eq!(text, "message réseau test"),
                _ => panic!("expected a message"),
            }
        } else {
            assert!(r.is_none());
        }
    }
}

#[test]
fn forged_signature_is_rejected() {
    // Eve forges a message "from Alice" — she claims Alice's node_id
    // but signs with HER own key. Bob knows Alice's signing key
    // out-of-band (the trust anchor) and must reject.
    let alice = LocalIdentity::generate();
    let eve = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("faux message d'alice", &pk).unwrap();
    let session = "forged-session";
    let bob_node = node_id(&bob);
    let alice_node = node_id(&alice);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    known_peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    // KEM envelope: from = Alice's node, sig+signer = Eve's key.
    let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
    let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
        session,
        &alice_node,
        &bob_node,
        test_now(),
        output.kem_ct.as_bytes(),
        &ciphertext,
    ));
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node, // claims to be Alice
        to: bob_node.clone(),
        session: session.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: Some(sig.as_bytes().to_vec()),
        signer: Some(eve.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(test_now()),
    };
    process_line(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    for frag in output.fragments.iter() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "a forged signature must never complete");
    }
    assert!(
        sessions.is_empty(),
        "une identité usurpée ne doit pas garder son slot"
    );
}

#[test]
fn invalid_signature_releases_session_after_all_fragment_indices_arrive() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send("signature corrompue", &bob.kem_public_key().unwrap()).unwrap();
    let session = "invalid-signature-release";
    let sender_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    known_peers.insert(sender_node.clone(), alice.sign_pk_hex.clone());
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    let mut kem_env: NetEnvelope = serde_json::from_str(&signed_kem_json(
        session,
        &alice,
        &bob_node,
        test_now(),
        &output,
        None,
    ))
    .unwrap();
    kem_env.sig.as_mut().unwrap()[0] ^= 1;
    process_line(
        &serde_json::to_string(&kem_env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    for (index, fragment) in output.fragments.iter().enumerate() {
        let result = process_line(
            &frag_json(
                session,
                &sender_node,
                &bob_node,
                fragment.index,
                &fragment.share,
            ),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(result.is_none());
        if index == 3 {
            assert!(sessions.contains_key(&format!("{sender_node}|{session}")));
        }
    }
    assert!(
        sessions.is_empty(),
        "un échec définitif ne doit pas conserver le slot jusqu'au TTL"
    );
}

#[test]
fn known_peer_with_matching_key_is_accepted() {
    // The same scenario, but Alice signs with HER key: the trust
    // anchor matches, the signature verifies, the message completes.
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("vraiment alice", &pk).unwrap();
    let session = "known-peer-session";
    let bob_node = node_id(&bob);
    let alice_node = node_id(&alice);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    known_peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    for (i, frag) in output.fragments.iter().enumerate() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        if i == 3 {
            let (_, recv) = r.expect("trusted peer must complete");
            match recv {
                Received::Message(text) => assert_eq!(text, "vraiment alice"),
                _ => panic!("expected a message"),
            }
            return;
        }
    }
    panic!("trusted peer session never completed");
}

#[test]
fn fragment_without_kem_does_not_reserve_session_state() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send("fragment", &bob.kem_public_key().unwrap()).unwrap();
    let fragment = &output.fragments[0];
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    let result = process_line(
        &frag_json(
            "orphan-fragment",
            &node_id(&alice),
            &node_id(&bob),
            fragment.index,
            &fragment.share,
        ),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    assert!(result.is_none());
    assert!(sessions.is_empty());
}

#[test]
fn tampered_fragment_is_detected_and_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message intègre", &pk).unwrap();
    let session = "tamper-session";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    // Corrupt ONE fragment (flip a byte). Send exactly 4 fragments,
    // one of them corrupt: the only 4-subset reconstructs a wrong
    // secret, so the signature can never verify.
    let mut frags: Vec<_> = output.fragments.clone();
    frags[0].share[0] ^= 0xFF;
    for frag in frags.iter().take(4) {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "tampered fragments must never complete");
    }
}

#[test]
fn wrong_recipient_is_ignored() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let carol = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message pour bob", &pk).unwrap();
    let session = "wrong-recipient";
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    // Envelope addressed to carol, delivered to bob.
    let kem_line = signed_kem_json(session, &alice, &node_id(&carol), test_now(), &output, None);
    let r = process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(r.is_none(), "envelopes not addressed to me must be ignored");
    assert!(sessions.is_empty(), "session must not be buffered");
}

#[test]
fn duplicate_fragment_index_is_dropped() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message dup idx", &pk).unwrap();
    let session = "dup-idx";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    // Send idx=1 twice + two other distinct fragments → 3 distinct
    // fragments buffered (the duplicate is dropped), session must NOT
    // complete yet.
    let f1 = &output.fragments[0];
    let f2 = &output.fragments[1];
    let f3 = &output.fragments[2];
    let f4 = &output.fragments[3];
    process_line(
        &frag_json(session, &node_id(&alice), &bob_node, f1.index, &f1.share),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    let r = process_line(
        &frag_json(session, &node_id(&alice), &bob_node, f1.index, &f1.share),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(r.is_none(), "duplicate idx must be dropped");
    process_line(
        &frag_json(session, &node_id(&alice), &bob_node, f2.index, &f2.share),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    process_line(
        &frag_json(session, &node_id(&alice), &bob_node, f3.index, &f3.share),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    // 3 distinct fragments → nothing completed yet, duplicate not buffered.
    let skey = format!("{}|{}", node_id(&alice), session);
    let buf = sessions.get(&skey).expect("session buffered");
    assert_eq!(buf.fragments.len(), 3, "the duplicate must not be buffered");
    // The 4th distinct fragment completes the session — normally.
    let line = frag_json(session, &node_id(&alice), &bob_node, f4.index, &f4.share);
    let (_, recv) = process_line(
        &line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap()
    .expect("4 distinct fragments must complete");
    match recv {
        Received::Message(text) => assert_eq!(text, "message dup idx"),
        _ => panic!("expected a message"),
    }
}

#[test]
fn unsigned_envelope_is_rejected() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("pas signé", &pk).unwrap();
    let session = "unsigned";

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    // Build a KEM envelope WITHOUT sig/signer (legacy or attacker).
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "alice".into(),
        to: node_id(&bob),
        session: session.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };
    process_line(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter() {
        let line = frag_json(session, "alice", &bob_node, frag.index, &frag.share);
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "unsigned sessions must fail closed");
    }
}

#[test]
fn full_pipeline_signed_file_with_encrypted_name() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let bytes = b"contenu secret du fichier".to_vec();
    let expected_hash = blake3::hash(&bytes);
    let output = crate::msg::send_bytes(&bytes, &pk).unwrap();
    let session = "test-session-file";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let kem_line = signed_kem_json(
        session,
        &alice,
        &bob_node,
        test_now(),
        &output,
        Some("plan.txt"),
    );
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    // The relay must never see the plaintext name.
    let env: NetEnvelope = serde_json::from_str(&kem_line).unwrap();
    let ct = env.name_ct.expect("name_ct present");
    assert!(
        !ct.windows(b"plan.txt".len()).any(|w| w == b"plan.txt"),
        "file name must be opaque on the wire"
    );

    for (i, frag) in output.fragments.iter().enumerate() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        if i == 3 {
            let (_, recv) = r.expect("file session must complete");
            match recv {
                Received::File { name, bytes: got } => {
                    assert_eq!(name, "plan.txt");
                    assert_eq!(got, bytes);
                    assert_eq!(blake3::hash(&got), expected_hash);
                }
                _ => panic!("expected a file"),
            }
            return;
        }
    }
    panic!("file session never completed");
}

#[test]
fn invalid_authenticated_filename_is_rejected_without_failing_receiver() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send_bytes(b"payload", &bob.kem_public_key().unwrap()).unwrap();
    let session = "invalid-filename";
    let sender_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    process_line(
        &signed_kem_json(session, &alice, &bob_node, test_now(), &output, Some("..")),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for fragment in output.fragments.iter().take(4) {
        let result = process_line(
            &frag_json(
                session,
                &sender_node,
                &bob_node,
                fragment.index,
                &fragment.share,
            ),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(
            result.is_none(),
            "invalid file names must be isolated to the envelope"
        );
    }
    assert!(completed.is_empty());
    assert!(
        sessions.is_empty(),
        "une session de fichier rejetée définitivement doit libérer son slot"
    );
}

#[test]
fn malformed_authenticated_drive_chunk_releases_session_slot() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output =
        crate::msg::send_bytes(b"PGDRIVE1-invalid", &bob.kem_public_key().unwrap()).unwrap();
    let session = "malformed-drive-chunk";
    let sender_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    process_line(
        &signed_kem_json(
            session,
            &alice,
            &bob_node,
            test_now(),
            &output,
            Some("plan.txt"),
        ),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for fragment in output.fragments.iter().take(4) {
        process_line(
            &frag_json(
                session,
                &sender_node,
                &bob_node,
                fragment.index,
                &fragment.share,
            ),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
    }
    assert!(completed.is_empty());
    assert!(
        sessions.is_empty(),
        "un en-tête Drive invalide après authentification ne doit pas réserver un slot"
    );

    let late_fragment = &output.fragments[4];
    let result = process_line(
        &frag_json(
            session,
            &sender_node,
            &bob_node,
            late_fragment.index,
            &late_fragment.share,
        ),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(result.is_none());
    assert!(
        sessions.is_empty(),
        "un fragment tardif sans KEM ne doit pas recréer le slot libéré"
    );
}

#[test]
fn signed_network_drive_file_reassembles_chunks_out_of_order() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let recipient_pk = bob.kem_public_key().unwrap();
    let sender_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let data: Vec<u8> = (0..(crate::drive::CHUNK_SIZE * 2 + 317))
        .map(|i| (i % 251) as u8)
        .collect();
    let outputs =
        crate::drive::DriveFile::prepare_send(&data, "archive.bin", &recipient_pk).unwrap();
    assert_eq!(outputs.len(), 3);

    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    let mut received_chunks = Vec::new();
    for (index, output) in outputs.iter().enumerate() {
        let session = format!("drive-network-{index}");
        process_line(
            &signed_kem_json(
                &session,
                &alice,
                &bob_node,
                test_now(),
                output,
                Some("archive.bin"),
            ),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        for fragment in output.fragments.iter().take(4) {
            let line = frag_json(
                &session,
                &sender_node,
                &bob_node,
                fragment.index,
                &fragment.share,
            );
            if let Some((_, received)) = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap()
            {
                match received {
                    Received::DriveChunk { sender, chunk } => {
                        assert_eq!(sender, sender_node);
                        received_chunks.push(chunk);
                    }
                    _ => panic!("expected an authenticated Drive chunk"),
                }
            }
        }
    }
    assert_eq!(received_chunks.len(), outputs.len());

    let mut pending = PendingDriveFiles::default();
    let mut assembled = None;
    for chunk in received_chunks.into_iter().rev() {
        if let Some(file) = pending
            .accept(sender_node.clone(), chunk, test_now())
            .unwrap()
        {
            assembled = Some(file);
        }
    }
    let (name, rebuilt) = assembled.expect("all network chunks should complete the file");
    assert_eq!(name, "archive.bin");
    assert_eq!(rebuilt, data);
    assert_eq!(blake3::hash(&rebuilt), blake3::hash(&data));
    assert_eq!(pending.buffered_bytes, 0);
}

#[test]
fn pending_network_drive_rejects_duplicate_chunks_and_discards_partial_file() {
    let mut pending = PendingDriveFiles::default();
    let chunk = || crate::drive::DecodedChunk {
        transfer_id: [9; 16],
        seq: 0,
        total: 2,
        filename: "duplicate.bin".into(),
        data: vec![5; crate::drive::CHUNK_SIZE],
    };
    assert!(pending
        .accept("alice".into(), chunk(), test_now())
        .unwrap()
        .is_none());
    assert_eq!(pending.buffered_bytes, crate::drive::CHUNK_SIZE);

    let error = pending
        .accept("alice".into(), chunk(), test_now())
        .unwrap_err();
    assert!(error.to_string().contains("dupliqué"));
    assert_eq!(pending.buffered_bytes, 0);
    assert!(pending.files.is_empty());
}

#[test]
fn pending_network_drive_rejects_inconsistent_headers() {
    let mut pending = PendingDriveFiles::default();
    let transfer_id = [11; 16];
    let first = crate::drive::DecodedChunk {
        transfer_id,
        seq: 0,
        total: 2,
        filename: "same.bin".into(),
        data: vec![1; crate::drive::CHUNK_SIZE],
    };
    assert!(pending
        .accept("alice".into(), first, test_now())
        .unwrap()
        .is_none());

    let inconsistent = crate::drive::DecodedChunk {
        transfer_id,
        seq: 1,
        total: 3,
        filename: "same.bin".into(),
        data: vec![2; 17],
    };
    let error = pending
        .accept("alice".into(), inconsistent, test_now())
        .unwrap_err();
    assert!(error.to_string().contains("incohérents"));
    assert_eq!(pending.buffered_bytes, 0);
    assert!(pending.files.is_empty());
}

#[test]
fn stale_pending_drive_transfers_release_their_memory_budget() {
    let mut pending = PendingDriveFiles::default();
    let chunk = crate::drive::DecodedChunk {
        transfer_id: [12; 16],
        seq: 0,
        total: 2,
        filename: "abandoned.bin".into(),
        data: vec![3; crate::drive::CHUNK_SIZE],
    };
    assert!(pending
        .accept("alice".into(), chunk, test_now())
        .unwrap()
        .is_none());
    assert_eq!(pending.buffered_bytes, crate::drive::CHUNK_SIZE);

    pending.expire_stale(test_now() + SESSION_TTL_SECS);
    assert!(pending.files.is_empty());
    assert_eq!(pending.buffered_bytes, 0);
}

#[test]
fn three_fragments_never_complete() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("trop peu de fragments", &pk).unwrap();
    let session = "test-incomplete";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    process_line(
        &signed_kem_json(session, &alice, &bob_node, test_now(), &output, None),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter().take(3) {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        assert!(process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions
        )
        .unwrap()
        .is_none());
    }
}

#[test]
fn grant_reply_routes_back_to_requester() {
    let ghost = LocalIdentity::generate();
    let req = NetEnvelope {
        kind: "fragment".into(),
        from: "borrower-node".into(),
        to: "ghost-node".into(),
        session: "res-session-1".into(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: serde_json::json!({"action": "compute", "task": "bench"})
            .to_string()
            .into_bytes(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };

    let grant = grant_for(&req, &ghost, grant_payload(&ghost));
    assert_eq!(grant.typ, "grant");
    assert_eq!(grant.to, "borrower-node"); // routed back to the requester
    assert_eq!(grant.session, "res-session-1"); // same session context
    assert_eq!(grant.from, node_id(&ghost)); // granted by the ghost
    let body: serde_json::Value =
        serde_json::from_slice(&grant.payload).expect("grant payload is JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["node"], node_id(&ghost));
}

#[tokio::test]
async fn borrow_request_rejects_wrong_recipient_without_tofu() {
    let borrower = LocalIdentity::generate();
    let borrower_node = node_id(&borrower);
    let ghost = LocalIdentity::generate();
    let ghost_node = node_id(&ghost);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap().to_string();
    let relay_ghost_node = ghost_node.clone();

    let relay = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(socket);
        let mut hello = String::new();
        reader.read_line(&mut hello).await.unwrap();
        assert_eq!(hello.trim(), format!("HELLO {borrower_node}"));
        reader.get_mut().write_all(b"HELLO_OK\n").await.unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let req: NetEnvelope = serde_json::from_str(&line).unwrap();
        assert_eq!(req.typ, "req");
        assert_eq!(req.to, relay_ghost_node);

        let mut grant = grant_for(&req, &ghost, grant_payload(&ghost));
        grant.to = "not-the-requester".into();
        let ts = grant.ts.expect("signed grant has a timestamp");
        let sig = ghost.sign_signer().unwrap().sign(&canonical_bytes(
            &grant.session,
            &grant.from,
            &grant.to,
            ts,
            &[],
            &grant.payload,
        ));
        grant.sig = Some(sig.as_bytes().to_vec());

        // Prove the mismatched recipient is the only invalid property: its
        // signature is valid and would otherwise trigger first-contact TOFU.
        let mut independently_verified_peers = HashMap::new();
        assert!(verify_grant(
            &grant,
            &mut independently_verified_peers,
            &relay_ghost_node
        ));
        assert_eq!(
            independently_verified_peers.get(&relay_ghost_node),
            grant.signer.as_ref()
        );

        let response = format!("{}\n", serde_json::to_string(&grant).unwrap());
        reader
            .get_mut()
            .write_all(response.as_bytes())
            .await
            .unwrap();
    });

    let mut known_peers = HashMap::new();
    let response = borrow_request(
        &relay_addr,
        &ghost_node,
        serde_json::json!({"action": "compute", "task": "route guard test"}),
        &borrower,
        &mut known_peers,
        std::time::Duration::from_secs(1),
    )
    .await
    .unwrap();

    assert_eq!(
        response, None,
        "le grant destiné à un autre nœud doit être rejeté"
    );
    assert!(
        known_peers.is_empty(),
        "un grant mal routé ne doit pas apprendre la clé du ghost par TOFU"
    );
    relay.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn valid_outbound_grant_is_not_accepted_when_pin_persistence_fails() {
    use std::os::unix::fs::PermissionsExt;

    let path = peer_test_path("grant-persistence-failure");
    initialize_peers_at(&path).unwrap();
    let parent = path.parent().unwrap().to_path_buf();
    let borrower = LocalIdentity::generate();
    let borrower_node = node_id(&borrower);
    let ghost = LocalIdentity::generate();
    let ghost_node = node_id(&ghost);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap().to_string();
    let relay_parent = parent.clone();

    let relay = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(socket);
        let mut hello = String::new();
        reader.read_line(&mut hello).await.unwrap();
        assert_eq!(hello.trim(), format!("HELLO {borrower_node}"));
        reader.get_mut().write_all(b"HELLO_OK\n").await.unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let request: NetEnvelope = serde_json::from_str(&line).unwrap();
        let grant = grant_for(&request, &ghost, grant_payload(&ghost));

        std::fs::set_permissions(&relay_parent, std::fs::Permissions::from_mode(0o500)).unwrap();
        reader
            .get_mut()
            .write_all(format!("{}\n", serde_json::to_string(&grant).unwrap()).as_bytes())
            .await
            .unwrap();
    });

    let mut known_peers = load_peers_at(&path).unwrap();
    let result = borrow_request_with_store_path(
        &relay_addr,
        &ghost_node,
        serde_json::json!({"action": "compute", "task": "grant persistence test"}),
        &borrower,
        &mut known_peers,
        std::time::Duration::from_secs(2),
        &path,
    )
    .await;
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert!(
        result.is_err(),
        "le grant ne doit pas être livré sans pin durable"
    );
    assert!(known_peers.is_empty(), "le pin volatil doit être annulé");
    assert!(load_peers_at(&path).unwrap().is_empty());
    relay.await.unwrap();
    remove_peer_test_path(&path);
}

#[test]
fn grant_with_execution_output_added_after_signing_fails_verification() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-output-regression");
    let ghost_node = node_id(&ghost);

    let mut grant = grant_for(&req, &ghost, grant_payload(&ghost));
    let mut body: serde_json::Value = serde_json::from_slice(&grant.payload).unwrap();
    body["output"] = serde_json::Value::String("test".into());
    grant.payload = body.to_string().into_bytes();

    assert_invalid_grant_does_not_learn_tofu(&grant, &ghost_node);
}

#[test]
fn grant_signature_verifies_success_without_output() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-success-empty");

    let verified = signed_and_verified_grant_body(&req, &ghost, grant_payload(&ghost));
    assert_eq!(verified["ok"], true);
    assert!(verified.get("output").is_none());
}

#[test]
fn grant_signature_verifies_success_with_output() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-success-output");
    let mut body = grant_payload(&ghost);
    body["output"] = serde_json::Value::String("succès".into());

    let verified = signed_and_verified_grant_body(&req, &ghost, body);
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["output"], "succès");
}

#[test]
fn grant_signature_verifies_authentication_refusal() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-refusal");
    let mut body = grant_payload(&ghost);
    body["ok"] = serde_json::Value::Bool(false);
    body["refus"] = serde_json::Value::String("requête non authentifiée".into());

    let verified = signed_and_verified_grant_body(&req, &ghost, body);
    assert_eq!(verified["ok"], false);
    assert_eq!(verified["refus"], "requête non authentifiée");
}

#[test]
fn grant_signature_verifies_reputation_refusal() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-reputation-refusal");
    let mut body = grant_payload(&ghost);
    body["ok"] = serde_json::Value::Bool(false);
    body["refus"] = serde_json::Value::String("réputation insuffisante".into());

    let verified = signed_and_verified_grant_body(&req, &ghost, body);
    assert_eq!(verified["ok"], false);
    assert_eq!(verified["refus"], "réputation insuffisante");
}

#[test]
fn grant_signature_verifies_execution_result() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-execution-result");
    let mut body = grant_payload(&ghost);
    body["output"] = serde_json::Value::String("42\n".into());

    let verified = signed_and_verified_grant_body(&req, &ghost, body);
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["output"], "42\n");
}

#[test]
fn grant_signature_rejects_wrong_session_without_tofu() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-original-session");
    let ghost_node = node_id(&ghost);
    let mut grant = grant_for(&req, &ghost, grant_payload(&ghost));
    grant.session = "res-other-session".into();

    assert_invalid_grant_does_not_learn_tofu(&grant, &ghost_node);
}

#[test]
fn grant_signature_rejects_wrong_sender_without_tofu() {
    let ghost = LocalIdentity::generate();
    let req = res_request("res-wrong-sender");
    let ghost_node = node_id(&ghost);
    let mut grant = grant_for(&req, &ghost, grant_payload(&ghost));
    grant.from = "impostor-node".into();

    assert_invalid_grant_does_not_learn_tofu(&grant, &ghost_node);
}

#[test]
fn grant_signature_rejects_wrong_signer_key_without_tofu() {
    let ghost = LocalIdentity::generate();
    let impostor = LocalIdentity::generate();
    let req = res_request("res-wrong-key");
    let ghost_node = node_id(&ghost);
    let mut grant = grant_for(&req, &ghost, grant_payload(&ghost));

    grant.signer = Some(impostor.sign_pk_hex.clone());
    assert_invalid_grant_does_not_learn_tofu(&grant, &ghost_node);

    let grant = grant_for(&req, &ghost, grant_payload(&ghost));
    let mut anchored_peers = HashMap::from([(ghost_node.clone(), impostor.sign_pk_hex)]);
    let expected_peers = anchored_peers.clone();
    assert!(!verify_grant(&grant, &mut anchored_peers, &ghost_node));
    assert_eq!(
        anchored_peers, expected_peers,
        "un ancrage existant ne doit pas être remplacé"
    );
}

// ── TOFU : premier contact appris, ancre ENFORCÉE ensuite ───────────────
// La config production charge known_peers depuis peers.json (vide au
// premier contact) et l'apprend après un message vérifié. Ce test
// verrouille la propriété réelle :
//   1. premier contact (map vide) → accepté (TOFU) + binding appris ;
//   2. message suivant du MÊME from avec une AUTRE clé → rejeté.
#[test]
fn tofu_learns_and_enforces_the_anchor() {
    let alice = LocalIdentity::generate(); // la vraie Alice
    let eve = LocalIdentity::generate(); // l'attaquante
    let bob = LocalIdentity::generate(); // la victime
    let pk = bob.kem_public_key().unwrap();
    let bob_node = node_id(&bob);
    let alice_node = node_id(&alice);

    let mut known_peers: HashMap<String, String> = HashMap::new(); // vide = prod fraîche
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    // 1) Eve (premier contact, from=alice, SA clé) → TOFU accepte…
    let output = crate::msg::send("premier contact", &pk).unwrap();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let session1 = "tofu-1";
    let ciphertext = output.ciphertext.clone().expect("ciphertext");
    let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
        session1,
        &alice_node,
        &bob_node,
        test_now(),
        output.kem_ct.as_bytes(),
        &ciphertext,
    ));
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node.clone(),
        to: bob_node.clone(),
        session: session1.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: Some(sig.as_bytes().to_vec()),
        signer: Some(eve.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(test_now()),
    };
    process_line(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter() {
        let line = frag_json(
            session1,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let _ = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
    }
    // …et le binding from→clé d'Eve est APPRIS.
    assert_eq!(
        known_peers.get(&alice_node).map(|s| s.as_str()),
        Some(eve.sign_pk_hex.as_str()),
        "le premier contact vérifié doit être appris (TOFU)"
    );

    // 2) La vraie Alice, MÊME from, SA clé → REJETÉE (ancre enforce).
    let output2 = crate::msg::send("vraiment alice cette fois", &pk).unwrap();
    let mut sessions2: HashMap<String, SessionBuffer> = HashMap::new();
    let session2 = "tofu-2";
    let ciphertext2 = output2.ciphertext.clone().expect("ciphertext");
    let sig2 = alice.sign_signer().unwrap().sign(&canonical_bytes(
        session2,
        &alice_node,
        &bob_node,
        test_now(),
        output2.kem_ct.as_bytes(),
        &ciphertext2,
    ));
    let env2 = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node.clone(),
        to: bob_node.clone(),
        session: session2.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output2.kem_ct.as_bytes().to_vec(),
        sig: Some(sig2.as_bytes().to_vec()),
        signer: Some(alice.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(test_now()),
    };
    process_line(
        &serde_json::to_string(&env2).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions2,
    )
    .unwrap();
    for frag in output2.fragments.iter() {
        let line = frag_json(
            session2,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions2,
        )
        .unwrap();
        assert!(
            r.is_none(),
            "une clé différente pour un from déjà ancré doit être rejetée"
        );
    }
    // Le premier from ancré est conservé (pas écrasé par le second).
    assert_eq!(
        known_peers.get(&alice_node).map(|s| s.as_str()),
        Some(eve.sign_pk_hex.as_str())
    );
}

#[test]
fn replayed_session_is_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message rejoué", &pk).unwrap();
    let bob_node = node_id(&bob);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    // Session complète (ts valide).
    let session = "replay-1";
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let _ = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
    }

    // La même session, rejouée → rejetée (anti-replay cache).
    let mut sessions2: HashMap<String, SessionBuffer> = HashMap::new();
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions2,
    )
    .unwrap();
    for frag in output.fragments.iter() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions2,
        )
        .unwrap();
        assert!(r.is_none(), "une session déjà complétée doit être rejetée");
    }
}

#[test]
fn replay_after_completed_cache_eviction_is_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("rejeu après éviction", &pk).unwrap();
    let bob_node = node_id(&bob);
    let sender_node = node_id(&alice);
    let session = "replay-evicted";

    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter().take(4) {
        let result = process_line(
            &frag_json(session, &sender_node, &bob_node, frag.index, &frag.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        if frag.index == 4 {
            assert!(result.is_some(), "première livraison valide acceptée");
        }
    }
    assert!(completed.iter().any(|(sid, _)| sid == session));

    // Simule les livraisons suivantes avec la politique FIFO du cache : après
    // MAX_COMPLETED entrées fraîches, la première session disparaît.
    for i in 0..MAX_COMPLETED {
        completed.push_back((format!("later-{i}"), test_now()));
        if completed.len() > MAX_COMPLETED {
            completed.pop_front();
        }
    }
    assert_eq!(completed.len(), MAX_COMPLETED);
    assert!(!completed.iter().any(|(sid, _)| sid == session));

    // Les anciennes enveloppes gardent une signature et un timestamp encore
    // frais. Elles ne doivent pas redevenir acceptables à cause de l'éviction.
    let mut replay_sessions = HashMap::new();
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut replay_sessions,
    )
    .unwrap();
    for frag in output.fragments.iter().take(4) {
        let result = process_line(
            &frag_json(session, &sender_node, &bob_node, frag.index, &frag.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut replay_sessions,
        )
        .unwrap();
        assert!(result.is_none(), "un rejeu évincé ne doit pas être accepté");
    }
}

#[test]
fn completed_cache_releases_capacity_after_replay_window_expires() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send("cache expiré", &bob.kem_public_key().unwrap()).unwrap();
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut sessions = HashMap::new();
    let mut completed: VecDeque<_> = (0..MAX_COMPLETED)
        .map(|index| (format!("old-{index}"), test_now()))
        .collect();

    let boundary_now = test_now() + COMPLETED_RETENTION_SECS;
    process_line(
        &signed_kem_json(
            "at-retention-boundary",
            &alice,
            &bob_node,
            boundary_now,
            &output,
            None,
        ),
        &bob,
        &mut known_peers,
        boundary_now,
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert_eq!(completed.len(), MAX_COMPLETED);
    assert!(
        sessions.is_empty(),
        "les ids restent protégés à la borne inclusive"
    );

    let expired_now = boundary_now + 1;
    process_line(
        &signed_kem_json(
            "after-retention-window",
            &alice,
            &bob_node,
            expired_now,
            &output,
            None,
        ),
        &bob,
        &mut known_peers,
        expired_now,
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(completed.is_empty());
    assert!(sessions.contains_key(&format!("{}|after-retention-window", node_id(&alice))));
}

#[test]
fn stale_timestamp_is_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("vieux message", &pk).unwrap();
    let bob_node = node_id(&bob);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    // Signé il y a 10 heures (hors fenêtre ±300 s).
    let old_ts = test_now() - 10 * 3600;
    let session = "stale-1";
    let kem_line = signed_kem_json(session, &alice, &bob_node, old_ts, &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    for frag in output.fragments.iter() {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "un horodatage hors fenêtre doit être rejeté");
    }
}

#[test]
fn duplicate_kem_and_fragment_do_not_refresh_session_ttl() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send("fragment unique", &bob.kem_public_key().unwrap()).unwrap();
    let sender_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let session = "duplicate-does-not-refresh";
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);

    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    let key = format!("{sender_node}|{session}");
    assert_eq!(sessions.get(&key).unwrap().touched, test_now());

    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now() + 1,
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert_eq!(sessions.get(&key).unwrap().touched, test_now());

    let fragment = &output.fragments[0];
    let fragment_line = frag_json(
        session,
        &sender_node,
        &bob_node,
        fragment.index,
        &fragment.share,
    );
    process_line(
        &fragment_line,
        &bob,
        &mut known_peers,
        test_now() + 2,
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert_eq!(sessions.get(&key).unwrap().touched, test_now() + 2);

    process_line(
        &fragment_line,
        &bob,
        &mut known_peers,
        test_now() + SESSION_TTL_SECS - 1,
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert_eq!(
        sessions.get(&key).unwrap().touched,
        test_now() + 2,
        "un fragment dupliqué ne doit pas renouveler l’inactivité"
    );
}

#[test]
fn session_cap_rejects_new_sessions() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let output = crate::msg::send("capacity check", &bob.kem_public_key().unwrap()).unwrap();
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    // Fill the map to the cap with dummy (from|session) keys.
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    for i in 0..MAX_SESSIONS {
        sessions.insert(
            format!("node{}|s{}", i % 7, i),
            SessionBuffer {
                touched: test_now(),
                ..SessionBuffer::default()
            },
        );
    }
    assert_eq!(sessions.len(), MAX_SESSIONS);

    // A NEW session beyond the cap → fail closed, nothing buffered.
    let r = process_line(
        &signed_kem_json(
            "fresh-session",
            &alice,
            &bob_node,
            test_now(),
            &output,
            None,
        ),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(r.is_none());
    assert_eq!(
        sessions.len(),
        MAX_SESSIONS,
        "le plafond de sessions ne doit jamais être dépassé"
    );
}

#[test]
fn expired_sessions_at_capacity_are_purged_before_accepting_new_session() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let output = crate::msg::send("TTL recovery", &bob.kem_public_key().unwrap()).unwrap();
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    for i in 0..MAX_SESSIONS {
        sessions.insert(
            format!("old-node{}|old-session{}", i % 7, i),
            SessionBuffer {
                touched: test_now().saturating_sub(SESSION_TTL_SECS),
                ..SessionBuffer::default()
            },
        );
    }

    process_line(
        &signed_kem_json(
            "fresh-session",
            &alice,
            &bob_node,
            test_now(),
            &output,
            None,
        ),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    assert_eq!(sessions.len(), 1);
    assert!(sessions.contains_key(&format!("{}|fresh-session", node_id(&alice))));
}

#[test]
fn malformed_envelopes_do_not_reserve_session_slots() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    let mut env = NetEnvelope {
        kind: "fragment".into(),
        from: "peer".into(),
        to: bob_node,
        session: "bad-session".into(),
        seq: 0,
        typ: "unknown".into(),
        idx: 1,
        threshold: 4,
        total: 7,
        payload: vec![1, 2, 3],
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };

    for (typ, idx, session) in [
        ("unknown", 1, "unknown"),
        ("frag", 0, "bad-index"),
        ("frag", 1, "pre-kem"),
    ] {
        env.typ = typ.into();
        env.idx = idx;
        env.session = session.into();
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(sessions.is_empty());
    }

    env.typ = "kem".into();
    env.session = "malformed-kem".into();
    process_line(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn out_of_range_fragment_index_is_dropped() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("idx hors bornes", &pk).unwrap();
    let bob_node = node_id(&bob);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    let session = "bad-idx";
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    // idx=200 (u8 hors 1..=7) → drop, jamais bufferisé.
    let r = process_line(
        &frag_json(session, &node_id(&alice), &bob_node, 200, &[1, 2, 3]),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(r.is_none());
    // 4 vrais fragments suffisent toujours à compléter (le 200 n'a rien pollué).
    for (i, frag) in output.fragments.iter().enumerate().take(4) {
        let line = frag_json(
            session,
            &node_id(&alice),
            &bob_node,
            frag.index,
            &frag.share,
        );
        let r = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        if i == 3 {
            assert!(r.is_some(), "4 fragments valides doivent compléter");
        }
    }
}

#[test]
fn disappearance_during_session_blocks_delivery_and_rolls_back_new_pin() {
    let path = peer_test_path("disappeared-during-session");
    initialize_peers_at(&path).unwrap();

    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let output = crate::msg::send(
        "non livré si le store disparaît",
        &bob.kem_public_key().unwrap(),
    )
    .unwrap();
    let alice_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let session = "store-disappeared-in-flight";
    let mut peers = load_peers_at(&path).unwrap();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    assert!(process_line_with_persistence(
        &kem_line,
        &bob,
        &mut peers,
        test_now(),
        &mut completed,
        &mut sessions,
        &path,
    )
    .unwrap()
    .is_none());

    // The process loaded a valid empty store, but it disappeared before this
    // newly authenticated message could commit its first trust pin.
    std::fs::remove_file(&path).unwrap();
    for (index, fragment) in output.fragments.iter().take(4).enumerate() {
        let line = frag_json(
            session,
            &alice_node,
            &bob_node,
            fragment.index,
            &fragment.share,
        );
        let result = process_line_with_persistence(
            &line,
            &bob,
            &mut peers,
            test_now(),
            &mut completed,
            &mut sessions,
            &path,
        );
        if index == 3 {
            assert!(
                result.is_err(),
                "l’erreur de persistance doit bloquer la livraison"
            );
        } else {
            assert!(result.unwrap().is_none());
        }
    }
    assert!(peers.is_empty(), "le pin volatile doit être annulé");
    assert!(
        !path.exists(),
        "la persistance ne doit pas recréer le store disparu"
    );
    remove_peer_test_path(&path);
}

#[test]
fn learned_pin_is_persisted_reloaded_and_replacement_is_rejected() {
    let path = peer_test_path("learned-pin-durable");
    initialize_peers_at(&path).unwrap();

    let alice = LocalIdentity::generate();
    let impostor = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let alice_node = node_id(&alice);
    let bob_node = node_id(&bob);
    let output = crate::msg::send("pin persistant", &bob.kem_public_key().unwrap()).unwrap();
    let session = "pin-committed-before-delivery";
    let mut peers = load_peers_at(&path).unwrap();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();

    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    assert!(process_line_with_persistence(
        &kem_line,
        &bob,
        &mut peers,
        test_now(),
        &mut completed,
        &mut sessions,
        &path,
    )
    .unwrap()
    .is_none());
    let mut delivered = None;
    for fragment in output.fragments.iter().take(4) {
        let line = frag_json(
            session,
            &alice_node,
            &bob_node,
            fragment.index,
            &fragment.share,
        );
        let result = process_line_with_persistence(
            &line,
            &bob,
            &mut peers,
            test_now(),
            &mut completed,
            &mut sessions,
            &path,
        )
        .unwrap();
        if result.is_some() {
            delivered = result;
        }
    }
    assert!(delivered.is_some());
    assert_eq!(peers.get(&alice_node), Some(&alice.sign_pk_hex));
    let reloaded = load_peers_at(&path).unwrap();
    assert_eq!(reloaded.get(&alice_node), Some(&alice.sign_pk_hex));

    let replacement_output =
        crate::msg::send("substitution", &bob.kem_public_key().unwrap()).unwrap();
    let replacement_session = "replacement-after-reload";
    let ciphertext = replacement_output.ciphertext.as_deref().unwrap();
    let signature = impostor.sign_signer().unwrap().sign(&canonical_bytes(
        replacement_session,
        &alice_node,
        &bob_node,
        test_now(),
        replacement_output.kem_ct.as_bytes(),
        ciphertext,
    ));
    let replacement = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node.clone(),
        to: bob_node,
        session: replacement_session.into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: replacement_output.kem_ct.as_bytes().to_vec(),
        sig: Some(signature.as_bytes().to_vec()),
        signer: Some(impostor.sign_pk_hex),
        name_ct: None,
        ts: Some(test_now()),
    };
    assert!(process_line_with_persistence(
        &serde_json::to_string(&replacement).unwrap(),
        &bob,
        &mut peers,
        test_now(),
        &mut completed,
        &mut sessions,
        &path,
    )
    .unwrap()
    .is_none());
    assert_eq!(load_peers_at(&path).unwrap(), reloaded);
    remove_peer_test_path(&path);
}

#[test]
fn concurrent_pin_commits_serialize_and_never_lose_the_first_anchor() {
    let path = peer_test_path("concurrent-commits");
    initialize_peers_at(&path).unwrap();
    let previous = HashMap::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let path_a = path.clone();
    let previous_a = previous.clone();
    let barrier_a = barrier.clone();
    let mut peers_a = HashMap::from([("shared-node".to_string(), "key-a".to_string())]);
    let thread_a = std::thread::spawn(move || {
        barrier_a.wait();
        let result = persist_peer_changes_at(&mut peers_a, &previous_a, &path_a);
        (result, peers_a)
    });
    let path_b = path.clone();
    let previous_b = previous.clone();
    let barrier_b = barrier.clone();
    let mut peers_b = HashMap::from([("shared-node".to_string(), "key-b".to_string())]);
    let thread_b = std::thread::spawn(move || {
        barrier_b.wait();
        let result = persist_peer_changes_at(&mut peers_b, &previous_b, &path_b);
        (result, peers_b)
    });

    barrier.wait();
    let (result_a, peers_a) = thread_a.join().unwrap();
    let (result_b, peers_b) = thread_b.join().unwrap();
    assert_ne!(
        result_a.is_ok(),
        result_b.is_ok(),
        "un seul commit concurrent doit gagner"
    );
    let stored = load_peers_at(&path).unwrap();
    assert!(stored == peers_a || stored == peers_b);
    if result_a.is_err() {
        assert!(
            peers_a.is_empty(),
            "l’instantané refusé est annulé en mémoire"
        );
    }
    if result_b.is_err() {
        assert!(
            peers_b.is_empty(),
            "l’instantané refusé est annulé en mémoire"
        );
    }
    assert_eq!(stored.len(), 1, "aucune ancre commise ne doit être perdue");
    remove_peer_test_path(&path);
}

#[test]
fn directory_sync_failure_restores_the_previous_store_before_returning_error() {
    let path = peer_test_path("directory-sync-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let replacement = br#"{"trusted-peer":"key-after"}"#;
    let sync_calls = std::cell::Cell::new(0usize);

    let error =
        atomic_write_bytes_with_hooks(&path, replacement, std::io::Write::write_all, |_| {
            let call = sync_calls.get();
            sync_calls.set(call + 1);
            if call == 1 {
                Err(std::io::Error::other("échec fsync dossier injecté"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();

    assert!(error.to_string().contains("échec fsync dossier injecté"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    assert_eq!(load_peers_at(&path).unwrap(), original);
    assert!(std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .all(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            !name.contains(".backup-") && !name.contains(".tmp-") && !name.ends_with(".transaction")
        }));
    remove_peer_test_path(&path);
}

#[test]
fn failed_replace_keeps_backup_and_transaction_for_explicit_recovery() {
    let path = peer_test_path("replace-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |_, _| Err(std::io::Error::other("rename injecté")),
        |backup| std::fs::remove_file(backup),
        |_| Ok(()),
    )
    .unwrap_err();

    assert!(error.to_string().contains("rename injecté"));
    assert!(error.to_string().contains("état indéterminé"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().contains(".backup-"))
        .expect("la sauvegarde doit rester disponible");
    assert_eq!(std::fs::read(backup.path()).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn replace_that_mutates_then_errors_keeps_backup_and_blocks_load() {
    let path = peer_test_path("replace-mutates-then-errors");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let cleanup_calls = std::cell::Cell::new(0usize);
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| {
            std::fs::rename(from, to)?;
            Err(std::io::Error::other("erreur après rename injectée"))
        },
        |backup| {
            cleanup_calls.set(cleanup_calls.get() + 1);
            std::fs::remove_file(backup)
        },
        |_| Ok(()),
    )
    .unwrap_err();

    assert!(error.to_string().contains("erreur après rename injectée"));
    assert!(error.to_string().contains("état indéterminé"));
    assert!(error.to_string().contains("récupération explicite requise"));
    assert_eq!(
        cleanup_calls.get(),
        0,
        "le backup ne doit pas être supprimé"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        br#"{"trusted-peer":"key-after"}"#
    );
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().contains(".backup-"))
        .expect("le store antérieur doit être conservé en backup");
    assert_eq!(std::fs::read(backup.path()).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn failed_restore_after_parent_sync_error_blocks_load_for_explicit_recovery() {
    let path = peer_test_path("restore-rename-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let rename_calls = std::cell::Cell::new(0usize);
    let sync_calls = std::cell::Cell::new(0usize);
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| {
            let call = rename_calls.get();
            rename_calls.set(call + 1);
            if call == 0 {
                std::fs::rename(from, to)
            } else {
                Err(std::io::Error::other("restauration injectée"))
            }
        },
        |backup| std::fs::remove_file(backup),
        |_| {
            let call = sync_calls.get();
            sync_calls.set(call + 1);
            if call == 1 {
                Err(std::io::Error::other("fsync après remplacement injecté"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("restauration injectée"));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        br#"{"trusted-peer":"key-after"}"#
    );
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().contains(".backup-"))
        .expect("le backup d’origine doit rester disponible");
    assert_eq!(std::fs::read(backup.path()).unwrap(), previous_bytes);
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn rollback_replace_after_effect_keeps_backup_and_blocks_load() {
    let path = peer_test_path("rollback-replace-after-effect");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let rename_calls = std::cell::Cell::new(0usize);
    let sync_calls = std::cell::Cell::new(0usize);
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| {
            let call = rename_calls.get();
            rename_calls.set(call + 1);
            std::fs::rename(from, to)?;
            if call == 1 {
                Err(std::io::Error::other("rollback rename après effet injecté"))
            } else {
                Ok(())
            }
        },
        |backup| std::fs::remove_file(backup),
        |_| {
            let call = sync_calls.get();
            sync_calls.set(call + 1);
            if call == 1 {
                Err(std::io::Error::other("fsync après remplacement injecté"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("rollback rename après effet injecté"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().contains(".backup-"))
        .expect("le backup d’origine doit survivre au rename de rollback en erreur");
    assert_eq!(std::fs::read(backup.path()).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn failed_backup_cleanup_before_effect_keeps_backup_and_blocks_load() {
    let path = peer_test_path("backup-cleanup-restore-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let rename_calls = std::cell::Cell::new(0usize);
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| {
            let call = rename_calls.get();
            rename_calls.set(call + 1);
            if call == 0 {
                std::fs::rename(from, to)
            } else {
                Err(std::io::Error::other("restauration après cleanup injectée"))
            }
        },
        |_| Err(std::io::Error::other("suppression backup injectée")),
        |_| Ok(()),
    )
    .unwrap_err();

    assert!(error.to_string().contains("suppression backup injectée"));
    assert_eq!(
        rename_calls.get(),
        1,
        "aucun rollback ne doit consommer le backup"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        br#"{"trusted-peer":"key-after"}"#
    );
    let backup = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().contains(".backup-"))
        .expect("le backup doit rester disponible après l’échec de suppression");
    assert_eq!(std::fs::read(backup.path()).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn backup_cleanup_that_errors_after_removal_is_verified_as_completed() {
    let path = peer_test_path("backup-cleanup-after-effect");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let replacement = HashMap::from([("trusted-peer".to_string(), "key-after".to_string())]);

    atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| std::fs::rename(from, to),
        |backup| {
            std::fs::remove_file(backup)?;
            Err(std::io::Error::other(
                "suppression backup après effet injectée",
            ))
        },
        |_| Ok(()),
    )
    .expect("la suppression vérifiée comme effective ne doit pas annuler le commit");

    assert_eq!(load_peers_at(&path).unwrap(), replacement);
    assert!(!peer_transaction_path(&path).exists());
    assert!(std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".backup-")));
    remove_peer_test_path(&path);
}

#[test]
fn failed_rollback_directory_sync_leaves_transaction_marker_and_blocks_load() {
    let path = peer_test_path("rollback-sync-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let sync_calls = std::cell::Cell::new(0usize);
    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| std::fs::rename(from, to),
        |backup| std::fs::remove_file(backup),
        |_| {
            let call = sync_calls.get();
            sync_calls.set(call + 1);
            match call {
                1 => Err(std::io::Error::other("fsync après remplacement injecté")),
                2 => Err(std::io::Error::other("fsync rollback injecté")),
                _ => Ok(()),
            }
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("fsync rollback injecté"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn orphaned_backup_without_transaction_marker_blocks_load() {
    let path = peer_test_path("orphan-backup");
    initialize_peers_at(&path).unwrap();
    let backup = path.parent().unwrap().join(".peers.json.backup-orphaned");
    std::fs::write(&backup, b"{}").unwrap();

    let error = load_peers_at(&path).unwrap_err().to_string();
    assert!(error.contains("sauvegarde du store de confiance non résolue"));
    assert!(error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn trust_store_purge_waits_for_writer_and_removes_temporary_residues() {
    let path = peer_test_path("locked-purge");
    initialize_peers_at(&path).unwrap();
    let parent = path.parent().unwrap();
    let marker = peer_marker_path(&path);
    let leftovers = [
        parent.join(".peers.json.tmp-abandoned"),
        parent.join(".peers.json.backup-abandoned"),
        parent.join("peers.json.transaction"),
        parent.join(".peers.json.initialized.tmp-abandoned"),
        parent.join(".peers.json.initialized.backup-abandoned"),
        parent.join("peers.json.initialized.transaction"),
    ];
    for leftover in &leftovers {
        std::fs::write(leftover, b"stale trust data").unwrap();
    }

    let writer_lock = lock_peer_store(&path).unwrap();
    let purge_path = path.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let purge = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(remove_peers_at(&purge_path)).unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err(),
        "la purge doit attendre le verrou de l’écrivain"
    );
    drop(writer_lock);
    assert!(done_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap());
    purge.join().unwrap();

    assert!(!path.exists());
    assert!(!marker.exists());
    for leftover in leftovers {
        assert!(
            !leftover.exists(),
            "résidu de confiance : {}",
            leftover.display()
        );
    }
    remove_peer_test_path(&path);
}

#[test]
fn distinct_concurrent_new_pins_are_merged_without_stopping_either_commit() {
    let path = peer_test_path("concurrent-distinct-pins");
    initialize_peers_at(&path).unwrap();
    let previous = HashMap::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

    let path_a = path.clone();
    let previous_a = previous.clone();
    let barrier_a = barrier.clone();
    let mut peers_a = HashMap::from([("peer-a".to_string(), "key-a".to_string())]);
    let thread_a = std::thread::spawn(move || {
        barrier_a.wait();
        let result = persist_peer_changes_at(&mut peers_a, &previous_a, &path_a);
        (result, peers_a)
    });

    let path_b = path.clone();
    let previous_b = previous.clone();
    let barrier_b = barrier.clone();
    let mut peers_b = HashMap::from([("peer-b".to_string(), "key-b".to_string())]);
    let thread_b = std::thread::spawn(move || {
        barrier_b.wait();
        let result = persist_peer_changes_at(&mut peers_b, &previous_b, &path_b);
        (result, peers_b)
    });

    barrier.wait();
    let (result_a, _) = thread_a.join().unwrap();
    let (result_b, _) = thread_b.join().unwrap();
    assert!(result_a.is_ok(), "premier pin: {result_a:?}");
    assert!(
        result_b.is_ok(),
        "le pin distinct concurrent doit être fusionné: {result_b:?}"
    );
    let stored = load_peers_at(&path).unwrap();
    assert_eq!(stored.get("peer-a").map(String::as_str), Some("key-a"));
    assert_eq!(stored.get("peer-b").map(String::as_str), Some("key-b"));
    remove_peer_test_path(&path);
}

#[test]
fn peer_store_process_lock_child() {
    let Some(path) = std::env::var_os("POLYGONE_PEERS_LOCK_TEST_PATH") else {
        return;
    };
    let ready = std::path::PathBuf::from(
        std::env::var_os("POLYGONE_PEERS_LOCK_TEST_READY").expect("ready path"),
    );
    let release = std::path::PathBuf::from(
        std::env::var_os("POLYGONE_PEERS_LOCK_TEST_RELEASE").expect("release path"),
    );
    let _lock = lock_peer_store(std::path::Path::new(&path)).unwrap();
    std::fs::write(ready, b"locked").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !release.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "parent did not release lock"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn peer_store_lock_serializes_separate_processes() {
    let path = peer_test_path("cross-process-lock");
    initialize_peers_at(&path).unwrap();
    let parent = path.parent().unwrap();
    let ready = parent.join("child-ready");
    let release = parent.join("child-release");
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "net::tests::peer_store_process_lock_child",
            "--nocapture",
        ])
        .env("POLYGONE_PEERS_LOCK_TEST_PATH", &path)
        .env("POLYGONE_PEERS_LOCK_TEST_READY", &ready)
        .env("POLYGONE_PEERS_LOCK_TEST_RELEASE", &release)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut child = child;
    let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !ready.exists() && std::time::Instant::now() < ready_deadline {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("processus de verrouillage sorti prématurément : {status}");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !ready.exists() {
        std::fs::write(&release, b"release").unwrap();
        let _ = child.wait();
        panic!("le processus enfant n’a pas acquis le verrou partagé");
    }

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        let mut peers = HashMap::from([("cross-process-peer".to_string(), "key".to_string())]);
        let result = persist_peer_changes_at(&mut peers, &HashMap::new(), &writer_path);
        done_tx.send(result.is_ok()).unwrap();
    });
    let early_result = done_rx.recv_timeout(std::time::Duration::from_millis(150));
    let was_blocked = matches!(
        &early_result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    );
    std::fs::write(&release, b"release").unwrap();
    let writer_succeeded = match early_result {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("l’écrivain doit reprendre après la libération du verrou"),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            panic!("l’écrivain s’est arrêté avant de rendre son résultat")
        }
    };
    writer.join().unwrap();
    assert!(
        child.wait().unwrap().success(),
        "le processus enfant doit finir"
    );
    assert!(
        was_blocked,
        "l’écrivain n’a pas attendu le verrou détenu par l’autre processus"
    );
    assert!(writer_succeeded);
    assert_eq!(
        load_peers_at(&path)
            .unwrap()
            .get("cross-process-peer")
            .map(String::as_str),
        Some("key")
    );
    remove_peer_test_path(&path);
}

#[test]
fn purge_of_absent_store_directory_still_waits_for_stable_lock() {
    let id = PEER_TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "polygone-peers-absent-parent-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let parent = root.join("not-created-yet");
    let path = parent.join("peers.json");
    let held_lock = lock_peer_store(&path).unwrap();
    let purge_path = path.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let purge = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(remove_peers_at(&purge_path)).unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err(),
        "une purge sans dossier doit tout de même prendre le verrou stable"
    );
    drop(held_lock);
    assert!(!done_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap());
    purge.join().unwrap();
    assert!(
        !parent.exists(),
        "la purge ne doit pas créer le dossier du store"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn marker_clear_sync_failure_restores_the_previous_store() {
    let path = peer_test_path("marker-clear-sync-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let sync_calls = std::cell::Cell::new(0usize);

    let error = atomic_write_bytes_with_full_hooks(
        &path,
        br#"{"trusted-peer":"key-after"}"#,
        std::io::Write::write_all,
        |from, to| std::fs::rename(from, to),
        |backup| std::fs::remove_file(backup),
        |_| {
            let call = sync_calls.get();
            sync_calls.set(call + 1);
            if call == 2 {
                Err(std::io::Error::other(
                    "fsync effacement du marqueur injecté",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("fsync effacement du marqueur injecté"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    assert_eq!(load_peers_at(&path).unwrap(), original);
    remove_peer_test_path(&path);
}

#[test]
fn purge_rejects_directory_residue_without_claiming_success() {
    let path = peer_test_path("directory-residue");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key-before".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let previous_bytes = std::fs::read(&path).unwrap();
    let residue = path
        .parent()
        .unwrap()
        .join(".peers.json.backup-unexpected-dir");
    std::fs::create_dir_all(&residue).unwrap();
    std::fs::write(residue.join("old-store-copy"), &previous_bytes).unwrap();

    let error = remove_peers_at(&path).unwrap_err();
    assert!(error
        .to_string()
        .contains("résidu de persistance non régulier"));
    assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
    assert!(peer_transaction_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("récupération explicite requise"));
    remove_peer_test_path(&path);
}

#[test]
fn purge_marker_clear_sync_failure_reinstalls_fail_closed_guard() {
    let path = peer_test_path("purge-marker-clear-sync-failure");
    initialize_peers_at(&path).unwrap();
    let original = HashMap::from([("trusted-peer".to_string(), "key".to_string())]);
    save_peers_at(&path, &original).unwrap();
    let sync_calls = std::cell::Cell::new(0usize);

    let error = remove_peers_at_with_sync(&path, |parent| {
        let call = sync_calls.get();
        sync_calls.set(call + 1);
        if call == 2 {
            Err(std::io::Error::other("fsync retrait garde injecté"))
        } else {
            sync_parent_directory(parent)
        }
    })
    .unwrap_err();

    assert!(error.to_string().contains("fsync retrait garde injecté"));
    assert!(peer_transaction_path(&path).exists());
    assert!(!path.exists());
    assert!(!peer_marker_path(&path).exists());
    let load_error = load_peers_at(&path).unwrap_err().to_string();
    assert!(load_error.contains("transaction du store de confiance non résolue"));
    remove_peer_test_path(&path);
}
