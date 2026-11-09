use super::*;

use polygone_core::crypto::kem::{self, KemCiphertext, KemPublicKey, KemSecretKey};
use polygone_core::sign;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

async fn spawn_strict_relay() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = run_listener_with_options(listener, false).await;
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

async fn read_line(stream: &mut TcpStream) -> String {
    let mut line = String::new();
    let mut reader = BufReader::new(&mut *stream);
    reader.read_line(&mut line).await.unwrap();
    line
}

/// Send `AUTH_INIT`, consume `CHALLENGE`, and return the fields needed to
/// build a response.
async fn start_auth(
    stream: &mut TcpStream,
    kem_pk: &KemPublicKey,
    sign_kp: &sign::KeyPair,
) -> (String, String, String) {
    let sign_pk_hex = sign_kp.verifier.public_key().to_hex();
    let node_id = sign::node_id_from_sign_pk_hex(&sign_pk_hex).expect("node id");
    let kem_pk_hex = kem_pk.to_hex();
    stream
        .write_all(format!("AUTH_INIT {node_id} {sign_pk_hex} {kem_pk_hex}\n").as_bytes())
        .await
        .unwrap();
    let challenge = read_line(stream).await;
    let ct_hex = challenge
        .trim()
        .strip_prefix("CHALLENGE ")
        .unwrap_or_else(|| panic!("expected CHALLENGE, got {challenge:?}"))
        .to_string();
    (node_id, sign_pk_hex, ct_hex)
}

fn auth_resp(
    node_id: &str,
    sign_pk_hex: &str,
    kem_pk_hex: &str,
    ct_hex: &str,
    kem_sk: &KemSecretKey,
    sign_kp: &sign::KeyPair,
) -> String {
    let ct = KemCiphertext::from_hex(ct_hex).expect("ciphertext");
    let shared = kem::decapsulate(kem_sk, &ct).expect("decapsulate");
    let token = sign::relay_auth_token(&shared.0);
    let transcript = sign::relay_auth_transcript(node_id, sign_pk_hex, kem_pk_hex, ct_hex);
    let signature = sign_kp.signer.sign(&transcript);
    format!(
        "AUTH_RESP {} {token}\n",
        sign::encode_hex(signature.as_bytes())
    )
}

