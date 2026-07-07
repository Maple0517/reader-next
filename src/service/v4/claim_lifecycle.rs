use crate::storage::db::v4::claim_repo::ClaimRepo;
use crate::storage::db::v4::quality_repo::{NewQuarantinedClaim, QualityRepo};
use sqlx::SqlitePool;

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
    pool: &SqlitePool,
    claim_id: &str,
    reason_text: &str,
    counter: &mut usize,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let claim_book: Option<(String,)> = sqlx::query_as("SELECT book_id FROM claims WHERE id = ?")
        .bind(claim_id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((book_id,)) = claim_book else {
        anyhow::bail!("claim not found for quarantine workflow");
    };

    ClaimRepo::update_claim_status_with_conn(&mut tx, claim_id, "quarantined").await?;
    QualityRepo::upsert_quarantined_claim_with_conn(
        &mut tx,
        NewQuarantinedClaim {
            id: None,
            book_id: &book_id,
            claim_id,
            reason_code: "domain_decision_quarantine",
            reason_text: Some(reason_text),
            suggested_action: "needs_manual_review",
            priority: 50,
        },
    )
    .await?;
    tx.commit().await?;
    *counter += 1;
    Ok(())
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
