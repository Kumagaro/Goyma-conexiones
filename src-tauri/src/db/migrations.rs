use rusqlite::{Connection, Transaction};

use crate::errors::AppError;

/// Versión del esquema que crea `schema::create_schema` (specs 1 a 3). Todo
/// cambio posterior llega como migración numerada (`Constitution.md` §4,
/// v1.3.0; `plan-04` §2.1).
pub const BASE_VERSION: i64 = 1;

pub type Migration = fn(&Transaction) -> rusqlite::Result<()>;

/// Migraciones en orden estricto: cada una sube la versión en uno.
pub const MIGRATIONS: &[(i64, Migration)] = &[];

pub fn latest_version() -> i64 {
    latest_of(MIGRATIONS)
}

fn latest_of(migrations: &[(i64, Migration)]) -> i64 {
    migrations
        .last()
        .map_or(BASE_VERSION, |(version, _)| *version)
}

pub fn schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Lleva la base a la última versión. Se llama después de `init_schema`, que
/// deja creado (o intacto) el esquema base.
pub fn run_migrations(connection: &mut Connection) -> Result<i64, AppError> {
    run_migrations_with(connection, MIGRATIONS)
}

fn run_migrations_with(
    connection: &mut Connection,
    migrations: &[(i64, Migration)],
) -> Result<i64, AppError> {
    let mut current = schema_version(connection)
        .map_err(|error| migration_failed(&format!("failed to read schema version: {error}")))?;
    // Una base sin versión es la de las specs 1 a 3 (o una recién creada por
    // init_schema): se marca como versión base sin tocar sus datos.
    if current == 0 {
        connection
            .pragma_update(None, "user_version", BASE_VERSION)
            .map_err(|error| migration_failed(&format!("failed to set base version: {error}")))?;
        current = BASE_VERSION;
    }
    let latest = latest_of(migrations);
    if current > latest {
        return Err(AppError::new(
            "schemaTooNew",
            "Esta base de datos es de una versión más nueva de la aplicación",
            &format!("schema version {current} is newer than supported {latest}"),
        ));
    }
    let start = current;
    for (version, migrate) in migrations.iter().filter(|(version, _)| *version > start) {
        // Cada migración y su número de versión van en la misma transacción:
        // si falla, la base queda en la versión anterior.
        let transaction = connection.transaction().map_err(|error| {
            migration_failed(&format!("failed to open migration {version}: {error}"))
        })?;
        migrate(&transaction)
            .and_then(|()| transaction.pragma_update(None, "user_version", version))
            .and_then(|()| transaction.commit())
            .map_err(|error| migration_failed(&format!("migration {version} failed: {error}")))?;
        current = *version;
    }
    Ok(current)
}

fn migration_failed(cause: &str) -> AppError {
    AppError::new(
        "migrationFailed",
        "No se pudo actualizar la base de datos. La aplicación no puede continuar",
        cause,
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, Transaction};

    use super::{
        latest_version, run_migrations, run_migrations_with, schema_version, Migration,
        BASE_VERSION,
    };
    use crate::db::schema::create_schema;

    fn table_exists(connection: &Connection, name: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [name],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn add_first(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationFirst (id INTEGER PRIMARY KEY);")
    }

    fn add_second(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationSecond (id INTEGER PRIMARY KEY);")
    }

    fn fails_halfway(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationBroken (id INTEGER PRIMARY KEY);")?;
        tx.execute_batch("INSERT INTO tableThatDoesNotExist VALUES (1);")
    }

    #[test]
    fn a_database_from_specs_1_to_3_is_marked_as_the_base_version_without_changes() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        connection
            .execute("INSERT INTO categories (name, nameNormalized, createdAt, updatedAt) VALUES ('Cables', 'cables', '2026-10-28T12:00:00Z', '2026-10-28T12:00:00Z')", [])
            .unwrap();
        assert_eq!(schema_version(&connection).unwrap(), 0);

        let version = run_migrations(&mut connection).unwrap();
        assert_eq!(version, latest_version());
        assert!(version >= BASE_VERSION);
        assert_eq!(schema_version(&connection).unwrap(), version);
        let categories: i64 = connection
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(categories, 1);

        // Ejecutarlo otra vez no cambia nada.
        assert_eq!(run_migrations(&mut connection).unwrap(), version);
    }

    #[test]
    fn pending_migrations_apply_in_order_and_only_once() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        let migrations: &[(i64, Migration)] = &[(2, add_first), (3, add_second)];

        assert_eq!(run_migrations_with(&mut connection, migrations).unwrap(), 3);
        assert!(table_exists(&connection, "migrationFirst"));
        assert!(table_exists(&connection, "migrationSecond"));
        // Ya aplicadas: no se repiten (CREATE TABLE fallaría si lo hicieran).
        assert_eq!(run_migrations_with(&mut connection, migrations).unwrap(), 3);
    }

    #[test]
    fn a_failing_migration_leaves_the_previous_version_and_reports_migration_failed() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        let migrations: &[(i64, Migration)] = &[(2, add_first), (3, fails_halfway)];

        let error = run_migrations_with(&mut connection, migrations).unwrap_err();
        assert_eq!(error.code, "migrationFailed");
        assert_eq!(
            error.message,
            "No se pudo actualizar la base de datos. La aplicación no puede continuar"
        );
        assert_eq!(schema_version(&connection).unwrap(), 2);
        assert!(table_exists(&connection, "migrationFirst"));
        // Lo que la migración fallida alcanzó a crear se revierte con ella.
        assert!(!table_exists(&connection, "migrationBroken"));
    }

    #[test]
    fn a_newer_database_is_rejected_with_schema_too_new() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        connection
            .pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();

        let error = run_migrations(&mut connection).unwrap_err();
        assert_eq!(error.code, "schemaTooNew");
        assert_eq!(
            error.message,
            "Esta base de datos es de una versión más nueva de la aplicación"
        );
        assert_eq!(schema_version(&connection).unwrap(), latest_version() + 1);
    }
}
