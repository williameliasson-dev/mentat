//! Exercises the vault rpcs over a real connection, against a real bucket:
//! accounts cannot reach each other's vaults, the header is compare-and-swap,
//! and the log numbers, pulls and prunes the way clients rely on.
//!
//! Needs the `S3_*` bucket, i.e. a `devenv up s3` versitygw. That matters
//! here: the conditional writes under test are the bucket's, not the server's.

mod common;

use std::collections::HashMap;

use common::{Server, with_key};
use proto::pb::vault::v1::vaults_client::VaultsClient;
use proto::pb::vault::v1::{
    DeleteSnapshotRequest, GetHeaderRequest, GetSnapshotRequest, ListVaultsRequest, PruneRequest,
    PullRequest, PullResponse, PushRequest, PutHeaderRequest, PutSnapshotRequest,
};
use tonic::Code;
use tonic::transport::Channel;
use uuid::Uuid;

async fn client(server: &Server) -> VaultsClient<Channel> {
    VaultsClient::connect(server.url.clone()).await.unwrap()
}

async fn push(
    vaults: &mut VaultsClient<Channel>,
    key: &str,
    vault: &str,
    device: &str,
    batch: &str,
) -> u64 {
    vaults
        .push(with_key(
            PushRequest {
                vault_id: vault.into(),
                device_id: device.into(),
                batch: batch.into(),
            },
            key,
        ))
        .await
        .expect("push")
        .into_inner()
        .seq
}

/// (device, seq, batch) for everything past `after`.
async fn pull(
    vaults: &mut VaultsClient<Channel>,
    key: &str,
    vault: &str,
    after: &[(&str, u64)],
) -> Vec<(String, u64, String)> {
    let mut stream = vaults
        .pull(with_key(
            PullRequest {
                vault_id: vault.into(),
                after: after
                    .iter()
                    .map(|&(d, s)| (d.to_string(), s))
                    .collect::<HashMap<_, _>>(),
            },
            key,
        ))
        .await
        .expect("pull")
        .into_inner();
    let mut batches = Vec::new();
    while let Some(PullResponse {
        device_id,
        seq,
        batch,
    }) = stream.message().await.expect("pull stream")
    {
        batches.push((device_id, seq, String::from_utf8(batch).unwrap()));
    }
    batches
}

#[tokio::test]
async fn accounts_cannot_reach_each_others_vaults() {
    let server = Server::start().await;
    let (_, alice) = server.create_account().await;
    let (_, bob) = server.create_account().await;
    let mut vaults = client(&server).await;
    let vault = Uuid::new_v4().to_string();
    let device = Uuid::new_v4().to_string();

    vaults
        .put_header(with_key(
            PutHeaderRequest {
                vault_id: vault.clone(),
                header: b"alice's".to_vec(),
                if_match: None,
            },
            &alice,
        ))
        .await
        .expect("create header");
    push(&mut vaults, &alice, &vault, &device, "alice's batch").await;

    // Alice sees her vault; Bob sees none.
    let listed = |key: &str| {
        let mut vaults = vaults.clone();
        let key = key.to_string();
        async move {
            vaults
                .list_vaults(with_key(ListVaultsRequest {}, &key))
                .await
                .unwrap()
                .into_inner()
                .vault_ids
        }
    };
    assert_eq!(listed(&alice).await, vec![vault.clone()]);
    assert!(listed(&bob).await.is_empty());

    // Bob naming Alice's vault id reaches only his own (empty) namespace.
    let status = vaults
        .get_header(with_key(
            GetHeaderRequest {
                vault_id: vault.clone(),
            },
            &bob,
        ))
        .await
        .expect_err("another account's header must not be readable");
    assert_eq!(status.code(), Code::NotFound);
    assert!(pull(&mut vaults, &bob, &vault, &[]).await.is_empty());

    // And no key at all gets nowhere.
    let status = vaults
        .list_vaults(ListVaultsRequest {})
        .await
        .expect_err("no key must be rejected");
    assert_eq!(status.code(), Code::Unauthenticated);

    server.cleanup().await;
}

