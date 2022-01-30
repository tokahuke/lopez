use std::rc::Rc;
use tokio_postgres::{Client, Statement};

use lib_lopez::backend::{async_trait, PageRanker};

const LINKAGE: &str = include_str!("sql/linkage.sql");
const ENSURE_PAGE_RANK: &str = include_str!("sql/ensure_page_rank.sql");

pub struct PostgresPageRanker {
    client: Rc<Client>,
    wave_id: i32,
    linkage: Statement,
    ensure_page_rank: Statement,
}

impl PostgresPageRanker {
    pub(super) async fn init(
        client: Rc<Client>,
        wave_id: i32,
    ) -> Result<PostgresPageRanker, anyhow::Error> {
        // Prepare statements:
        let linkage = client.prepare(LINKAGE).await?;
        let ensure_page_rank = client.prepare(ENSURE_PAGE_RANK).await?;

        Ok(PostgresPageRanker {
            client,
            wave_id,
            linkage,
            ensure_page_rank,
        })
    }
}

#[async_trait(?Send)]
impl PageRanker for PostgresPageRanker {
    type PageId = i64;

    async fn linkage(
        &mut self,
    ) -> Result<Box<dyn Iterator<Item = (Self::PageId, Self::PageId)>>, anyhow::Error> {
        // Create a stream of links:
        let edges = self
            .client
            .query(&self.linkage, &[&self.wave_id])
            .await?
            .into_iter()
            .map(|row| {
                (
                    row.get::<_, i64>("from_page_id"),
                    row.get::<_, i64>("to_page_id"),
                )
            });

        Ok(Box::new(edges))
    }

    async fn push_page_ranks(
        &mut self,
        ranked: &[(Self::PageId, f64)],
    ) -> Result<(), anyhow::Error> {
        let (page_batch, rank_batch): (Vec<_>, Vec<_>) = ranked.iter().cloned().unzip();
        let params = params![&self.wave_id, page_batch, rank_batch];
        self.client.execute(&self.ensure_page_rank, params).await?;

        Ok(())
    }
}
