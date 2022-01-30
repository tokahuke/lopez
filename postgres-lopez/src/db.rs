use include_dir::Dir;
use migrant_lib::migration::EmbeddedMigration;
use migrant_lib::{Config, Migratable, Migrator, Settings};
use serde_derive::{Deserialize, Serialize};
use std::env;
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

#[derive(Debug, StructOpt, Serialize, Deserialize)]
pub struct DbConfig {
    /// The host name of the PostgreSQL server.
    #[structopt(long, env = "DB_HOST", default_value = "localhost")]
    host: String,
    /// The port number of the PostgreSQL server.
    #[structopt(long, default_value = "5432", env = "DB_PORT")]
    port: u16,
    /// The user that the application will use to log into the server.
    #[structopt(long, env = "DB_USER")]
    user: Option<String>,
    /// The name of the database in the server in which information will be
    /// stored.
    #[structopt(long, env = "DB_DBNAME")]
    dbname: Option<String>,
    /// The password of the user that the application will use to log into the
    /// server.
    #[structopt(long, env = "DB_PASSWORD", default_value = "")]
    password: String,
}

impl DbConfig {
    fn user(&self) -> String {
        self.user
            .clone()
            .or_else(|| env::var("USER").ok())
            .unwrap_or_default()
    }

    fn dbname(&self) -> String {
        self.dbname
            .clone()
            .or_else(|| env::var("USER").ok())
            .unwrap_or_default()
    }

    pub async fn connect(&self) -> Result<Rc<Client>, anyhow::Error> {
        // Connect to database:
        let (client, connection) = pg_connect(
            &format!(
                "host={} port={} user={} dbname={} password={}",
                self.host,
                self.port,
                self.user(),
                self.dbname(),
                self.password,
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
    pub async fn ensure_create_db(&self) -> Result<(), anyhow::Error> {
        // Connect to database:
        let (client, connection) = pg_connect(
            &format!(
                "host={} port={} user={} dbname={} password={}",
                self.host,
                self.port,
                self.user(),
                "postgres",
                self.password,
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
            .simple_query(&format!("create database \"{}\";", self.dbname()))
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
        const MIGRATIONS: Dir = include_dir::include_dir!("postgres-lopez/migrations");

        let mut migration_names = MIGRATIONS
            .dirs()
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
    pub async fn sync_migrations(self: Arc<Self>) -> Result<(), migrant_lib::Error> {
        // Need to spawn blocking because a second runtime is inited by migrant
        // and Tokio is not happy with runtime within runtime.
        tokio::task::spawn_blocking(move || {
            log::info!("Ensuring migrations are up-to-date");

            // Settings for migrations:
            let settings = Settings::configure_postgres()
                .database_host(&self.host)
                .database_port(self.port)
                .database_name(&self.dbname())
                .database_user(&self.user())
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
