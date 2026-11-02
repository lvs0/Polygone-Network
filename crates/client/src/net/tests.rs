use super::*;
use crate::identity::LocalIdentity;

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
fn grant_reply_routes_back_and_signs_the_final_output_payload() {
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

    let mut body = grant_payload(&ghost);
    body["output"] = serde_json::Value::String("résultat signé".into());
    let grant = grant_for(&req, &ghost, body).expect("grant signing succeeds");
    assert_eq!(grant.typ, "grant");
    assert_eq!(grant.to, borrower_node);
    assert_eq!(grant.session, "res-session-1");
    assert_eq!(grant.from, ghost_node);
    let body: serde_json::Value =
        serde_json::from_slice(&grant.payload).expect("grant payload is JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["node"], node_id(&ghost));
    assert_eq!(body["output"], "résultat signé");

    // The requester accepts the exact final payload, but not a changed output.
    let mut known = HashMap::new();
    assert!(verify_grant(&grant, &mut known, &node_id(&ghost)));
    assert_eq!(known.get(&node_id(&ghost)), Some(&ghost.sign_pk_hex));

    let mut tampered = grant;
    let mut changed: serde_json::Value = serde_json::from_slice(&tampered.payload).unwrap();
    changed["output"] = serde_json::Value::String("résultat falsifié".into());
    tampered.payload = changed.to_string().into_bytes();
    assert!(!verify_grant(&tampered, &mut known, &node_id(&ghost)));
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
    let mut completed: CompletedSessions = VecDeque::new();

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
