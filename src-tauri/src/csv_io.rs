//! Exportar e importar CSV en el formato de Excel en español (CSV-01,
//! CSV-02): `;`, coma decimal, fechas `dd/mm/aaaa` y UTF-8 con BOM.
//!
//! Cada fila importada usa el mismo `create_*_inner` que el formulario, que
//! abre su propia transacción: una fila que falla se revierte sola y las
//! válidas quedan guardadas. SQLite no admite un `BEGIN` dentro de un
//! `SAVEPOINT`, así que no hay transacción exterior (desviación de D-14).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::activity::{record_activity, Entry};
use crate::commands::auth::{require_session, Author};
use crate::commands::categories::{create_category_inner, list_categories_inner};
use crate::commands::clients::create_client_inner;
use crate::commands::invoices::list_invoices_inner;
use crate::commands::products::{create_product_inner, list_products_inner};
use crate::commands::suppliers::create_supplier_inner;
use crate::commands::units::{create_unit_inner, list_units_inner};
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::client::ContactInput;
use crate::models::csv::{CsvFilters, ImportSummary, SkippedRow};
use crate::models::product::CreateProductInput;
use crate::models::supplier::SupplierInput;
use crate::text::{normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::today_local;

const MAX_ROWS: usize = 5_000;
const SEPARATOR: u8 = b';';
const BOM: &[u8] = b"\xEF\xBB\xBF";

const PRODUCT_HEADERS: &[&str] = &[
    "Nombre",
    "Descripción",
    "Categoría",
    "Unidad",
    "Costo",
    "% de Ganancia",
    "Cantidad",
    "Umbral de Pocas Unidades",
    "Caducidad",
    "Precio de Venta",
    "Precio Calculado",
    "Valor Total",
];
const CATEGORY_HEADERS: &[&str] = &["Nombre", "Descripción", "Cantidad de Productos", "Ingresos"];
const UNIT_HEADERS: &[&str] = &["Nombre", "Abreviatura", "Cantidad de Productos"];
const CLIENT_HEADERS: &[&str] = &[
    "Nombre",
    "Apellidos",
    "Carnet de Identidad",
    "Teléfono",
    "Empresa",
    "Cargo",
    "Correo",
    "Dirección",
    "Información de Factura",
];
const SUPPLIER_HEADERS: &[&str] = &[
    "Nombre",
    "Apellidos",
    "Carnet de Identidad",
    "Teléfono",
    "Empresa",
    "Cargo",
    "Correo",
    "Dirección",
];
const INVOICE_HEADERS: &[&str] = &[
    "Número",
    "Tipo",
    "Contacto",
    "Fecha",
    "Total",
    "Estado",
    "Productos",
    "Responsable",
];

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

fn export_failed(cause: &str) -> AppError {
    // La causa no incluye la ruta: puede contener el nombre del usuario.
    AppError::new(
        "exportFailed",
        "No se pudo guardar el archivo",
        &format!("csv export failed: {cause}"),
    )
}

/// Un texto que empieza por `=`, `+`, `-` o `@` no se ejecuta como fórmula.
fn text(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

fn optional_text(value: Option<&str>) -> String {
    value.map(text).unwrap_or_default()
}

fn money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let size = cents.abs();
    format!("{sign}{},{:02}", size / 100, size % 100)
}

fn date(iso: &str) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map_or_else(|_| iso.to_owned(), |day| day.format("%d/%m/%Y").to_string())
}

fn invoice_type_label(code: &str) -> &'static str {
    match code {
        "E" => "Entrada",
        "V" => "Venta",
        "S" => "Servicio",
        "GG" => "Garantía",
        "GI" => "Insumos",
        _ => "Pérdida",
    }
}

fn invoice_status_label(code: &str, status: Option<&str>) -> &'static str {
    match (code, status) {
        (_, None) => "",
        (_, Some("cancelled")) => "Cancelada",
        ("E", Some("pending")) => "Por Pagar",
        ("E", _) => "Pagada",
        ("V" | "S", Some("pending")) => "Por Cobrar",
        ("V" | "S", _) => "Cobrada",
        (_, Some("pending")) => "Pendiente",
        _ => "Completada",
    }
}

type Table = (&'static [&'static str], Vec<Vec<String>>);

