use include_dir::Dir;
use migrant_lib::migration::EmbeddedMigration;
use migrant_lib::{Config, Migratable, Migrator, Settings};
use std::rc::Rc;
use std::sync::Arc;
use structopt::StructOpt;
use tokio_postgres::error::SqlState;
use tokio_postgres::{connect as pg_connect, Client, NoTls};

#[macro_export]
macro_rules! params {
    ($($param:expr),*$(,)?) => {
        &[$(
            &$param as &(dyn tokio_postgres::types::ToSql + Sync),
        )*]
    }
}

#[derive(Debug, StructOpt)]
pub struct DbConfig {
    #[structopt(long, env = "DB_HOST")]
    host: String,
    #[structopt(long, default_value = "5432", env = "DB_PORT")]
    port: u16,
    #[structopt(long, env = "DB_USER")]
    user: String,
    #[structopt(long, env = "DB_DBNAME")]
    dbname: String,
    #[structopt(long, env = "DB_PASSWORD")]
    password: String,
}

impl DbConfig {
    pub async fn connect(&self) -> Result<Rc<Client>, crate::Error> {
        // Connect to database:
        let (client, connection) = pg_connect(
            &format!(
                "host={} port={} user={} dbname={} password={}",
                self.host, self.port, self.user, self.dbname, self.password,
            ),
            NoTls,
        )
        .await?;

        // Spawn connection to run:
        tokio::spawn(async move { 
            if let Err(err) = connection.await {
                log::error!("connection failed: {}", err);
            }
        });

        Ok(Rc::new(client))
    }

    /// Ensures that the database exists.
    pub async fn ensure_create_db(&self) -> Result<(), crate::Error> {
        // Connect to database:
        let (client, connection) = pg_connect(
            &format!(
                "host={} port={} user={} dbname={} password={}",
                self.host, self.port, self.user, "postgres", self.password,
            ),
            NoTls,
        )
        .await?;

        // Spawn connection to run:
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                log::error!("connection failed: {}", err);
            }
        });

        // Now, do the thing:
        let outcome = client
            .simple_query(&format!("create database \"{}\";", self.dbname))
            .await;

        // Duplicate databases are ok:
        if let Err(error) = outcome {
            let code = error.code().expect("no code returned on create database");
            if *code != SqlState::DUPLICATE_DATABASE {
                Err(error)?;
            }
        }

        Ok(())
    }

    fn embedded_migrations() -> Vec<Box<dyn Migratable>> {
        // Build embedded migrations:
        const MIGRATIONS: Dir = include_dir::include_dir!("migrations");

        let mut migration_names = MIGRATIONS
            .dirs()
            .iter()
            .map(|migration_dir| migration_dir.path().to_owned())
            .collect::<Vec<_>>();

        migration_names.sort_unstable();

        migration_names
            .into_iter()
            .map(|path| {
                EmbeddedMigration::with_tag(path.to_string_lossy().as_ref())
                    .up(String::from_utf8_lossy(
                        MIGRATIONS
                            .get_file(path.clone().join("up.sql"))
                            .expect("missing `up.sql`")
                            .contents(),
                    ))
                    .down(String::from_utf8_lossy(
                        MIGRATIONS
                            .get_file(path.clone().join("down.sql"))
                            .expect("missing `down.sql`")
                            .contents(),
                    ))
                    .boxed()
            })
            .collect()
    }

    /// Ensures all migrations are up-to-date.
    pub async fn sync_migrations(self: Arc<Self>) -> Result<(), crate::Error> {
        // Need to spawn blocking because a second runtime is inited by migrant
        // and Tokio is not happy with runtime within runtime.
        tokio::task::spawn_blocking(move || {
            log::info!("Ensuring migrations are up-to-date");

            // Settings for migrations:
            let settings = Settings::configure_postgres()
                .database_host(&self.host)
                .database_port(self.port)
                .database_name(&self.dbname)
                .database_user(&self.user)
                .database_password(&self.password)
                .build()?;

            // Configuration for migrations:
            let mut config = Config::with_settings(&settings);

            config.use_migrations(DbConfig::embedded_migrations())?; // set migrations up
            config.setup()?; // set migrant stuff up in db
            config = config.reload()?; // queries what has already been applied (funny name...)

            // Do migraty thingies:
            let mut migrator = Migrator::with_config(&config);
            migrator.all(true).swallow_completion(true).apply()?;

            log::info!("everything up-to-date");

            Ok(())
        })
        .await
        .expect("spawn error")
    }
}