async fn auth_hello(
    stream: &mut TcpStream,
    kem_pk: &KemPublicKey,
    kem_sk: &KemSecretKey,
    sign_kp: &sign::KeyPair,
) -> String {
    let (node_id, sign_pk_hex, ct_hex) = start_auth(stream, kem_pk, sign_kp).await;
    let response = auth_resp(
        &node_id,
        &sign_pk_hex,
        &kem_pk.to_hex(),
        &ct_hex,
        kem_sk,
        sign_kp,
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    let ack = read_line(stream).await;
    assert_eq!(ack.trim(), "AUTH_OK", "handshake failed: {ack}");
    node_id
}

#[tokio::test]
async fn strict_relay_refuses_legacy_hello() {
    let addr = spawn_strict_relay().await;
    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(b"HELLO bob\n").await.unwrap();
    let ack = read_line(&mut client).await;
    assert!(
        ack.starts_with("HELLO_DENIED"),
        "legacy HELLO was accepted: {ack}"
    );
}

#[tokio::test]
async fn authenticated_peers_can_route_fragments() {
    let addr = spawn_strict_relay().await;
    let (bob_pk, bob_sk) = kem::generate_keypair().unwrap();
    let bob_sign = sign::generate_keypair().unwrap();
    let (alice_pk, alice_sk) = kem::generate_keypair().unwrap();
    let alice_sign = sign::generate_keypair().unwrap();

    let mut bob = TcpStream::connect(addr).await.unwrap();
    let bob_id = auth_hello(&mut bob, &bob_pk, &bob_sk, &bob_sign).await;
    let mut alice = TcpStream::connect(addr).await.unwrap();
    let alice_id = auth_hello(&mut alice, &alice_pk, &alice_sk, &alice_sign).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = format!(
        "{{\"kind\":\"fragment\",\"from\":\"{alice_id}\",\"to\":\"{bob_id}\",\"session\":\"s1\",\"seq\":1,\"payload\":[1,2,3]}}\n"
    );
    alice.write_all(envelope.as_bytes()).await.unwrap();

    let line = tokio::time::timeout(Duration::from_secs(2), read_line(&mut bob))
        .await
        .expect("fragment timed out");
    assert!(line.contains("\"session\":\"s1\""), "forwarded: {line}");
}

#[tokio::test]
async fn a_valid_token_with_a_wrong_signature_is_refused() {
    let addr = spawn_strict_relay().await;
    let (kem_pk, kem_sk) = kem::generate_keypair().unwrap();
    let alice = sign::generate_keypair().unwrap();
    let mallory = sign::generate_keypair().unwrap();

    let mut client = TcpStream::connect(addr).await.unwrap();
    let (node_id, sign_pk_hex, ct_hex) = start_auth(&mut client, &kem_pk, &alice).await;
    // Correct KEM proof, but the transcript is signed by Mallory.
    let response = auth_resp(
        &node_id,
        &sign_pk_hex,
        &kem_pk.to_hex(),
        &ct_hex,
        &kem_sk,
        &mallory,
    );
    client.write_all(response.as_bytes()).await.unwrap();
    let ack = read_line(&mut client).await;
    assert!(
        ack.starts_with("AUTH_DENIED bad_signature"),
        "wrong signer was accepted: {ack}"
    );
}

#[tokio::test]
async fn a_valid_signature_without_the_kem_secret_is_refused() {
    let addr = spawn_strict_relay().await;
    let (alice_pk, _alice_sk) = kem::generate_keypair().unwrap();
    let alice_sign = sign::generate_keypair().unwrap();

    let mut client = TcpStream::connect(addr).await.unwrap();
    let (node_id, sign_pk_hex, ct_hex) = start_auth(&mut client, &alice_pk, &alice_sign).await;
    let transcript =
        sign::relay_auth_transcript(&node_id, &sign_pk_hex, &alice_pk.to_hex(), &ct_hex);
    let signature = alice_sign.signer.sign(&transcript);
    let guessed_token = sign::encode_hex(&[0u8; 32]);
    client
        .write_all(
            format!(
                "AUTH_RESP {} {guessed_token}\n",
                sign::encode_hex(signature.as_bytes())
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let ack = read_line(&mut client).await;
    assert!(
        ack.starts_with("AUTH_DENIED invalid_token"),
        "a guessed KEM token was accepted: {ack}"
    );
}

#[tokio::test]
async fn a_node_id_not_derived_from_the_signing_key_is_refused() {
    let addr = spawn_strict_relay().await;
    let (kem_pk, _kem_sk) = kem::generate_keypair().unwrap();
    let sign_kp = sign::generate_keypair().unwrap();
    let mut client = TcpStream::connect(addr).await.unwrap();

    let fake_id = "00".repeat(16);
    client
        .write_all(
            format!(
                "AUTH_INIT {fake_id} {} {}\n",
                sign_kp.verifier.public_key().to_hex(),
                kem_pk.to_hex()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let ack = read_line(&mut client).await;
    assert!(
        ack.starts_with("AUTH_DENIED identity_mismatch"),
        "mismatched node id was accepted: {ack}"
    );
}

#[tokio::test]
async fn replaying_auth_resp_on_a_fresh_challenge_is_refused() {
    let addr = spawn_strict_relay().await;
    let (kem_pk, kem_sk) = kem::generate_keypair().unwrap();
    let sign_kp = sign::generate_keypair().unwrap();

    let mut first = TcpStream::connect(addr).await.unwrap();
    let (node_id, sign_pk_hex, ct_hex) = start_auth(&mut first, &kem_pk, &sign_kp).await;
    let captured = auth_resp(
        &node_id,
        &sign_pk_hex,
        &kem_pk.to_hex(),
        &ct_hex,
        &kem_sk,
        &sign_kp,
    );
    first.write_all(captured.as_bytes()).await.unwrap();
    assert_eq!(read_line(&mut first).await.trim(), "AUTH_OK");

    let mut second = TcpStream::connect(addr).await.unwrap();
    let _ = start_auth(&mut second, &kem_pk, &sign_kp).await;
    second.write_all(captured.as_bytes()).await.unwrap();
    let ack = read_line(&mut second).await;
    assert!(
        ack.starts_with("AUTH_DENIED invalid_token"),
        "a replayed AUTH_RESP was accepted: {ack}"
    );
}

/// P0: after an abrupt disconnect — no goodbye, no protocol message — the relay
/// must release the routing slot so the same identity can come back, and the
/// returned slot must be live (a third connection is refused, not granted).
#[tokio::test]
async fn an_abruptly_disconnected_peer_can_reconnect_with_the_same_identity() {
    let addr = spawn_strict_relay().await;
    let (kem_pk, kem_sk) = kem::generate_keypair().unwrap();
    let sign_kp = sign::generate_keypair().unwrap();

    let mut first = TcpStream::connect(addr).await.unwrap();
    let first_id = auth_hello(&mut first, &kem_pk, &kem_sk, &sign_kp).await;
    drop(first); // abrupt close: the peer never says goodbye
    tokio::time::sleep(Duration::from_millis(150)).await;

    let mut second = TcpStream::connect(addr).await.unwrap();
    let second_id = auth_hello(&mut second, &kem_pk, &kem_sk, &sign_kp).await;
    assert_eq!(
        second_id, first_id,
        "l'identité doit être stable d'une connexion à l'autre"
    );

    // The reconnected slot is live: a third connection with the same keys is
    // refused. Without this witness, a relay that had simply stopped
    // registering anyone would pass the reconnection assertion.
    let mut third = TcpStream::connect(addr).await.unwrap();
    let (node_id, sign_pk_hex, ct_hex) = start_auth(&mut third, &kem_pk, &sign_kp).await;
    let response = auth_resp(
        &node_id,
        &sign_pk_hex,
        &kem_pk.to_hex(),
        &ct_hex,
        &kem_sk,
        &sign_kp,
    );
    third.write_all(response.as_bytes()).await.unwrap();
    let ack = read_line(&mut third).await;
    assert!(
        ack.starts_with("AUTH_DENIED duplicate"),
        "la reconnexion a laissé un slot mort ou volable : {ack}"
    );
}

#[tokio::test]
async fn a_second_connection_cannot_steal_a_live_authenticated_slot() {
    let addr = spawn_strict_relay().await;
    let (kem_pk, kem_sk) = kem::generate_keypair().unwrap();
    let sign_kp = sign::generate_keypair().unwrap();

    let mut first = TcpStream::connect(addr).await.unwrap();
    let _ = auth_hello(&mut first, &kem_pk, &kem_sk, &sign_kp).await;

    let mut second = TcpStream::connect(addr).await.unwrap();
    let (node_id, sign_pk_hex, ct_hex) = start_auth(&mut second, &kem_pk, &sign_kp).await;
    let response = auth_resp(
        &node_id,
        &sign_pk_hex,
        &kem_pk.to_hex(),
        &ct_hex,
        &kem_sk,
        &sign_kp,
    );
    second.write_all(response.as_bytes()).await.unwrap();
    let ack = read_line(&mut second).await;
    assert!(
        ack.starts_with("AUTH_DENIED duplicate"),
        "a live authenticated slot was stolen: {ack}"
    );
}