#[tokio::test]
async fn header_is_compare_and_swap() {
    let server = Server::start().await;
    let (_, key) = server.create_account().await;
    let mut vaults = client(&server).await;
    let vault = Uuid::new_v4().to_string();

    let put = |header: &str, if_match: Option<String>| {
        with_key(
            PutHeaderRequest {
                vault_id: vault.clone(),
                header: header.into(),
                if_match,
            },
            &key,
        )
    };

    let first = vaults
        .put_header(put("v1", None))
        .await
        .expect("create")
        .into_inner()
        .etag;

    // Creating again must not clobber the existing header.
    let status = vaults
        .put_header(put("v1 again", None))
        .await
        .expect_err("second create");
    assert_eq!(status.code(), Code::Aborted);

    // Updating from the current version works and moves the etag on.
    let second = vaults
        .put_header(put("v2", Some(first.clone())))
        .await
        .expect("update from current etag")
        .into_inner()
        .etag;
    assert_ne!(first, second);

    // Updating from a stale version loses.
    let status = vaults
        .put_header(put("v3", Some(first)))
        .await
        .expect_err("stale etag");
    assert_eq!(status.code(), Code::Aborted);

    let current = vaults
        .get_header(with_key(
            GetHeaderRequest {
                vault_id: vault.clone(),
            },
            &key,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(current.header, b"v2");
    assert_eq!(current.etag, second);

    server.cleanup().await;
}

#[tokio::test]
async fn log_numbers_pulls_and_prunes() {
    let server = Server::start().await;
    let (_, key) = server.create_account().await;
    let mut vaults = client(&server).await;
    let vault = Uuid::new_v4().to_string();
    let laptop = Uuid::new_v4().to_string();
    let desktop = Uuid::new_v4().to_string();

    // Each device's log is numbered on its own, from 1.
    assert_eq!(push(&mut vaults, &key, &vault, &laptop, "l1").await, 1);
    assert_eq!(push(&mut vaults, &key, &vault, &laptop, "l2").await, 2);
    assert_eq!(push(&mut vaults, &key, &vault, &desktop, "d1").await, 1);
    assert_eq!(push(&mut vaults, &key, &vault, &laptop, "l3").await, 3);

    // A fresh device pulls everything; per device it arrives in seq order.
    let mut all = pull(&mut vaults, &key, &vault, &[]).await;
    all.sort();
    let mut expected = vec![
        (laptop.clone(), 1, "l1".to_string()),
        (laptop.clone(), 2, "l2".to_string()),
        (laptop.clone(), 3, "l3".to_string()),
        (desktop.clone(), 1, "d1".to_string()),
    ];
    expected.sort();
    assert_eq!(all, expected);

    // Cursors skip what was already seen.
    assert_eq!(
        pull(&mut vaults, &key, &vault, &[(&laptop, 2), (&desktop, 1)]).await,
        vec![(laptop.clone(), 3, "l3".to_string())]
    );

    // Pruning past the end keeps the newest batch, so numbering carries on
    // from 3 instead of restarting and hiding the next batch behind a cursor.
    vaults
        .prune(with_key(
            PruneRequest {
                vault_id: vault.clone(),
                device_id: laptop.clone(),
                up_to: 3,
            },
            &key,
        ))
        .await
        .expect("prune");
    assert_eq!(
        pull(&mut vaults, &key, &vault, &[(&desktop, 1)]).await,
        vec![(laptop.clone(), 3, "l3".to_string())]
    );
    assert_eq!(push(&mut vaults, &key, &vault, &laptop, "l4").await, 4);
    assert_eq!(
        pull(&mut vaults, &key, &vault, &[(&laptop, 3), (&desktop, 1)]).await,
        vec![(laptop.clone(), 4, "l4".to_string())]
    );

    server.cleanup().await;
}

#[tokio::test]
async fn concurrent_pushes_get_distinct_seqs() {
    let server = Server::start().await;
    let (_, key) = server.create_account().await;
    let vaults = client(&server).await;
    let vault = Uuid::new_v4().to_string();
    let device = Uuid::new_v4().to_string();

    let pushes = (0..8).map(|i| {
        let mut vaults = vaults.clone();
        let (key, vault, device) = (key.clone(), vault.clone(), device.clone());
        tokio::spawn(
            async move { push(&mut vaults, &key, &vault, &device, &format!("b{i}")).await },
        )
    });
    let mut seqs: Vec<u64> = futures::future::try_join_all(pushes).await.unwrap();
    seqs.sort();
    assert_eq!(seqs, (1..=8).collect::<Vec<_>>());

    server.cleanup().await;
}

#[tokio::test]
async fn snapshots_round_trip() {
    let server = Server::start().await;
    let (_, key) = server.create_account().await;
    let mut vaults = client(&server).await;
    let (vault, doc, snapshot) = (
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
        Uuid::new_v4().to_string(),
    );
    let get = || {
        with_key(
            GetSnapshotRequest {
                vault_id: vault.clone(),
                doc_id: doc.clone(),
                snapshot_id: snapshot.clone(),
            },
            &key,
        )
    };
    let delete = || {
        with_key(
            DeleteSnapshotRequest {
                vault_id: vault.clone(),
                doc_id: doc.clone(),
                snapshot_id: snapshot.clone(),
            },
            &key,
        )
    };

    vaults
        .put_snapshot(with_key(
            PutSnapshotRequest {
                vault_id: vault.clone(),
                doc_id: doc.clone(),
                snapshot_id: snapshot.clone(),
                snapshot: b"state".to_vec(),
            },
            &key,
        ))
        .await
        .expect("put snapshot");
    assert_eq!(
        vaults
            .get_snapshot(get())
            .await
            .unwrap()
            .into_inner()
            .snapshot,
        b"state"
    );

    vaults.delete_snapshot(delete()).await.expect("delete");
    assert_eq!(
        vaults.get_snapshot(get()).await.unwrap_err().code(),
        Code::NotFound
    );
    // Deleting again is fine: a retried compaction must be able to rerun.
    vaults
        .delete_snapshot(delete())
        .await
        .expect("delete again");

    // Ids become path segments, so anything but a UUID is refused outright.
    let status = vaults
        .get_snapshot(with_key(
            GetSnapshotRequest {
                vault_id: "../../accounts".into(),
                doc_id: doc.clone(),
                snapshot_id: snapshot.clone(),
            },
            &key,
        ))
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::InvalidArgument);

    server.cleanup().await;
}
