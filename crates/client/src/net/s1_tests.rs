use super::*;
use crate::identity::LocalIdentity;
use std::collections::{HashMap, VecDeque};

fn now() -> u64 {
    1_700_000_000
}

/// The S1 regression: a hostile relay knows every public key and can mint
/// envelopes, but it does not know the signing key that names the victim.
/// Before the fix, an empty trust cache made TOFU accept the first forged
/// binding; the receiver must now reject it before learning anything.
#[test]
fn a_forged_from_is_rejected_even_with_an_empty_trust_cache() {
    let alice = LocalIdentity::generate();
    let eve = LocalIdentity::generate();
    let bob = LocalIdentity::generate();
    let alice_node = node_id(&alice);
    let bob_node = node_id(&bob);

    let pk = bob.kem_public_key().unwrap();
    let output = crate::msg::send("faux message d'alice", &pk).unwrap();
    let session = "s1-forged-empty-cache";
    let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
    let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
        session,
        &alice_node,
        &bob_node,
        now(),
        output.kem_ct.as_bytes(),
        &ciphertext,
    ));
    let env = NetEnvelope {
        kind: "fragment".into(),
        from: alice_node.clone(), // claims to be Alice
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
        ts: Some(now()),
    };

    let mut known_peers: HashMap<String, String> = HashMap::new();
    let mut completed = VecDeque::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let (received, refusal) = process_line_detailed(
        &serde_json::to_string(&env).unwrap(),
        &bob,
        &mut known_peers,
        now(),
        &mut completed,
        &mut sessions,
    )
    .unwrap();

    assert!(received.is_none());
    assert_eq!(refusal, None);

    // A KEM envelope alone does not complete a session: the identity is
    // checked when the fourth fragment closes it. Use the real shares so the
    // only reason for refusal is the forged `from`.
    let mut last_refusal = None;
    for frag in output.fragments.iter().take(4) {
        let line = frag_json(session, &alice_node, &bob_node, frag.index, &frag.share);
        let (received, refusal) = process_line_detailed(
            &line,
            &bob,
            &mut known_peers,
            now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(received.is_none());
        last_refusal = refusal;
    }

    assert_eq!(last_refusal, Some(Refusal::IdentityMismatch));
    assert!(
        known_peers.is_empty(),
        "TOFU must not learn a binding from a forged `from`"
    );
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

#[test]
fn node_id_is_derived_from_the_signing_key() {
    let identity = LocalIdentity::generate();
    let expected =
        polygone_core::sign::node_id_from_sign_pk_hex(&identity.sign_pk_hex).expect("valid key");

    assert_eq!(node_id(&identity), expected);
    assert_eq!(node_id(&identity).len(), 32);
    assert_ne!(
        node_id(&identity),
        identity.kem_pk_hex.chars().take(16).collect::<String>()
    );
}
