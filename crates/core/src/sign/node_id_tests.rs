use super::*;

#[test]
fn node_id_is_bound_to_the_signing_key() {
    let a = generate_keypair().unwrap();
    let b = generate_keypair().unwrap();
    let a_pk = a.verifier.public_key().to_hex();
    let b_pk = b.verifier.public_key().to_hex();
    let a_id = node_id_from_sign_pk_hex(&a_pk).expect("valid key");
    let b_id = node_id_from_sign_pk_hex(&b_pk).expect("valid key");

    assert_eq!(a_id.len(), 32);
    assert_eq!(a_id, node_id_from_sign_pk_hex(&a_pk).unwrap());
    assert_ne!(
        a_id, b_id,
        "different signing keys must name different nodes"
    );
}

#[test]
fn node_id_rejects_malformed_keys() {
    assert_eq!(node_id_from_sign_pk_hex(""), None);
    assert_eq!(node_id_from_sign_pk_hex("00"), None);
    assert_eq!(
        node_id_from_sign_pk_hex(&"zz".repeat(PUBLIC_KEY_SIZE / 2)),
        None
    );
    assert_eq!(
        node_id_from_sign_pk_hex(&"00".repeat(PUBLIC_KEY_SIZE / 2 - 1)),
        None
    );
}

#[test]
fn relay_transcript_separates_fields() {
    let reference = relay_auth_transcript("aa", "bb", "cc", "dd");
    assert_eq!(reference, relay_auth_transcript("aa", "bb", "cc", "dd"));
    assert_ne!(reference, relay_auth_transcript("aabb", "", "cc", "dd"));
    assert_ne!(reference, relay_auth_transcript("aa", "bb", "cc", "de"));
}
