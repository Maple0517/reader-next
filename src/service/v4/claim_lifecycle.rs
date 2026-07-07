use crate::storage::db::v4::claim_repo::ClaimRepo;

pub(crate) async fn accept_claim(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    set_claim_status(claim_repo, claim_id, "accepted", counter).await
}

pub(crate) async fn reject_claim(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    set_claim_status(claim_repo, claim_id, "rejected", counter).await
}

pub(crate) async fn mark_uncertain_claim(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    set_claim_status(claim_repo, claim_id, "uncertain", counter).await
}

pub(crate) async fn quarantine_claim(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    set_claim_status(claim_repo, claim_id, "quarantined", counter).await
}

pub(crate) async fn redirect_claim(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    set_claim_status(claim_repo, claim_id, "redirected", counter).await
}

async fn set_claim_status(
    claim_repo: &ClaimRepo,
    claim_id: &str,
    status: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    claim_repo.update_claim_status(claim_id, status).await?;
    *counter += 1;
    Ok(())
}
