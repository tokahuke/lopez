use std::rc::Rc;
use tokio_postgres::{Client, Statement};

use lib_lopez::backend::{async_trait, Reason, StatusCode, Url, Value, WorkerBackend};
use lib_lopez::hash;

const ENSURE_LINKS: &str = include_str!("sql/ensure_links.sql");
const ENSURE_ANALYZED: &str = include_str!("sql/ensure_analyzed.sql");
const ENSURE_CLOSED: &str = include_str!("sql/ensure_closed.sql");
const ENSURE_ERROR: &str = include_str!("sql/ensure_error.sql");
const ENSURE_STATUS: &str = include_str!("sql/ensure_status.sql");
const ENSURE_NAMES: &str = include_str!("sql/ensure_names.sql");

pub struct PostgresWorkerBackend {
    client: Rc<Client>,
    wave_id: i32,
    ensure_links: Statement,
    ensure_analyzed: Statement,
    ensure_closed: Statement,
    ensure_error: Statement,
    ensure_status: Statement,
    ensure_names: Statement,
}

impl PostgresWorkerBackend {
    pub(super) async fn init(
        client: Rc<Client>,
        wave_id: i32,
    ) -> Result<PostgresWorkerBackend, anyhow::Error> {
        // Prepare statements:
        let ensure_links = client.prepare(ENSURE_LINKS).await?;
        let ensure_analyzed = client.prepare(ENSURE_ANALYZED).await?;
        let ensure_closed = client.prepare(ENSURE_CLOSED).await?;
        let ensure_error = client.prepare(ENSURE_ERROR).await?;
        let ensure_status = client.prepare(ENSURE_STATUS).await?;
        let ensure_names = client.prepare(ENSURE_NAMES).await?;

        Ok(PostgresWorkerBackend {
            client,
            wave_id,
            ensure_links,
            ensure_analyzed,
            ensure_closed,
            ensure_error,
            ensure_status,
            ensure_names,
        })
    }
}

#[async_trait(?Send)]
impl WorkerBackend for PostgresWorkerBackend {
    async fn ensure_analyzed(
        &self,
        url: &Url,
        analyses: Vec<(String, Value)>,
    ) -> Result<(), anyhow::Error> {
        let (analysis_names, results): (Vec<_>, Vec<_>) = analyses
            .into_iter()
            .map(|(name, result)| (name, tokio_postgres::types::Json(result)))
            .unzip();
        let params = params![self.wave_id, hash(&url.as_str()), analysis_names, results];
        self.client.execute(&self.ensure_analyzed, params).await?;

        Ok(())
    }

    async fn ensure_explored(
        &self,
        from_url: &Url,
        status_code: StatusCode,
        link_depth: u16,
        links: Vec<(Reason, Url)>,
    ) -> Result<(), anyhow::Error> {
        let wave_id = self.wave_id;
        let from_page_id = hash(&from_url.as_str());
        let (reasons, to_urls): (Vec<_>, Vec<_>) = links
            .into_iter()
            .map(|(reason, url)| (reason, url.to_string()))
            .unzip();
        let to_page_ids = to_urls
            .iter()
            .map(|to_url| hash(to_url))
            .collect::<Vec<_>>();
        let reasons_str = reasons.iter().map(Reason::to_string).collect::<Vec<_>>();

        let params = params![wave_id, from_page_id, to_page_ids, reasons_str];
        let _ensure_links = self.client.execute(&self.ensure_links, params).await?;
        drop(reasons_str);

        let params = params![to_page_ids, to_urls];
        let _ensure_names = self.client.execute(&self.ensure_names, params).await?;
        drop(to_urls);

        let params = params![wave_id, to_page_ids, link_depth as i16];
        let _ensure_status = self.client.execute(&self.ensure_status, params).await?;
        drop(to_page_ids);

        let params = params![wave_id, from_page_id, status_code.as_u16() as i32];
        let _ensure_closed = self.client.execute(&self.ensure_closed, params).await?;

        Ok(())
    }

    async fn ensure_error(&self, url: &Url) -> Result<(), anyhow::Error> {
        let wave_id = self.wave_id;
        let page_id = hash(&url.as_str());

        self.client
            .execute(&self.ensure_error, &[&wave_id, &page_id])
            .await?;

        Ok(())
    }
}