fn product_rows(
    connection: &mut Connection,
    now: DateTime<Utc>,
    filters: CsvFilters,
) -> Result<Table, AppError> {
    let items = list_products_inner(
        connection,
        now,
        today_local(now),
        filters.products.unwrap_or_default(),
    )?;
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        let description: Option<String> = connection
            .query_row(
                "SELECT description FROM products WHERE id = ?1",
                [item.id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, "exportCsv", &[]))?;
        rows.push(vec![
            text(&item.name),
            optional_text(description.as_deref()),
            optional_text(item.category_name.as_deref()),
            optional_text(item.unit_name.as_deref()),
            money(item.cost_cents),
            item.profit_percent,
            item.quantity,
            item.low_stock_threshold.unwrap_or_default(),
            item.expires_on.as_deref().map(date).unwrap_or_default(),
            money(item.selling_price_cents),
            money(item.calculated_price_cents),
            money(item.valor_total_cents),
        ]);
    }
    Ok((PRODUCT_HEADERS, rows))
}

fn contact_rows(
    connection: &mut Connection,
    now: DateTime<Utc>,
    table: &str,
    search: Option<&str>,
) -> Result<Vec<Vec<String>>, AppError> {
    require_session(connection, now)?;
    let invoice_info = if table == "clients" {
        "invoiceInfo"
    } else {
        "NULL"
    };
    let read = || -> rusqlite::Result<Vec<[Option<String>; 9]>> {
        connection
            .prepare(&format!(
                "SELECT firstName, lastName, nationalId, phone, company, position, email, address,
                        {invoice_info} FROM {table}"
            ))?
            .query_map([], |row| {
                Ok([
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ])
            })?
            .collect()
    };
    let mut contacts = read().map_err(|error| map_db_error(error, "exportCsv", &[]))?;
    let search = search.map(normalize_text).filter(|query| !query.is_empty());
    if let Some(query) = search {
        contacts.retain(|contact| {
            contact
                .iter()
                .take(5)
                .flatten()
                .any(|field| normalize_text(field).contains(&query))
        });
    }
    sort_spanish(&mut contacts, |contact| {
        let name = contact[0].clone().unwrap_or_default();
        spanish_sort_key(&format!(
            "{name} {}",
            contact[1].clone().unwrap_or_default()
        ))
    });
    Ok(contacts
        .iter()
        .map(|contact| {
            contact
                .iter()
                .map(|field| optional_text(field.as_deref()))
                .collect()
        })
        .collect())
}

fn table(
    connection: &mut Connection,
    now: DateTime<Utc>,
    kind: &str,
    filters: CsvFilters,
) -> Result<Table, AppError> {
    let search = filters.search.clone();
    match kind {
        "products" => product_rows(connection, now, filters),
        "categories" => {
            let items = list_categories_inner(connection, now, search.as_deref())?.items;
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        text(&item.name),
                        optional_text(item.description.as_deref()),
                        item.product_count.to_string(),
                        money(item.ingresos_cents),
                    ]
                })
                .collect();
            Ok((CATEGORY_HEADERS, rows))
        }
        "units" => {
            let items = list_units_inner(connection, now, search.as_deref())?.items;
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        text(&item.name),
                        text(&item.abbreviation),
                        item.product_count.to_string(),
                    ]
                })
                .collect();
            Ok((UNIT_HEADERS, rows))
        }
        "clients" => Ok((
            CLIENT_HEADERS,
            contact_rows(connection, now, "clients", search.as_deref())?,
        )),
        "suppliers" => {
            let mut rows = contact_rows(connection, now, "suppliers", search.as_deref())?;
            rows.iter_mut().for_each(|row| row.truncate(8));
            Ok((SUPPLIER_HEADERS, rows))
        }
        "invoices" => {
            let items = list_invoices_inner(connection, now, filters.invoices.unwrap_or_default())?;
            let rows = items
                .iter()
                .map(|item| {
                    vec![
                        text(&item.number),
                        invoice_type_label(&item.invoice_type).to_owned(),
                        optional_text(item.contact_name.as_deref()),
                        date(&item.date),
                        money(item.total_cents),
                        invoice_status_label(&item.invoice_type, item.status.as_deref()).to_owned(),
                        item.product_count.to_string(),
                        text(&item.created_by_name),
                    ]
                })
                .collect();
            Ok((INVOICE_HEADERS, rows))
        }
        _ => Err(validation_failed(
            "Este listado no se puede exportar",
            "unknown csv export kind",
        )),
    }
}

