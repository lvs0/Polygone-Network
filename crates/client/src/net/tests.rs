use super::*;
use crate::identity::LocalIdentity;

#[test]
fn relay_transport_disconnect_errors_are_retryable() {
    for kind in [
        std::io::ErrorKind::BrokenPipe,
        std::io::ErrorKind::ConnectionAborted,
        std::io::ErrorKind::ConnectionRefused,
        std::io::ErrorKind::ConnectionReset,
        std::io::ErrorKind::NotConnected,
        std::io::ErrorKind::TimedOut,
        std::io::ErrorKind::UnexpectedEof,
    ] {
        assert!(is_retryable_relay_io(kind), "{kind:?} must reconnect");
    }
}

#[test]
fn malformed_relay_frames_are_not_retryable_transport_errors() {
    for kind in [
        std::io::ErrorKind::InvalidData,
        std::io::ErrorKind::PermissionDenied,
        std::io::ErrorKind::InvalidInput,
    ] {
        assert!(
            !is_retryable_relay_io(kind),
            "{kind:?} must remain a visible error, not be hidden by reconnect"
        );
    }
}

#[test]
fn protocol_and_authentication_errors_are_not_retryable_transport_errors() {
    let error = anyhow::anyhow!("relay: réponse inattendue au handshake: AUTH_INVALID");
    assert!(!is_retryable_relay_error(&error));
}

#[test]
fn wrapped_relay_io_errors_remain_classifiable() {
    let error: anyhow::Error = std::io::Error::new(
        std::io::ErrorKind::ConnectionReset,
        "relay closed the TCP stream",
    )
    .into();
    assert!(is_retryable_relay_error(&error));
}

