use std::rc::Rc;
use tokio_postgres::{Client, Statement};

use lib_lopez::backend::{async_trait, PageRanker};

const CANONICAL_LINKAGE: &str = include_str!("sql/canonical_linkage.sql");
const ENSURE_PAGE_RANK: &str = include_str!("sql/ensure_page_rank.sql");

pub struct PostgresPageRanker {
    client: Rc<Client>,
    wave_id: i32,
    canonical_linkage: Statement,
    ensure_page_rank: Statement,
}

impl PostgresPageRanker {
    pub(super) async fn init(
        client: Rc<Client>,
        wave_id: i32,
    ) -> Result<PostgresPageRanker, crate::Error> {
        // Prepare statements:
        let canonical_linkage = client.prepare(CANONICAL_LINKAGE).await?;
        let ensure_page_rank = client.prepare(ENSURE_PAGE_RANK).await?;

        Ok(PostgresPageRanker {
            client,
            wave_id,
            canonical_linkage,
            ensure_page_rank,
        })
    }
}

#[async_trait(?Send)]
impl PageRanker for PostgresPageRanker {
    type Error = crate::Error;
    type PageId = i64;

    async fn canonical_linkage(
        &self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, crate::Error> {
        // Create a stream of links:
        let edges = self
            .client
            .query(&self.canonical_linkage, &[&self.wave_id])
            .await?
            .into_iter()
            .map(|row| {
                (
                    row.get::<_, i64>("from_canonical_page_id"),
                    row.get::<_, i64>("to_canonical_page_id"),
                )
            });

        Ok(Box::new(edges))
    }

    async fn push_page_ranks(&self, ranked: &[(Self::PageId, f64)]) -> Result<(), crate::Error> {
        let (page_batch, rank_batch): (Vec<_>, Vec<_>) = ranked.iter().cloned().unzip();
        let params = params![&self.wave_id, page_batch, rank_batch];
        self.client.execute(&self.ensure_page_rank, params).await?;

        Ok(())
    }
}