/// Devuelve cuántas filas se escribieron.
pub fn export_csv_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    kind: &str,
    filters: CsvFilters,
    destination: &Path,
) -> Result<usize, AppError> {
    require_session(connection, now)?;
    let (headers, rows) = table(connection, now, kind, filters)?;
    let mut writer = csv::WriterBuilder::new()
        .delimiter(SEPARATOR)
        .from_writer(BOM.to_vec());
    writer
        .write_record(headers)
        .map_err(|error| export_failed(&error.to_string()))?;
    for row in &rows {
        writer
            .write_record(row)
            .map_err(|error| export_failed(&error.to_string()))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|error| export_failed(&error.to_string()))?;
    fs::write(destination, bytes).map_err(|error| export_failed(&error.kind().to_string()))?;
    Ok(rows.len())
}

struct Layout {
    known: &'static [&'static str],
    required: &'static [&'static str],
}

fn layout(kind: &str) -> Result<Layout, AppError> {
    let (known, required): (&[&str], &[&str]) = match kind {
        "products" => (
            PRODUCT_HEADERS,
            &[
                "Nombre",
                "Costo",
                "% de Ganancia",
                "Cantidad",
                "Umbral de Pocas Unidades",
            ],
        ),
        "categories" => (CATEGORY_HEADERS, &["Nombre"]),
        "units" => (UNIT_HEADERS, &["Nombre", "Abreviatura"]),
        // spec-06 CT-01: solo el nombre es obligatorio.
        "clients" => (CLIENT_HEADERS, &["Nombre"]),
        "suppliers" => (SUPPLIER_HEADERS, &["Nombre"]),
        _ => {
            return Err(validation_failed(
                "Este tipo de datos no se puede importar",
                "unknown csv import kind",
            ))
        }
    };
    Ok(Layout { known, required })
}

/// Una fila leída: valores por encabezado, ya sin el escape de fórmulas.
struct Row<'a> {
    columns: &'a HashMap<String, usize>,
    record: &'a csv::StringRecord,
}

impl Row<'_> {
    fn get(&self, header: &str) -> String {
        let value = self
            .columns
            .get(header)
            .and_then(|index| self.record.get(*index))
            .unwrap_or("")
            .trim();
        let unescaped = value
            .strip_prefix('\'')
            .filter(|rest| rest.starts_with(['=', '+', '-', '@']));
        unescaped.unwrap_or(value).to_owned()
    }

    fn optional(&self, header: &str) -> Option<String> {
        Some(self.get(header)).filter(|value| !value.is_empty())
    }

    /// Las cifras llegan con coma; el backend las lee con punto.
    fn decimal(&self, header: &str) -> String {
        self.get(header).replace(',', ".")
    }
}

fn parse_date(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(&value, "%d/%m/%Y")
        .map(|day| Some(day.format("%Y-%m-%d").to_string()))
        .map_err(|_| {
            validation_failed(
                "La fecha de caducidad debe tener el formato dd/mm/aaaa",
                "invalid csv date",
            )
        })
}

fn find_by_name(connection: &Connection, table: &str, name: &str) -> Result<Option<i64>, AppError> {
    connection
        .query_row(
            &format!("SELECT id FROM {table} WHERE nameNormalized = ?1"),
            [normalize_text(name)],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "importCsv", &[]))
}

fn remove_created(connection: &Connection, table: &str, id: Option<i64>) {
    if let Some(id) = id {
        // shortcut: si esto falla queda una categoría o unidad sin productos,
        // que el usuario puede borrar; no justifica rechazar la importación.
        let _ = connection.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id]);
    }
}

