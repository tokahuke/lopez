use failure_derive::Fail;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Database(tokio_postgres::Error),
    #[fail(display = "migration error: {}", _0)]
    Migration(String), // !Sync
}

impl From<tokio_postgres::Error> for Error {
    fn from(this: tokio_postgres::Error) -> Error {
        Error::Database(this)
    }
}

impl From<migrant_lib::errors::Error> for Error {
    fn from(this: migrant_lib::errors::Error) -> Error {
        Error::Migration(format!("{}", this))
    }
}

impl From<crate::Error> for lib_lopez::Error {
    fn from(this: Error) -> lib_lopez::Error {
        lib_lopez::Error::Custom(format!("{}", this))
    }
}
