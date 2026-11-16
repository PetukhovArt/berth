use berth_hub::client::HubClient;
use berth_hub::{serve, TlsFiles};
use rcgen::{CertificateParams, KeyPair, SanType};
use std::path::{Path, PathBuf};

fn throwaway_identity(dir: &Path) -> (TlsFiles, PathBuf) {
    let mut params = CertificateParams::default();
    params.subject_alt_names = vec![SanType::IpAddress("127.0.0.1".parse().unwrap())];
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    let (cert_path, key_path) = (dir.join("hub.pem"), dir.join("hub.key"));
    std::fs::write(&cert_path, cert.pem()).unwrap();
    std::fs::write(&key_path, key.serialize_pem()).unwrap();
    (
        TlsFiles {
            cert: cert_path.clone(),
            key: key_path,
        },
        cert_path,
    )
}

#[tokio::test]
async fn a_self_signed_hub_is_reached_only_through_the_extra_ca() {
    let dir = tempfile::tempdir().unwrap();
    let (files, ca) = throwaway_identity(dir.path());
    let hub = serve(
        &dir.path().join("hub.db"),
        "127.0.0.1:0".parse().unwrap(),
        Some("admin-secret".into()),
        Some(files),
    )
    .await
    .unwrap();
    let url = format!("https://{}", hub.addr);

    let untrusting = HubClient::new(&url, "admin-secret").unwrap();
    assert!(untrusting.devices().await.is_err());

    let admin = HubClient::trusting(&url, "admin-secret", Some(&ca)).unwrap();
    assert!(admin.devices().await.unwrap().is_empty());

    let device = HubClient::trusting(&url, "", Some(&ca)).unwrap();
    let (id, token) = device.pair_device("ann").await.unwrap();
    admin
        .approve(&id, &["read", "post", "create"])
        .await
        .unwrap();
    let ann = HubClient::trusting(&url, &token, Some(&ca)).unwrap();
    ann.create_channel("PROJ-1").await.unwrap();
    let mut live = ann.subscribe(None).await.unwrap();
    ann.post("PROJ-1", "over tls", None).await.unwrap();
    let got = live.next().await.unwrap().unwrap();
    assert_eq!(got.content, "over tls");

    assert!(HubClient::trusting(&url, "x", Some(&dir.path().join("missing.pem"))).is_err());
    let not_pem = dir.path().join("empty.pem");
    std::fs::write(&not_pem, "").unwrap();
    assert!(HubClient::trusting(&url, "x", Some(&not_pem)).is_err());
}
