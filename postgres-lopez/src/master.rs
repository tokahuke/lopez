use std::rc::Rc;
use tokio_postgres::{Client, Statement};

use lib_lopez::backend::{async_trait, MasterBackend, Type, Url};
use lib_lopez::hash;

const ENSURE_WAVE: &str = include_str!("sql/ensure_wave.sql");
const ENSURE_STATUS: &str = include_str!("sql/ensure_status.sql");
const ENSURE_NAMES: &str = include_str!("sql/ensure_names.sql");
const CREATE_ANALYSES: &str = include_str!("sql/create_analyses.sql");
const RESET_QUEUE: &str = include_str!("sql/reset_queue.sql");
const FETCH: &str = include_str!("sql/fetch.sql");
const COUNT_CRAWLED: &str = include_str!("sql/count_crawled.sql");
const EXISTS_TAKEN: &str = include_str!("sql/exists_taken.sql");

pub struct PostgresMasterBackend {
    client: Rc<Client>,
    wave_id: i32,
    ensure_status: Statement,
    ensure_names: Statement,
    create_analyses: Statement,
    reset_queue: Statement,
    exists_taken: Statement,
    fetch: Statement,
    count_crawled: Statement,
}

impl PostgresMasterBackend {
    pub async fn init(
        client: Rc<Client>,
        wave: &str,
    ) -> Result<PostgresMasterBackend, anyhow::Error> {
        // Prepare statements:
        let ensure_wave = client.prepare(ENSURE_WAVE).await?;
        let ensure_status = client.prepare(ENSURE_STATUS).await?;
        let ensure_names = client.prepare(ENSURE_NAMES).await?;
        let create_analyses = client.prepare(CREATE_ANALYSES).await?;
        let reset_queue = client.prepare(RESET_QUEUE).await?;
        let exists_taken = client.prepare(EXISTS_TAKEN).await?;
        let fetch = client.prepare(FETCH).await?;
        let count_crawled = client.prepare(COUNT_CRAWLED).await?;

        // Find out current wave:
        let wave_id = client
            .query(&ensure_wave, &[&wave])
            .await?
            .into_iter()
            .map(|row| row.get::<_, i32>("wave_id"))
            .next()
            .expect("must always return something");

        Ok(PostgresMasterBackend {
            client,
            wave_id,
            ensure_status,
            ensure_names,
            create_analyses,
            reset_queue,
            exists_taken,
            fetch,
            count_crawled,
        })
    }
}

#[async_trait(?Send)]
impl MasterBackend for PostgresMasterBackend {
    fn wave_id(&mut self) -> i32 {
        self.wave_id
    }

    async fn ensure_seeded(&mut self, seeds: &[Url]) -> Result<(), anyhow::Error> {
        let wave_id = self.wave_id;
        let page_ids = seeds
            .iter()
            .map(|base_urls| hash(&base_urls.as_str()))
            .collect::<Vec<_>>();

        // Seeds are now a known page.
        let params = params![
            page_ids,
            seeds.iter().map(|seed| seed.as_str()).collect::<Vec<_>>()
        ];
        let _ensure_names = self.client.execute(&self.ensure_names, params).await?;

        // Seeds are marked as visited.
        let params = params![wave_id, page_ids, 0i16];
        let _ensure_status = self.client.execute(&self.ensure_status, params).await?;

        Ok(())
    }

    async fn create_analyses(
        &mut self,
        analysis_names: &[(String, Type)],
    ) -> Result<(), anyhow::Error> {
        let (analysis_names, result_types): (Vec<_>, Vec<_>) = analysis_names
            .iter()
            .map(|(name, typ)| (name.to_owned(), typ.to_string()))
            .unzip();
        let params = params![self.wave_id, analysis_names, result_types];
        self.client.execute(&self.create_analyses, params).await?;

        Ok(())
    }

    async fn count_crawled(&mut self) -> Result<usize, anyhow::Error> {
        let wave_id = self.wave_id;
        let crawled = self
            .client
            .query(&self.count_crawled, &[&wave_id])
            .await?
            .into_iter()
            .map(|row| row.get::<_, Option<i64>>("crawled").unwrap_or(0) as usize)
            .next()
            .unwrap_or(0);

        Ok(crawled)
    }

    async fn reset_queue(&mut self) -> Result<(), anyhow::Error> {
        let wave_id = self.wave_id;
        self.client.execute(&self.reset_queue, &[&wave_id]).await?;

        Ok(())
    }

    async fn exists_taken(&mut self) -> Result<bool, anyhow::Error> {
        let wave_id = self.wave_id;
        let exists_taken = self
            .client
            .query(&self.exists_taken, &[&wave_id])
            .await?
            .into_iter()
            .map(|row| row.get::<_, bool>("exists_taken"))
            .next()
            .expect("always retrns a row");

        Ok(exists_taken)
    }

    async fn fetch(
        &mut self,
        batch_size: i64,
        max_depth: i16,
    ) -> Result<Vec<(Url, u16)>, anyhow::Error> {
        let batch = self
            .client
            .query(&self.fetch, &[&self.wave_id, &batch_size, &max_depth])
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.get::<_, String>("page_url").parse::<Url>()?,
                    row.get::<_, i16>("depth") as u16,
                )) as Result<_, anyhow::Error>
            })
            .filter_map(|url_and_depth| url_and_depth.ok())
            .collect::<Vec<_>>();

        Ok(batch)
    }
}

// #[tokio::test]
// async fn test_init_master() {
//     let connection = crate::db::connect().await.unwrap();
//     PostgresMasterBackend::init(connection, "foo").await.unwrap();
// }
