use std::collections::HashMap;
use std::pin::Pin;

use futures::{Stream, StreamExt, stream};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::auth::authenticated_account;
use crate::service::VaultService;
use proto::pb::vault::v1::vaults_server::Vaults;
use proto::pb::vault::v1::{
    DeleteSnapshotRequest, DeleteSnapshotResponse, GetHeaderRequest, GetHeaderResponse,
    GetSnapshotRequest, GetSnapshotResponse, ListVaultsRequest, ListVaultsResponse, PruneRequest,
    PruneResponse, PullRequest, PullResponse, PushRequest, PushResponse, PutHeaderRequest,
    PutHeaderResponse, PutSnapshotRequest, PutSnapshotResponse,
};

/// The vault rpcs. Mounted behind [`AuthLayer`](crate::grpc::AuthLayer); each
/// one acts on the authenticated account, never on an account named in the
/// request, so there is no authorization check to forget.
pub struct VaultsRpc {
    vaults: VaultService,
}

impl VaultsRpc {
    pub fn new(vaults: VaultService) -> Self {
        Self { vaults }
    }
}

type PullStream = Pin<Box<dyn Stream<Item = Result<PullResponse, Status>> + Send>>;

#[tonic::async_trait]
impl Vaults for VaultsRpc {
    async fn list_vaults(
        &self,
        request: Request<ListVaultsRequest>,
    ) -> Result<Response<ListVaultsResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let vault_ids = self.vaults.list_vaults(account).await?;
        Ok(Response::new(ListVaultsResponse {
            vault_ids: vault_ids.iter().map(Uuid::to_string).collect(),
        }))
    }

    async fn get_header(
        &self,
        request: Request<GetHeaderRequest>,
    ) -> Result<Response<GetHeaderResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let vault = parse_id(&request.get_ref().vault_id, "vault_id")?;
        let (header, etag) = self.vaults.get_header(account, vault).await?;
        Ok(Response::new(GetHeaderResponse {
            header: header.into(),
            etag,
        }))
    }

    async fn put_header(
        &self,
        request: Request<PutHeaderRequest>,
    ) -> Result<Response<PutHeaderResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.into_inner();
        let vault = parse_id(&request.vault_id, "vault_id")?;
        let etag = self
            .vaults
            .put_header(account, vault, request.header.into(), request.if_match)
            .await?;
        Ok(Response::new(PutHeaderResponse { etag }))
    }

    async fn push(&self, request: Request<PushRequest>) -> Result<Response<PushResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.into_inner();
        let vault = parse_id(&request.vault_id, "vault_id")?;
        let device = parse_id(&request.device_id, "device_id")?;
        let seq = self
            .vaults
            .push(account, vault, device, request.batch.into())
            .await?;
        Ok(Response::new(PushResponse { seq }))
    }

    type PullStream = PullStream;

    async fn pull(
        &self,
        request: Request<PullRequest>,
    ) -> Result<Response<Self::PullStream>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.into_inner();
        let vault = parse_id(&request.vault_id, "vault_id")?;
        let after = request
            .after
            .iter()
            .map(|(device, &seq)| Ok((parse_id(device, "after")?, seq)))
            .collect::<Result<HashMap<_, _>, Status>>()?;

        // List up front, then read each batch only as the client takes it, so
        // a long log is never held in memory all at once.
        let entries = self.vaults.log_entries(account, vault, &after).await?;
        let vaults = self.vaults.clone();
        let batches = stream::iter(entries).then(move |entry| {
            let vaults = vaults.clone();
            async move {
                let batch = vaults.read_log(account, vault, entry).await?;
                Ok(PullResponse {
                    device_id: entry.device.to_string(),
                    seq: entry.seq,
                    batch: batch.into(),
                })
            }
        });
        Ok(Response::new(Box::pin(batches)))
    }

    async fn prune(
        &self,
        request: Request<PruneRequest>,
    ) -> Result<Response<PruneResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.into_inner();
        let vault = parse_id(&request.vault_id, "vault_id")?;
        let device = parse_id(&request.device_id, "device_id")?;
        self.vaults
            .prune(account, vault, device, request.up_to)
            .await?;
        Ok(Response::new(PruneResponse {}))
    }

    async fn put_snapshot(
        &self,
        request: Request<PutSnapshotRequest>,
    ) -> Result<Response<PutSnapshotResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.into_inner();
        let (vault, doc, snapshot) =
            snapshot_ids(&request.vault_id, &request.doc_id, &request.snapshot_id)?;
        self.vaults
            .put_snapshot(account, vault, doc, snapshot, request.snapshot.into())
            .await?;
        Ok(Response::new(PutSnapshotResponse {}))
    }

    async fn get_snapshot(
        &self,
        request: Request<GetSnapshotRequest>,
    ) -> Result<Response<GetSnapshotResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.get_ref();
        let (vault, doc, snapshot) =
            snapshot_ids(&request.vault_id, &request.doc_id, &request.snapshot_id)?;
        let bytes = self
            .vaults
            .get_snapshot(account, vault, doc, snapshot)
            .await?;
        Ok(Response::new(GetSnapshotResponse {
            snapshot: bytes.into(),
        }))
    }

    async fn delete_snapshot(
        &self,
        request: Request<DeleteSnapshotRequest>,
    ) -> Result<Response<DeleteSnapshotResponse>, Status> {
        let account = authenticated_account(&request)?.id;
        let request = request.get_ref();
        let (vault, doc, snapshot) =
            snapshot_ids(&request.vault_id, &request.doc_id, &request.snapshot_id)?;
        self.vaults
            .delete_snapshot(account, vault, doc, snapshot)
            .await?;
        Ok(Response::new(DeleteSnapshotResponse {}))
    }
}

/// Ids become object path segments, so anything but a UUID is refused here,
/// before it can get near a path.
fn parse_id(id: &str, field: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(id).map_err(|_| Status::invalid_argument(format!("invalid {field}")))
}

fn snapshot_ids(vault: &str, doc: &str, snapshot: &str) -> Result<(Uuid, Uuid, Uuid), Status> {
    Ok((
        parse_id(vault, "vault_id")?,
        parse_id(doc, "doc_id")?,
        parse_id(snapshot, "snapshot_id")?,
    ))
}