fn import_product(
    connection: &mut Connection,
    now: DateTime<Utc>,
    row: &Row,
) -> Result<(), AppError> {
    let expires_on = parse_date(row.optional("Caducidad"))?;
    let category = row.optional("Categoría");
    let unit = row.optional("Unidad");
    let existing_category = match &category {
        Some(name) => find_by_name(connection, "categories", name)?,
        None => None,
    };
    let existing_unit = match &unit {
        Some(name) => find_by_name(connection, "units", name)?,
        None => None,
    };
    let new_category = match (&category, existing_category) {
        (Some(name), None) => Some(create_category_inner(connection, now, name, None)?.id),
        _ => None,
    };
    let new_unit = match (&unit, existing_unit) {
        (Some(name), None) => {
            // El CSV no trae abreviatura: se toma del nombre, que el usuario
            // puede corregir después en Unidades.
            let abbreviation: String = name.chars().take(10).collect();
            match create_unit_inner(connection, now, name, &abbreviation) {
                Ok(created) => Some(created.id),
                Err(error) => {
                    remove_created(connection, "categories", new_category);
                    return Err(error);
                }
            }
        }
        _ => None,
    };
    let result = create_product_inner(
        connection,
        now,
        today_local(now),
        CreateProductInput {
            name: row.get("Nombre"),
            description: row.optional("Descripción"),
            category_id: existing_category.or(new_category),
            unit_id: existing_unit.or(new_unit),
            cost: row.decimal("Costo"),
            quantity: row.decimal("Cantidad"),
            low_stock_threshold: row.decimal("Umbral de Pocas Unidades"),
            profit_percent: row.decimal("% de Ganancia"),
            expires_on,
        },
    );
    if result.is_err() {
        // La fila omitida no deja rastro.
        remove_created(connection, "units", new_unit);
        remove_created(connection, "categories", new_category);
    }
    result.map(|_| ())
}

fn import_row(
    connection: &mut Connection,
    now: DateTime<Utc>,
    kind: &str,
    row: &Row,
) -> Result<(), AppError> {
    match kind {
        "products" => import_product(connection, now, row),
        "categories" => create_category_inner(
            connection,
            now,
            &row.get("Nombre"),
            row.optional("Descripción").as_deref(),
        )
        .map(|_| ()),
        "units" => create_unit_inner(connection, now, &row.get("Nombre"), &row.get("Abreviatura"))
            .map(|_| ()),
        "clients" => create_client_inner(
            connection,
            now,
            ContactInput {
                first_name: row.get("Nombre"),
                last_name: row.get("Apellidos"),
                national_id: row.get("Carnet de Identidad"),
                phone: row.get("Teléfono"),
                company: row.optional("Empresa"),
                position: row.optional("Cargo"),
                email: row.optional("Correo"),
                address: row.optional("Dirección"),
                invoice_info: row.optional("Información de Factura"),
            },
        )
        .map(|_| ()),
        _ => create_supplier_inner(
            connection,
            now,
            SupplierInput {
                first_name: row.get("Nombre"),
                last_name: row.get("Apellidos"),
                national_id: row.get("Carnet de Identidad"),
                phone: row.get("Teléfono"),
                company: row.optional("Empresa"),
                position: row.optional("Cargo"),
                email: row.optional("Correo"),
                address: row.optional("Dirección"),
            },
        )
        .map(|_| ()),
    }
}

/// Encabezados y filas del archivo. Lo que impide leerlo entero se rechaza
/// sin importar nada (§11, S-10).
fn read_file(
    source: &Path,
    layout: &Layout,
) -> Result<(HashMap<String, usize>, Vec<csv::StringRecord>), AppError> {
    let bytes = fs::read(source).map_err(|error| {
        validation_failed(
            "No se pudo leer el archivo",
            &format!("csv read failed: {}", error.kind()),
        )
    })?;
    let content = bytes.strip_prefix(BOM).unwrap_or(&bytes);
    let content = std::str::from_utf8(content).map_err(|_| {
        validation_failed(
            "El archivo debe estar guardado como CSV UTF-8",
            "csv is not utf-8",
        )
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(SEPARATOR)
        .flexible(true)
        .from_reader(content.as_bytes());
    let unreadable = |_| validation_failed("El archivo no es un CSV válido", "csv parse failed");
    let headers: Vec<String> = reader
        .headers()
        .map_err(unreadable)?
        .iter()
        .map(|header| header.trim().to_owned())
        .collect();
    if headers.iter().all(String::is_empty) {
        return Err(validation_failed("El archivo está vacío", "csv is empty"));
    }
    if let Some(unknown) = headers
        .iter()
        .find(|header| !layout.known.contains(&header.as_str()))
    {
        return Err(validation_failed(
            &format!("El archivo tiene una columna desconocida: «{unknown}»"),
            "csv has an unknown header",
        ));
    }
    if let Some(missing) = layout
        .required
        .iter()
        .find(|header| !headers.iter().any(|known| known == *header))
    {
        return Err(validation_failed(
            &format!("Al archivo le falta la columna «{missing}»"),
            "csv is missing a header",
        ));
    }
    let records = reader
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(unreadable)?;
    if records
        .iter()
        .all(|record| record.iter().all(|field| field.trim().is_empty()))
    {
        return Err(validation_failed(
            "El archivo está vacío",
            "csv has no rows",
        ));
    }
    if records.len() > MAX_ROWS {
        return Err(validation_failed(
            "El archivo no puede tener más de 5000 filas",
            "csv has too many rows",
        ));
    }
    let columns = headers
        .into_iter()
        .enumerate()
        .map(|(index, header)| (header, index))
        .collect();
    Ok((columns, records))
}

pub fn import_csv_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    kind: &str,
    source: &Path,
) -> Result<ImportSummary, AppError> {
    let actor = require_session(connection, now)?;
    let layout = layout(kind)?;
    let (columns, records) = read_file(source, &layout)?;

    let mut imported = 0;
    let mut skipped = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        let row = Row {
            columns: &columns,
            record,
        };
        match import_row(connection, now, kind, &row) {
            Ok(()) => imported += 1,
            Err(error) => skipped.push(SkippedRow {
                // La fila 1 del archivo son los encabezados.
                row: index + 2,
                reason: error.message,
            }),
        }
    }

    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    record_activity(
        connection,
        &Author::User(&actor),
        now,
        Entry {
            action: "import",
            entity_type: "csv",
            label: &file_name,
            details: Some(&format!(
                "{imported} importadas, {} omitidas",
                skipped.len()
            )),
        },
    )
    .map_err(|error| map_db_error(error, "importCsv", &[]))?;
    Ok(ImportSummary { imported, skipped })
}