fn private_test_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "polygone-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn missing_peer_store_means_first_contact_only_when_file_is_absent() {
    let root = private_test_path("peers-absent");
    let path = root.join(".polygone").join("peers.json");
    assert!(load_peers_from_path(&path).unwrap().is_empty());
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn malformed_peer_store_is_an_error_not_an_empty_tofu_store() {
    let root = private_test_path("peers-malformed");
    let path = root.join(".polygone").join("peers.json");
    crate::identity::write_private_atomic(&path, b"{not-json").unwrap();
    let error = load_peers_from_path(&path).unwrap_err();
    assert!(error.to_string().contains("ancre de confiance invalide"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn peer_store_is_atomic_private_and_restricted_before_reading() {
    let root = private_test_path("peers-private");
    let path = root.join(".polygone").join("peers.json");
    let peers = HashMap::from([("alice".to_string(), "public-key-hex".to_string())]);
    save_peers_to_path(&path, &peers).unwrap();
    assert_eq!(load_peers_from_path(&path).unwrap(), peers);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(load_peers_from_path(&path).unwrap(), peers);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn peer_store_symlink_is_refused() {
    let root = private_test_path("peers-symlink");
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("outside.json");
    std::fs::write(&target, b"{}").unwrap();
    let link = root.join("peers.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(load_peers_from_path(&link).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"{}");
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn peer_store_symlink_parent_is_refused_even_when_store_is_missing() {
    let root = private_test_path("peers-parent-symlink");
    std::fs::create_dir_all(&root).unwrap();
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let private_dir = root.join(".polygone");
    std::os::unix::fs::symlink(&outside, &private_dir).unwrap();
    let path = private_dir.join("peers.json");
    assert!(load_peers_from_path(&path).is_err());
    assert!(!outside.join("peers.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn oversized_file_is_rejected_before_opening_a_relay_connection() {
    let sender = LocalIdentity::generate();
    let recipient = LocalIdentity::generate();
    let payload = vec![0x5a; 128 * 1024];
    let error = send_network(
        "127.0.0.1:1",
        &node_id(&recipient),
        &payload,
        Some("large.bin"),
        &recipient.kem_public_key().unwrap(),
        &sender,
    )
    .await
    .expect_err("the serialized Shamir share exceeds the 64 KiB relay frame cap");
    assert!(
        error.to_string().contains("enveloppe trop volumineuse"),
        "preflight must fail before trying the intentionally closed relay: {error}"
    );
}

#[test]
fn outgoing_relay_envelopes_are_rejected_above_the_wire_cap() {
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "0123456789abcdef".into(),
        to: "fedcba9876543210".into(),
        session: "oversized".into(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: vec![0; MAX_WIRE_LINE],
        sig: None,
        signer: None,
        name_ct: None,
        ts: Some(1),
    };
    let error = encode_net_envelope(&env).unwrap_err();
    assert!(error.to_string().contains("enveloppe trop volumineuse"));
}

#[tokio::test]
async fn relay_frame_reader_times_out_after_a_partial_frame_starts() {
    let (mut writer, stream) = tokio::io::duplex(64);
    tokio::io::AsyncWriteExt::write_all(&mut writer, b"{\"kind\":")
        .await
        .unwrap();
    let mut reader = tokio::io::BufReader::new(stream);
    let error = read_line_capped(
        &mut reader,
        MAX_WIRE_LINE,
        std::time::Duration::from_millis(20),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn relay_frame_reader_accepts_bounded_lines_and_strips_newline() {
    let (mut writer, stream) = tokio::io::duplex(64);
    tokio::io::AsyncWriteExt::write_all(&mut writer, b"hello relay\n")
        .await
        .unwrap();
    let mut reader = tokio::io::BufReader::new(stream);
    let line = read_line_capped(&mut reader, 32, FRAME_CONTINUATION_TIMEOUT)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line, b"hello relay");
}

#[tokio::test]
async fn relay_frame_reader_rejects_oversized_lines_before_unbounded_growth() {
    let (mut writer, stream) = tokio::io::duplex(64);
    tokio::io::AsyncWriteExt::write_all(&mut writer, b"0123456789\n")
        .await
        .unwrap();
    let mut reader = tokio::io::BufReader::new(stream);
    let error = read_line_capped(&mut reader, 8, FRAME_CONTINUATION_TIMEOUT)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

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

#[test]
fn announce_new_peer_reports_exact_full_blake3_key_fingerprint() {
    let older = LocalIdentity::generate();
    let new = LocalIdentity::generate();
    let mut peers = HashMap::new();
    peers.insert("older-peer".to_string(), older.sign_pk_hex.clone());
    peers.insert("new-peer".to_string(), new.sign_pk_hex.clone());
    let new_pk = polygone_core::sign::PublicKey::from_hex(&new.sign_pk_hex).unwrap();
    let expected = blake3::hash(new_pk.as_bytes()).to_hex().to_string();
    assert_eq!(
        peer_fingerprint(&peers, "new-peer").as_deref(),
        Some(expected.as_str())
    );
    assert_eq!(expected.len(), 64);
    assert_eq!(peer_fingerprint(&peers, "missing-peer"), None);
}

#[test]
fn tofu_peer_map_is_bounded_and_never_rebinds_an_existing_node() {
    let mut peers = HashMap::new();
    assert!(bind_peer_key(&mut peers, "alice", "key-a".into()));
    assert!(bind_peer_key(&mut peers, "alice", "key-a".into()));
    assert!(!bind_peer_key(&mut peers, "alice", "key-b".into()));
    for index in 0..MAX_KNOWN_PEERS - 1 {
        peers.insert(format!("peer-{index}"), "pinned-key".into());
    }
    assert_eq!(peers.len(), MAX_KNOWN_PEERS);
    assert!(!bind_peer_key(&mut peers, "new-peer", "new-key".into()));
    assert_eq!(peers.len(), MAX_KNOWN_PEERS);
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
fn full_pipeline_signed_message_round_trip() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message réseau test", &pk).unwrap();
    let session = "test-session-1";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();

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
    let mut completed: CompletedSessions = VecDeque::new();
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
    let mut completed: CompletedSessions = VecDeque::new();
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
fn tampered_fragment_is_detected_and_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("message intègre", &pk).unwrap();
    let session = "tamper-session";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();

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
    let mut completed: CompletedSessions = VecDeque::new();

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
    let mut completed: CompletedSessions = VecDeque::new();

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
    let mut completed: CompletedSessions = VecDeque::new();
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
    let output = crate::msg::send_bytes(&bytes, &pk).unwrap();
    let session = "test-session-file";
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();

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
                }
                _ => panic!("expected a file"),
            }
            return;
        }
    }
    panic!("file session never completed");
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
    let mut completed: CompletedSessions = VecDeque::new();

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
fn grant_reply_routes_back_and_seals_the_final_output_payload() {
    let ghost = LocalIdentity::generate();
    let borrower = LocalIdentity::generate();
    let borrower_node = node_id(&borrower);
    let ghost_node = node_id(&ghost);
    let req = NetEnvelope {
        kind: "fragment".into(),
        from: borrower_node.clone(),
        to: ghost_node.clone(),
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

    let key = symmetric::SessionKey::from_bytes([0x42; 32]);
    let mut body = grant_payload(&ghost);
    body["output"] = serde_json::Value::String("résultat signé".into());
    let grant = grant_for(&req, &ghost, body, &key).expect("grant signing succeeds");
    assert_eq!(grant.typ, "grant");
    assert_eq!(grant.to, borrower_node);
    assert_eq!(grant.session, "res-session-1");
    assert_eq!(grant.from, ghost_node);
    assert!(
        !String::from_utf8_lossy(&grant.payload).contains("résultat signé"),
        "le grant ne doit jamais porter le résultat en clair"
    );
    let plain = open_compute_reply(&grant.payload, &key).expect("grant decrypts");
    let body: serde_json::Value =
        serde_json::from_slice(&plain).expect("decrypted grant payload is JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["node"], node_id(&ghost));
    assert_eq!(body["output"], "résultat signé");

    // The requester accepts the exact sealed payload, but not a changed byte.
    let mut known = HashMap::new();
    assert!(verify_grant(
        &grant,
        &mut known,
        &node_id(&ghost),
        TrustPolicy::Tofu
    ));
    assert_eq!(known.get(&node_id(&ghost)), Some(&ghost.sign_pk_hex));

    let mut tampered = grant;
    let last = tampered.payload.len() - 1;
    tampered.payload[last] ^= 0x01;
    assert!(!verify_grant(
        &tampered,
        &mut known,
        &node_id(&ghost),
        TrustPolicy::Tofu
    ));
    assert!(open_compute_reply(&tampered.payload, &key).is_err());
}

#[test]
fn compute_e2e_hides_task_and_result_from_the_relay() {
    let borrower = LocalIdentity::generate();
    let ghost = LocalIdentity::generate();
    let ghost_kem = ghost.kem_public_key().unwrap();
    let task = "TASK_SECRET_7f3a9c";
    let result = "RESULT_SECRET_2b8d1e";
    let body = serde_json::json!({"action": "compute", "task": task});
    let plain = body.to_string().into_bytes();

    let (sealed, key) = seal_compute_request(&plain, &ghost_kem).unwrap();
    let wire = String::from_utf8_lossy(&sealed);
    assert!(!wire.contains(task), "le relais ne doit pas lire la tâche");
    let outer: ComputeEnvelope = serde_json::from_slice(&sealed).unwrap();
    assert_eq!(outer.v, COMPUTE_E2E_VERSION);
    assert!(outer.kem_ct.is_some(), "la demande doit porter un KEM_CT");

    let (opened, reply_key) = open_compute_request(&sealed, &ghost).unwrap();
    assert_eq!(opened, plain);
    assert!(
        open_compute_request(&sealed, &borrower).is_err(),
        "une autre identité ne peut pas ouvrir la tâche"
    );

    let reply = seal_compute_reply(result.as_bytes(), &key).unwrap();
    assert!(
        !String::from_utf8_lossy(&reply).contains(result),
        "le relais ne doit pas lire le résultat"
    );
    assert_eq!(
        open_compute_reply(&reply, &reply_key).unwrap(),
        result.as_bytes()
    );
    assert!(
        open_compute_reply(br#"{"ok":true}"#, &key).is_err(),
        "un grant en clair doit être refusé"
    );
}

#[test]
fn key_rotation_changes_the_node_id_and_old_grants_stay_with_the_old_key() {
    let borrower = LocalIdentity::generate();
    let mut ghost = LocalIdentity::generate();
    let old_node = node_id(&ghost);
    let old_pk = ghost.sign_pk_hex.clone();
    let key = symmetric::SessionKey::from_bytes([0x24; 32]);
    let req = NetEnvelope {
        kind: "fragment".into(),
        from: node_id(&borrower),
        to: old_node.clone(),
        session: "res-session-rotation".into(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: Vec::new(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };

    let mut known = HashMap::new();
    let before = grant_for(&req, &ghost, grant_payload(&ghost), &key).expect("grant signs");
    assert!(verify_grant(
        &before,
        &mut known,
        &old_node,
        TrustPolicy::Tofu
    ));
    assert_eq!(known.get(&old_node), Some(&old_pk));

    let outcome = ghost.rotate_sign_key().expect("rotation succeeds");
    let new_node = node_id(&ghost);
    assert_ne!(
        new_node, old_node,
        "le node_id dérive de la clé de signature : la rotation change l'adresse"
    );
    assert_ne!(outcome.new_pk_hex, old_pk);

    // The rotated ghost cannot speak for the old node id...
    let after = grant_for(&req, &ghost, grant_payload(&ghost), &key).expect("grant signs");
    assert!(
        !verify_grant(&after, &mut known, &old_node, TrustPolicy::Tofu),
        "a rotated key must not authenticate the previous node id"
    );
    // ...but rotation is local: the old key's grants still verify for the old
    // node id, so it is not a distributed revocation. This is the documented
    // limit, not an accident of this test.
    assert!(
        verify_grant(&before, &mut known, &old_node, TrustPolicy::Tofu),
        "sans révocation distribuée, l'ancienne clé garde l'ancien node_id"
    );
    assert!(!ghost.accepts_signature_from(&old_pk));
}

#[test]
fn local_recomputation_promotes_a_reproduced_grant_to_l1() {
    let grant = serde_json::json!({
        "node": "ghost",
        "ok": true,
        "output": "42\n",
        "verification": VERIFICATION_PRODUCER_SIGNED,
        "verification_limit": "la signature prouve l'auteur, pas la justesse du résultat",
    })
    .to_string();

    let (annotated, reproduced) = annotate_local_recomputation(&grant, "42\n").unwrap();
    assert!(
        reproduced,
        "identical output must be reported as reproduced"
    );
    let body: serde_json::Value = serde_json::from_str(&annotated).unwrap();
    assert_eq!(body["verification"], VERIFICATION_RECOMPUTED);
    assert_eq!(body["recompute"]["match"], true);
    let digest = blake3::hash(b"42\n").to_hex().to_string();
    assert_eq!(body["recompute"]["grant_output_blake3"], digest.as_str());
    assert_eq!(body["recompute"]["local_output_blake3"], digest.as_str());
}

#[test]
fn local_recomputation_keeps_a_divergent_grant_at_l0() {
    let grant = serde_json::json!({"node": "ghost", "ok": true, "output": "42\n"}).to_string();

    let (annotated, reproduced) = annotate_local_recomputation(&grant, "41\n").unwrap();
    assert!(!reproduced, "a different output must never be accepted");
    let body: serde_json::Value = serde_json::from_str(&annotated).unwrap();
    assert_eq!(body["verification"], VERIFICATION_PRODUCER_SIGNED);
    assert_eq!(body["recompute"]["match"], false);
    assert_ne!(
        body["recompute"]["grant_output_blake3"],
        body["recompute"]["local_output_blake3"]
    );
    assert!(body["verification_limit"]
        .as_str()
        .unwrap()
        .contains("diverge"));
}

#[test]
fn local_recomputation_of_a_refusal_claims_nothing() {
    let grant = r#"{"ok":false,"error":"sandbox refusée"}"#;

    let (annotated, reproduced) = annotate_local_recomputation(grant, "").unwrap();
    assert!(!reproduced);
    let body: serde_json::Value = serde_json::from_str(&annotated).unwrap();
    assert!(
        body.get("recompute").is_none(),
        "a refusal has no output to verify"
    );
    assert_eq!(body["error"], "sandbox refusée");
}

#[test]
fn compute_request_replay_cache_rejects_duplicates_and_expires() {
    let mut seen = HashSet::new();
    let mut order = VecDeque::new();
    assert_eq!(
        remember_compute_request(&mut seen, &mut order, "alice", "session-1", 100, 2),
        ComputeReplayDecision::Accepted
    );
    assert_eq!(
        remember_compute_request(&mut seen, &mut order, "alice", "session-1", 101, 2),
        ComputeReplayDecision::Duplicate
    );
    assert_eq!(
        remember_compute_request(&mut seen, &mut order, "bob", "session-1", 101, 2),
        ComputeReplayDecision::Accepted
    );
    assert_eq!(
        remember_compute_request(
            &mut seen,
            &mut order,
            "alice",
            "session-1",
            100 + COMPUTE_REPLAY_TTL_SECS + 1,
            2,
        ),
        ComputeReplayDecision::Accepted
    );
}

#[test]
fn compute_request_replay_cache_fails_closed_when_full() {
    let mut seen = HashSet::new();
    let mut order = VecDeque::new();
    assert_eq!(
        remember_compute_request(&mut seen, &mut order, "alice", "live-1", 100, 1),
        ComputeReplayDecision::Accepted
    );
    assert_eq!(
        remember_compute_request(&mut seen, &mut order, "bob", "live-2", 101, 1),
        ComputeReplayDecision::Capacity
    );
    assert!(seen.contains(&("alice".into(), "live-1".into())));
    assert_eq!(
        order.len(),
        1,
        "unexpired replay markers must not be evicted"
    );
}

// ── TOFU : premier contact appris, puis identité NON SUBSTITUABLE ───────
// La config production charge known_peers depuis peers.json (vide au
// premier contact) et l'apprend après un message vérifié. Avec S1, le
// `from` est dérivé de la clé de signature : une autre clé ne peut plus
// revendiquer l'identité d'un pair déjà ancré, même en premier contact.
#[test]
fn tofu_learns_and_enforces_the_anchor() {
    let eve = LocalIdentity::generate(); // premier contact légitime
    let alice = LocalIdentity::generate(); // tentative de substitution
    let bob = LocalIdentity::generate(); // la victime
    let pk = bob.kem_public_key().unwrap();
    let bob_node = node_id(&bob);
    let eve_node = node_id(&eve);

    let mut known_peers: HashMap<String, String> = HashMap::new(); // vide = prod fraîche
    let mut completed: CompletedSessions = VecDeque::new();

    // 1) Eve (from=eve, SA clé) → TOFU accepte et apprend le binding.
    let output = crate::msg::send("premier contact", &pk).unwrap();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let session1 = "tofu-1";
    let kem_line = signed_kem_json(session1, &eve, &bob_node, test_now(), &output, None);
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
        let line = frag_json(session1, &eve_node, &bob_node, frag.index, &frag.share);
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
    assert_eq!(
        known_peers.get(&eve_node).map(|s| s.as_str()),
        Some(eve.sign_pk_hex.as_str()),
        "le premier contact vérifié doit être appris (TOFU)"
    );

    // 2) Alice signe un message qui revendique `from = eve` : sa clé ne
    //    dérive pas cet identifiant, donc la session est refusée et le
    //    binding ancré n'est pas écrasé.
    let output2 = crate::msg::send("substitution", &pk).unwrap();
    let mut sessions2: HashMap<String, SessionBuffer> = HashMap::new();
    let session2 = "tofu-2";
    let ciphertext2 = output2.ciphertext.clone().expect("ciphertext");
    let sig2 = alice.sign_signer().unwrap().sign(&canonical_bytes(
        session2,
        &eve_node,
        &bob_node,
        test_now(),
        output2.kem_ct.as_bytes(),
        &ciphertext2,
    ));
    let env2 = NetEnvelope {
        kind: "fragment".into(),
        from: eve_node.clone(),
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
    let mut last_refusal = None;
    for frag in output2.fragments.iter().take(4) {
        let line = frag_json(session2, &eve_node, &bob_node, frag.index, &frag.share);
        let (received, refusal) = process_line_detailed(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions2,
        )
        .unwrap();
        assert!(
            received.is_none(),
            "une identité substituée ne doit jamais être livrée"
        );
        last_refusal = refusal;
    }
    assert_eq!(last_refusal, Some(Refusal::IdentityMismatch));
    // Le premier from ancré est conservé (pas écrasé par le second).
    assert_eq!(
        known_peers.get(&eve_node).map(|s| s.as_str()),
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
    let mut completed: CompletedSessions = VecDeque::new();
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
fn stale_timestamp_is_rejected() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("vieux message", &pk).unwrap();
    let bob_node = node_id(&bob);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();
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
fn completed_session_cache_fails_closed_instead_of_evicting_live_replay_markers() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut completed: CompletedSessions = (0..MAX_COMPLETED)
        .map(|index| {
            (
                format!("sender-{index}"),
                format!("session-{index}"),
                test_now(),
            )
        })
        .collect();
    let mut sessions = HashMap::new();
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "fresh-sender".into(),
        to: bob_node,
        session: "fresh-session".into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: vec![0; 32],
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };
    let line = serde_json::to_string(&env).unwrap();
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
    assert!(
        sessions.is_empty(),
        "full live replay cache must fail closed"
    );
    assert_eq!(completed.len(), MAX_COMPLETED);

    for (_, _, completed_at) in completed.iter_mut() {
        *completed_at = test_now().saturating_sub(MAX_TS_SKEW_SECS + 1);
    }
    let _ = process_line(
        &line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert!(
        completed.is_empty(),
        "expired replay markers must be purged"
    );
    assert_eq!(
        sessions.len(),
        1,
        "a fresh session can proceed after expiry"
    );
}

#[test]
fn session_cap_rejects_new_sessions() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();

    // Fill the map to the cap with dummy (from|session) keys.
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    for i in 0..MAX_SESSIONS {
        sessions.insert(
            format!("node{}|s{}", i % 7, i),
            SessionBuffer {
                touched: test_now(),
                ..Default::default()
            },
        );
    }
    assert_eq!(sessions.len(), MAX_SESSIONS);

    // A NEW session beyond the cap → fail closed, nothing buffered.
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "attacker".into(),
        to: bob_node,
        session: "fresh-session".into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: vec![0u8; 32],
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };
    let r = process_line(
        &serde_json::to_string(&env).unwrap(),
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
fn expired_sessions_are_purged_before_a_full_session_cap_rejects_new_work() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions = HashMap::new();
    for i in 0..MAX_SESSIONS {
        sessions.insert(
            format!("stale-node-{i}|stale-session-{i}"),
            SessionBuffer {
                touched: test_now().saturating_sub(SESSION_TTL_SECS + 1),
                ..Default::default()
            },
        );
    }
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: "fresh-node".into(),
        to: bob_node,
        session: "fresh-session-after-expiry".into(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: vec![0u8; 32],
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    };
    let _ = process_line(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();
    assert_eq!(
        sessions.len(),
        1,
        "expired entries must be purged before insertion"
    );
    assert!(sessions.contains_key("fresh-node|fresh-session-after-expiry"));
}

#[test]
fn out_of_range_fragment_index_is_dropped() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("idx hors bornes", &pk).unwrap();
    let bob_node = node_id(&bob);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();
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
fn test_session_ttl_matches_the_published_promise() {
    // MSG.1 : la constante valait 300 s alors que trois documents publiés
    // annonçaient 30 s. Ce test relie les deux, dans les deux sens.
    assert_eq!(
        SESSION_TTL_SECS, PUBLISHED_SESSION_TTL_SECS,
        "la valeur implémentée et la valeur publiée doivent être la même"
    );

    let expected = format!("≤ {} s", PUBLISHED_SESSION_TTL_SECS);
    let documents: [(&str, &str); 3] = [
        ("LEGAL.md", include_str!("../../../../LEGAL.md")),
        (
            "THREAT_MODEL.md",
            include_str!("../../../../THREAT_MODEL.md"),
        ),
        (
            "docs/threat-commodity.md",
            include_str!("../../../../docs/threat-commodity.md"),
        ),
    ];
    for (name, text) in documents {
        assert!(
            text.contains(&expected),
            "{name} doit annoncer « {expected} » : c'est la durée que le code applique"
        );
    }

    // Et le relais ne doit pas être décrit comme stockant : il ne stocke rien.
    assert!(
        !documents[0].1.contains("≤ 30 s sur relay"),
        "LEGAL.md ne doit pas situer le TTL sur le relais, qui ne conserve aucun fragment"
    );
}

/// Lit une trame livrée par le relais, avec une borne : un test qui attend
/// indéfiniment est un test qui ment sur son résultat.
async fn read_relay_line(
    reader: &mut tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> String {
    use tokio::io::AsyncBufReadExt as _;
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        reader.read_line(&mut line),
    )
    .await
    .expect("le relais doit livrer la trame dans les 2 s")
    .expect("lecture relay");
    assert!(!line.is_empty(), "connexion fermée par le relais");
    line
}

/// Écrit une rafale de trames d'un seul coup : le relais reçoit exactement les
/// octets d'origine, sans recomposition côté test.
async fn send_frames(writer: &mut tokio::net::tcp::OwnedWriteHalf, frames: &[String]) {
    use tokio::io::AsyncWriteExt as _;
    let bytes: Vec<u8> = frames
        .iter()
        .flat_map(|frame| format!("{frame}\n").into_bytes())
        .collect();
    writer.write_all(&bytes).await.unwrap();
}

/// SEC.3 — l'anti-rejeu prouvé **sur le fil**, à travers le vrai relais.
///
/// Le relais est aveugle : il ne refuse rien, il recopie. Le refus vient donc
/// du récepteur, et il doit porter un code. Ce test fait traverser une vraie
/// socket TCP et le vrai `polygone_relay` aux octets d'une session valide,
/// les capture, puis les renvoie **à l'identique** :
///
///   1. session valide → déchiffrée (témoin positif, sans quoi un relais qui
///      ne route plus passerait le test) ;
///   2. rejeu immédiat  → `REPLAY.SESSION_ALREADY_COMPLETED` ;
///   3. rejeu à +10 min → `REPLAY.STALE_TIMESTAMP`.
///
/// Limite assumée : un adversaire qui contrôle l'horloge des deux pairs peut
/// rester dans la fenêtre ; c'est écrit dans `docs/threat-commodity.md`.
#[tokio::test]
async fn replay_through_relay_is_refused_with_an_explicit_code() {
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::{TcpListener, TcpStream};

    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let alice_node = node_id(&alice);

    // Le VRAI relais, sur un port éphémère.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        // This test exercises replay refusal, not the handshake: keep the
        // legacy registration explicit so the fixture stays minimal.
        let _ = polygone_relay::relay::run_listener_with_options(listener, true).await;
    });

    // Le récepteur s'enregistre ; HELLO_OK prouve qu'il possède son slot.
    let bob_stream = TcpStream::connect(relay_addr).await.unwrap();
    let (bob_read, mut bob_write) = bob_stream.into_split();
    let mut bob_reader = BufReader::new(bob_read);
    bob_write
        .write_all(format!("HELLO {bob_node}\n").as_bytes())
        .await
        .unwrap();
    let mut ack = String::new();
    bob_reader.read_line(&mut ack).await.unwrap();
    assert_eq!(
        ack.trim(),
        "HELLO_OK",
        "le relais doit accepter bob : {ack}"
    );

    // L'émetteur, sur le même relais.
    let alice_stream = TcpStream::connect(relay_addr).await.unwrap();
    let (alice_read, mut alice_write) = alice_stream.into_split();
    let mut alice_reader = BufReader::new(alice_read);
    alice_write
        .write_all(format!("HELLO {alice_node}\n").as_bytes())
        .await
        .unwrap();
    let mut aack = String::new();
    alice_reader.read_line(&mut aack).await.unwrap();
    assert_eq!(
        aack.trim(),
        "HELLO_OK",
        "le relais doit accepter alice : {aack}"
    );

    // Une session valide, exactement comme `envoyer` la produit.
    let output =
        crate::msg::send("message rejoué sur le fil", &bob.kem_public_key().unwrap()).unwrap();
    let session = "replay-wire-1";
    let mut frames = vec![signed_kem_json(
        session,
        &alice,
        &bob_node,
        test_now(),
        &output,
        None,
    )];
    for frag in output.fragments.iter().take(4) {
        frames.push(frag_json(
            session,
            &alice_node,
            &bob_node,
            frag.index,
            &frag.share,
        ));
    }

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    // 1) Témoin positif : la session traverse le relais et se déchiffre.
    send_frames(&mut alice_write, &frames).await;
    let mut delivered = None;
    for _ in 0..frames.len() {
        let line = read_relay_line(&mut bob_reader).await;
        let (outcome, refusal) = process_line_detailed(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert_eq!(refusal, None, "trame valide refusée : {refusal:?}");
        if let Some((_, received)) = outcome {
            delivered = Some(received);
        }
    }
    match delivered.expect("la session valide doit aboutir à travers le relais") {
        Received::Message(text) => assert_eq!(text, "message rejoué sur le fil"),
        _ => panic!("message attendu"),
    }

    // 2) Rejeu immédiat, octets identiques : refus explicite, jamais livré.
    send_frames(&mut alice_write, &frames).await;
    for _ in 0..frames.len() {
        let line = read_relay_line(&mut bob_reader).await;
        let (outcome, refusal) = process_line_detailed(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(outcome.is_none(), "un rejeu ne doit jamais être livré");
        assert_eq!(
            refusal.map(Refusal::code),
            Some("REPLAY.SESSION_ALREADY_COMPLETED"),
            "rejeu immédiat : {refusal:?}"
        );
    }

    // 3) Rejeu décalé de 10 minutes, vers un récepteur FRAIS : processus
    //    redémarré, cache anti-rejeu vide. Le cache ne peut donc pas sauver
    //    la mise — c'est la fenêtre signée ±300 s qui doit refuser, et elle
    //    le dit avec son propre code.
    let later = test_now() + 2 * MAX_TS_SKEW_SECS;
    let mut fresh_completed: CompletedSessions = VecDeque::new();
    let mut fresh_sessions: HashMap<String, SessionBuffer> = HashMap::new();
    send_frames(&mut alice_write, &frames).await;
    let mut stale_refusals = 0;
    for _ in 0..frames.len() {
        let line = read_relay_line(&mut bob_reader).await;
        let (outcome, refusal) = process_line_detailed(
            &line,
            &bob,
            &mut known_peers,
            later,
            &mut fresh_completed,
            &mut fresh_sessions,
        )
        .unwrap();
        assert!(
            outcome.is_none(),
            "un rejeu décalé ne doit jamais être livré"
        );
        if refusal.map(Refusal::code) == Some("REPLAY.STALE_TIMESTAMP") {
            stale_refusals += 1;
        }
    }
    assert_eq!(
        stale_refusals, 1,
        "le rejeu décalé doit être refusé une fois, avec son code"
    );
    assert!(
        fresh_completed.is_empty(),
        "une session refusée pour horodatage périmé ne doit pas être mémorisée comme livrée"
    );
}

// ── Politique de confiance : TOFU par défaut, strict sur demande ─────────

#[test]
fn trust_policy_parses_only_the_documented_values() {
    assert_eq!(TrustPolicy::parse(None).unwrap(), TrustPolicy::Tofu);
    assert_eq!(TrustPolicy::parse(Some("")).unwrap(), TrustPolicy::Tofu);
    assert_eq!(TrustPolicy::parse(Some("tofu")).unwrap(), TrustPolicy::Tofu);
    assert_eq!(
        TrustPolicy::parse(Some("strict")).unwrap(),
        TrustPolicy::Strict
    );
    let error = TrustPolicy::parse(Some("strcit")).unwrap_err();
    assert!(
        error.to_string().contains("tofu, strict"),
        "une valeur inconnue doit nommer les valeurs acceptées, pas retomber sur TOFU : {error}"
    );
}

/// En mode strict, un pair inconnu est refusé **avant** que le message ne soit
/// livré, et son identité n'est jamais apprise : c'est la différence avec TOFU.
#[test]
fn strict_policy_refuses_an_unknown_peer_without_learning_it() {
    let eve = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let bob_node = node_id(&bob);
    let eve_node = node_id(&eve);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: CompletedSessions = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    let output = crate::msg::send("inconnu", &pk).unwrap();
    let session = "strict-unknown";
    let kem_line = signed_kem_json(session, &eve, &bob_node, test_now(), &output, None);
    let (_, refusal) = process_line_detailed_with_policy(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
        TrustPolicy::Strict,
    )
    .unwrap();
    assert_eq!(
        refusal, None,
        "la trame KEM seule n'est pas encore un refus"
    );

    let mut last_refusal = None;
    for frag in output.fragments.iter().take(4) {
        let line = frag_json(session, &eve_node, &bob_node, frag.index, &frag.share);
        let (received, refusal) = process_line_detailed_with_policy(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
            TrustPolicy::Strict,
        )
        .unwrap();
        assert!(
            received.is_none(),
            "un pair inconnu ne doit jamais être livré en mode strict"
        );
        last_refusal = refusal.or(last_refusal);
    }
    assert_eq!(last_refusal, Some(Refusal::UnknownPeer));
    assert!(
        known_peers.is_empty(),
        "le mode strict ne doit pas apprendre l'ancre d'un pair inconnu"
    );
}

/// Le même message passe dès que la clé du pair est ancrée hors-ligne.
#[test]
fn strict_policy_accepts_an_anchored_peer() {
    let alice = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let pk = bob.kem_public_key().unwrap();
    let bob_node = node_id(&bob);
    let alice_node = node_id(&alice);

    let mut known_peers: HashMap<String, String> = HashMap::new();
    known_peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());
    let mut completed: CompletedSessions = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    let output = crate::msg::send("ancré", &pk).unwrap();
    let session = "strict-known";
    let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
    process_line_detailed_with_policy(
        &kem_line,
        &bob,
        &mut known_peers,
        test_now(),
        &mut completed,
        &mut sessions,
        TrustPolicy::Strict,
    )
    .unwrap();

    let mut delivered = None;
    // Quatre fragments suffisent : au-delà, la session est déjà complétée et
    // le refus attendu est `AlreadyCompleted`, pas un refus de confiance.
    for frag in output.fragments.iter().take(4) {
        let line = frag_json(session, &alice_node, &bob_node, frag.index, &frag.share);
        let (received, refusal) = process_line_detailed_with_policy(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
            TrustPolicy::Strict,
        )
        .unwrap();
        assert_eq!(refusal, None, "un pair ancré ne doit pas être refusé");
        if let Some((_, received)) = received {
            delivered = Some(received);
        }
    }
    match delivered.expect("le message d'un pair ancré doit être livré en mode strict") {
        Received::Message(text) => assert_eq!(text, "ancré"),
        _ => panic!("message attendu"),
    }
}
