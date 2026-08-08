
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

    let grant = grant_for(&req, &ghost);
    assert_eq!(grant.typ, "grant");
    assert_eq!(grant.to, "borrower-node"); // routed back to the requester
    assert_eq!(grant.session, "res-session-1"); // same session context
    assert_eq!(grant.from, node_id(&ghost)); // granted by the ghost
    let body: serde_json::Value =
        serde_json::from_slice(&grant.payload).expect("grant payload is JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["node"], node_id(&ghost));
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
fn session_cap_rejects_new_sessions() {
    let bob = LocalIdentity::generate();
    let bob_node = node_id(&bob);
    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();

    // Fill the map to the cap with dummy (from|session) keys.
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    for i in 0..MAX_SESSIONS {
        sessions.insert(format!("node{}|s{}", i % 7, i), SessionBuffer::default());
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