fn log_failure<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(connection, "csv", operation, error, now, app_data_dir);
            }
        }
    }
}

#[tauri::command]
pub fn export_csv(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    kind: String,
    filters: CsvFilters,
    destination_path: String,
) -> Result<usize, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = export_csv_inner(
        &mut connection,
        now,
        &kind,
        filters,
        Path::new(&destination_path),
    );
    log_failure(&connection, "exportCsv", &result, now);
    result
}

#[tauri::command]
pub fn import_csv(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    kind: String,
    source_path: String,
) -> Result<ImportSummary, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = import_csv_inner(&mut connection, now, &kind, Path::new(&source_path));
    log_failure(&connection, "importCsv", &result, now);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::InvoiceFilters;
    use crate::models::product::ProductFilters;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_category_fixture, insert_unit_fixture, setup_test_db,
    };
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("goyma-csv-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn signed_in() -> (Connection, DateTime<Utc>) {
        let now = Utc::now();
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "csvuser",
            "contador",
            "+5350000050",
            "12345678950",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        (connection, now)
    }

    fn write_file(content: &str) -> PathBuf {
        let path = temp_dir().join("datos.csv");
        fs::write(&path, [BOM, content.as_bytes()].concat()).unwrap();
        path
    }

    fn product_input(name: &str) -> CreateProductInput {
        CreateProductInput {
            low_stock_threshold: "5".into(),
            name: name.to_owned(),
            description: Some("=SUMA(A1)".to_owned()),
            category_id: None,
            unit_id: None,
            cost: "10.50".to_owned(),
            quantity: "2.5".to_owned(),
            profit_percent: "50".to_owned(),
            expires_on: None,
        }
    }

    fn export(
        connection: &mut Connection,
        now: DateTime<Utc>,
        kind: &str,
        filters: CsvFilters,
    ) -> String {
        let path = temp_dir().join("salida.csv");
        export_csv_inner(connection, now, kind, filters, &path).unwrap();
        let bytes = fs::read(path).unwrap();
        assert!(bytes.starts_with(BOM), "{kind}: falta el BOM");
        String::from_utf8(bytes[BOM.len()..].to_vec()).unwrap()
    }

    fn count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn export_products_uses_excel_format_and_neutralizes_formulas() {
        let (mut connection, now) = signed_in();
        let today = today_local(now);
        create_product_inner(&mut connection, now, today, product_input("Cable")).unwrap();
        let mut dated = product_input("-Bomba");
        dated.expires_on = Some("2099-03-05".to_owned());
        create_product_inner(&mut connection, now, today, dated).unwrap();

        let content = export(&mut connection, now, "products", CsvFilters::default());
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines[0],
            "Nombre;Descripción;Categoría;Unidad;Costo;% de Ganancia;Cantidad;Umbral de Pocas Unidades;Caducidad;Precio de Venta;Precio Calculado;Valor Total"
        );
        assert_eq!(lines.len(), 3);
        let row = lines.iter().find(|line| line.starts_with("Cable")).unwrap();
        assert!(row.contains("'=SUMA(A1)"), "{row}");
        assert!(row.contains(";10,50;"), "{row}");
        assert!(row.contains(";2,5;"), "{row}");
        assert!(row.contains(";15,75;"), "{row}");
        let dated = lines.iter().find(|line| line.contains("Bomba")).unwrap();
        assert!(dated.starts_with("'-Bomba"), "{dated}");
        assert!(dated.contains(";05/03/2099;"), "{dated}");
    }

    #[test]
    fn export_respects_the_active_filters() {
        let (mut connection, now) = signed_in();
        let today = today_local(now);
        create_product_inner(&mut connection, now, today, product_input("Cable")).unwrap();
        create_product_inner(&mut connection, now, today, product_input("Bomba")).unwrap();
        insert_category_fixture(&connection, "Tuberías", None);
        insert_category_fixture(&connection, "Eléctricos", None);
        insert_unit_fixture(&connection, "Metro", "m");
        insert_unit_fixture(&connection, "Kilogramo", "kg");

        let products = export(
            &mut connection,
            now,
            "products",
            CsvFilters {
                products: Some(ProductFilters {
                    search: Some("cab".to_owned()),
                    ..ProductFilters::default()
                }),
                ..CsvFilters::default()
            },
        );
        assert!(products.contains("Cable") && !products.contains("Bomba"));

        for (kind, search, kept, dropped) in [
            ("categories", "tuberias", "Tuberías", "Eléctricos"),
            ("units", "metro", "Metro", "Kilogramo"),
        ] {
            let content = export(
                &mut connection,
                now,
                kind,
                CsvFilters {
                    search: Some(search.to_owned()),
                    ..CsvFilters::default()
                },
            );
            assert!(content.contains(kept), "{kind}");
            assert!(!content.contains(dropped), "{kind}");
        }
    }

    #[test]
    fn export_contacts_gives_every_field_and_respects_the_search() {
        let (mut connection, now) = signed_in();
        let contact = "Nombre;Apellidos;Carnet de Identidad;Teléfono;Empresa\n\
                       Ana;Pérez;90010112345;+5355555555;Goyma\n\
                       Luis;Gómez;85020254321;+5355555556;\n";
        for kind in ["clients", "suppliers"] {
            import_csv_inner(&mut connection, now, kind, &write_file(contact)).unwrap();
            let content = export(
                &mut connection,
                now,
                kind,
                CsvFilters {
                    search: Some("perez".to_owned()),
                    ..CsvFilters::default()
                },
            );
            let lines: Vec<&str> = content.lines().collect();
            assert_eq!(lines.len(), 2, "{kind}");
            assert!(
                lines[1].starts_with("Ana;Pérez;90010112345;'+5355555555;Goyma"),
                "{kind}: {}",
                lines[1]
            );
        }
    }

    fn insert_invoice(connection: &Connection, kind: &str, number: &str, status: &str) {
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, createdByName,
                    createdAt, updatedAt)
                 VALUES (?1, ?2, '2026-03-05', ?3, 'Ana Pérez', 'ana perez', '90010112345',
                    '+5355555555', 'csvuser', '2026-03-05T12:00:00Z', '2026-03-05T12:00:00Z')",
                (kind, number, status),
            )
            .unwrap();
        let invoice_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName,
                    productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt,
                    updatedAt)
                 VALUES (?1, 1, 1, 'Cable', 'cable', 1000, 150050, 150050,
                    '2026-03-05T12:00:00Z', '2026-03-05T12:00:00Z')",
                [invoice_id],
            )
            .unwrap();
    }

    #[test]
    fn export_invoices_labels_each_type_and_respects_the_filter() {
        let (mut connection, now) = signed_in();
        insert_invoice(&connection, "V", "V-00001", "pending");
        insert_invoice(&connection, "E", "E-00001", "pending");

        let all = export(&mut connection, now, "invoices", CsvFilters::default());
        assert!(all.contains("V-00001;Venta;Ana Pérez;05/03/2026;1500,50;Por Cobrar;1;csvuser"));
        assert!(all.contains("E-00001;Entrada;Ana Pérez;05/03/2026;1500,50;Por Pagar;1;csvuser"));

        let sales = export(
            &mut connection,
            now,
            "invoices",
            CsvFilters {
                invoices: Some(InvoiceFilters {
                    invoice_type: Some("V".to_owned()),
                    ..InvoiceFilters::default()
                }),
                ..CsvFilters::default()
            },
        );
        assert!(sales.contains("V-00001") && !sales.contains("E-00001"));
    }

    #[test]
    fn export_rejects_an_unknown_list_and_reports_a_failed_write() {
        let (mut connection, now) = signed_in();
        let path = temp_dir().join("salida.csv");
        let error = export_csv_inner(&mut connection, now, "users", CsvFilters::default(), &path)
            .unwrap_err();
        assert_eq!(error.code, "validationFailed");

        let missing = temp_dir().join("no-existe").join("salida.csv");
        let error = export_csv_inner(
            &mut connection,
            now,
            "units",
            CsvFilters::default(),
            &missing,
        )
        .unwrap_err();
        assert_eq!(error.code, "exportFailed");
    }

    #[test]
    fn import_products_creates_what_is_missing_with_initial_lot_and_movement() {
        let (mut connection, now) = signed_in();
        let path = write_file(
            "Nombre;Descripción;Categoría;Unidad;Costo;% de Ganancia;Cantidad;Umbral de Pocas Unidades;Caducidad\n\
             Cable;Rojo;Eléctricos;Metro;10,50;50;2,5;1;05/03/2099\n\
             Bomba;;Eléctricos;;20;10;1;2;\n",
        );

        let summary = import_csv_inner(&mut connection, now, "products", &path).unwrap();

        assert_eq!(summary.imported, 2);
        assert!(summary.skipped.is_empty());
        assert_eq!(count(&connection, "categories"), 1);
        assert_eq!(count(&connection, "units"), 1);
        assert_eq!(count(&connection, "lots"), 2);
        let (cost, quantity, expires): (i64, i64, String) = connection
            .query_row(
                "SELECT costCents, quantityMilli, expiresOn FROM products WHERE name = 'Cable'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((cost, quantity), (1050, 2500));
        assert_eq!(expires, "2099-03-05");
        let initial: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE type = 'initial'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(initial, 2);
    }

    #[test]
    fn a_skipped_product_leaves_no_new_category_or_unit() {
        let (mut connection, now) = signed_in();
        let path = write_file(
            "Nombre;Categoría;Unidad;Costo;% de Ganancia;Cantidad;Umbral de Pocas Unidades\n\
             Cable;Nueva;Otra;no es costo;50;1;1\n",
        );

        let summary = import_csv_inner(&mut connection, now, "products", &path).unwrap();

        assert_eq!(summary.imported, 0);
        assert_eq!(summary.skipped.len(), 1);
        assert_eq!(count(&connection, "categories"), 0);
        assert_eq!(count(&connection, "units"), 0);
    }

    #[test]
    fn duplicated_or_invalid_rows_are_skipped_with_row_and_reason() {
        let (mut connection, now) = signed_in();
        let path = write_file(
            "Nombre;Abreviatura\n\
             Metro;m\n\
             metro;mt\n\
             ;x\n\
             Litro;l\n",
        );

        let summary = import_csv_inner(&mut connection, now, "units", &path).unwrap();

        assert_eq!(summary.imported, 2);
        let rows: Vec<usize> = summary.skipped.iter().map(|item| item.row).collect();
        assert_eq!(rows, vec![3, 4]);
        assert!(summary.skipped.iter().all(|item| !item.reason.is_empty()));
        assert_eq!(count(&connection, "units"), 2);
    }

    #[test]
    fn import_categories_clients_and_suppliers() {
        let (mut connection, now) = signed_in();
        let categories = write_file("Nombre;Descripción\nCables;Todos\n");
        let contact = "Nombre;Apellidos;Carnet de Identidad;Teléfono;Empresa\n\
                       Ana;Pérez;90010112345;'+5355555555;Goyma\n\
                       Ana;Otra;90010112345;+5355555556;\n";
        let clients = write_file(contact);
        let suppliers = write_file(contact);

        import_csv_inner(&mut connection, now, "categories", &categories).unwrap();
        let client_summary = import_csv_inner(&mut connection, now, "clients", &clients).unwrap();
        let supplier_summary =
            import_csv_inner(&mut connection, now, "suppliers", &suppliers).unwrap();

        assert_eq!(count(&connection, "categories"), 1);
        for summary in [client_summary, supplier_summary] {
            assert_eq!(summary.imported, 1);
            assert_eq!(summary.skipped[0].row, 3);
        }
        let phone: String = connection
            .query_row("SELECT phone FROM clients", [], |row| row.get(0))
            .unwrap();
        assert_eq!(phone, "+5355555555");
    }

    #[test]
    fn what_is_exported_can_be_imported_again() {
        let (mut connection, now) = signed_in();
        create_product_inner(
            &mut connection,
            now,
            today_local(now),
            product_input("Cable"),
        )
        .unwrap();
        let content = export(&mut connection, now, "products", CsvFilters::default());
        connection.execute("DELETE FROM products", []).unwrap();

        let summary =
            import_csv_inner(&mut connection, now, "products", &write_file(&content)).unwrap();

        assert_eq!(summary.imported, 1, "{:?}", summary.skipped);
        let description: String = connection
            .query_row("SELECT description FROM products", [], |row| row.get(0))
            .unwrap();
        assert_eq!(description, "=SUMA(A1)");
    }

    #[test]
    fn unusable_files_are_rejected_without_importing() {
        let (mut connection, now) = signed_in();
        let too_long = format!("Nombre\n{}", "Categoría\n".repeat(MAX_ROWS + 1));
        for (content, message) in [
            ("", "El archivo está vacío"),
            ("Nombre;Abreviatura\n", "El archivo está vacío"),
            (
                "Nombre;Abreviatura;Color\nMetro;m;rojo\n",
                "El archivo tiene una columna desconocida: «Color»",
            ),
            (
                "Nombre\nMetro\n",
                "Al archivo le falta la columna «Abreviatura»",
            ),
        ] {
            let error =
                import_csv_inner(&mut connection, now, "units", &write_file(content)).unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, message);
        }
        let error = import_csv_inner(&mut connection, now, "categories", &write_file(&too_long))
            .unwrap_err();
        assert_eq!(error.message, "El archivo no puede tener más de 5000 filas");

        let latin1 = temp_dir().join("latin1.csv");
        fs::write(&latin1, b"Nombre;Abreviatura\nA\xF1o;a\n").unwrap();
        let error = import_csv_inner(&mut connection, now, "units", &latin1).unwrap_err();
        assert_eq!(
            error.message,
            "El archivo debe estar guardado como CSV UTF-8"
        );

        let error = import_csv_inner(&mut connection, now, "invoices", &latin1).unwrap_err();
        assert_eq!(error.message, "Este tipo de datos no se puede importar");
        assert_eq!(count(&connection, "units"), 0);
        assert_eq!(count(&connection, "activityLog"), 0);
    }

    #[test]
    fn an_import_is_recorded_with_only_the_file_name_and_summary() {
        let (mut connection, now) = signed_in();
        let path = write_file("Nombre;Abreviatura\nMetro;m\nmetro;x\n");

        import_csv_inner(&mut connection, now, "units", &path).unwrap();

        let (action, entity, label, details): (String, String, String, String) = connection
            .query_row(
                "SELECT action, entityType, entityLabel, details FROM activityLog",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!((action.as_str(), entity.as_str()), ("import", "csv"));
        assert_eq!(label, "datos.csv");
        assert_eq!(details, "1 importadas, 1 omitidas");
    }

    #[test]
    fn csv_commands_require_session_and_allow_both_roles() {
        let dir = temp_dir();
        let destination = dir.join("salida.csv");
        let source = write_file("Nombre;Abreviatura\nMetro;m\n");
        assert_requires_session(|connection, now| {
            export_csv_inner(
                connection,
                now,
                "units",
                CsvFilters::default(),
                &destination,
            )
        });
        assert_requires_session(|connection, now| {
            import_csv_inner(connection, now, "units", &source)
        });
        assert_allows_both_roles(|connection, now| {
            export_csv_inner(
                connection,
                now,
                "units",
                CsvFilters::default(),
                &destination,
            )
        });
        assert_allows_both_roles(|connection, now| {
            import_csv_inner(connection, now, "units", &source)
        });
    }
}
