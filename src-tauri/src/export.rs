//! Exportación del historial de un producto a `.xlsx` y `.pdf` (HP-03,
//! plan-03 §3.10). Los archivos se arman en memoria y se escriben en un
//! temporal que se renombra al final: un fallo no deja un archivo a medias.

use std::io::{Cursor, Write};
use std::path::Path;

use chrono::{DateTime, Local, NaiveDate, Utc};
use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfLayerReference};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::errors::AppError;
use crate::models::product_movement::ProductHistoryItem;

/// Datos del encabezado: producto, filtros activos y momento de generación.
pub struct HistoryHeader {
    pub product_name: String,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub types: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

const COLUMNS: [&str; 8] = [
    "Fecha",
    "Tipo",
    "Cantidad",
    "Stock Resultante",
    "Precio Unitario",
    "Precio Total",
    "Factura",
    "Usuario",
];

fn write_failed(detail: &str) -> AppError {
    // La causa no incluye la ruta: puede contener el nombre del usuario.
    AppError::new(
        "internal",
        "No se pudo guardar el archivo",
        &format!("history export failed: {detail}"),
    )
}

/// Escribe en un temporal junto al destino y lo renombra al final (plan-03
/// §3.10): si algo falla, el destino no queda a medias.
fn write_atomically(destination: &Path, bytes: &[u8], kind: &str) -> Result<(), AppError> {
    let file_name = destination
        .file_name()
        .ok_or_else(|| write_failed(&format!("{kind} destination has no file name")))?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(".tmp");
    let temp = destination.with_file_name(temp_name);
    let result = std::fs::write(&temp, bytes)
        .and_then(|()| std::fs::rename(&temp, destination))
        .map_err(|error| write_failed(&format!("{kind} write: {:?}", error.kind())));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

pub fn movement_type_label(code: &str) -> &str {
    match code {
        "initial" => "Inicial",
        "entry" => "Entrada",
        "saleOut" => "Salida Venta",
        "serviceOut" => "Salida Servicio",
        "warrantyOut" => "Salida Garantía",
        "supplyOut" => "Salida Insumos",
        "loss" => "Pérdida",
        "adjustment" => "Ajuste",
        "reversal" => "Reversión",
        other => other,
    }
}

/// Importe como en la pantalla (`es-ES`): `1234,50 USD`, `12.345,67 USD`.
/// Igual que `Intl.NumberFormat("es-ES")`, solo agrupa miles desde 5 cifras.
pub fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let integer = (abs / 100).to_string();
    let grouped = if integer.len() >= 5 {
        let mut out = String::new();
        for (index, digit) in integer.chars().enumerate() {
            if index > 0 && (integer.len() - index).is_multiple_of(3) {
                out.push('.');
            }
            out.push(digit);
        }
        out
    } else {
        integer
    };
    format!("{sign}{grouped},{:02} USD", abs % 100)
}

/// Fecha y hora locales `dd/mm/aaaa HH:MM`, como `formatTimestamp` del frontend.
pub fn format_local_timestamp(timestamp: &str) -> String {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|value| {
            value
                .with_timezone(&Local)
                .format("%d/%m/%Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "—".to_owned())
}

fn format_date(value: &str) -> String {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| date.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|_| value.to_owned())
}

fn header_lines(header: &HistoryHeader) -> Vec<(&'static str, String)> {
    let range = match (header.from_date.as_deref(), header.to_date.as_deref()) {
        (Some(from), Some(to)) => format!("Desde {} hasta {}", format_date(from), format_date(to)),
        (Some(from), None) => format!("Desde {}", format_date(from)),
        (None, Some(to)) => format!("Hasta {}", format_date(to)),
        (None, None) => "Todas las fechas".to_owned(),
    };
    let types = if header.types.is_empty() {
        "Todos".to_owned()
    } else {
        header
            .types
            .iter()
            .map(|code| movement_type_label(code))
            .collect::<Vec<_>>()
            .join(", ")
    };
    vec![
        ("Producto", header.product_name.clone()),
        ("Rango de fechas", range),
        ("Tipos", types),
        (
            "Generado",
            header
                .generated_at
                .with_timezone(&Local)
                .format("%d/%m/%Y %H:%M")
                .to_string(),
        ),
    ]
}

/// Textos de una fila, en el orden de `COLUMNS` (los mismos que la pantalla).
fn row_texts(item: &ProductHistoryItem) -> [String; 8] {
    [
        format_local_timestamp(&item.created_at),
        movement_type_label(&item.movement_type).to_owned(),
        item.quantity.clone(),
        item.stock_after.clone(),
        format_money(item.unit_price_cents),
        format_money(item.total_price_cents),
        item.invoice_number
            .clone()
            .unwrap_or_else(|| "—".to_owned()),
        item.user_name.clone(),
    ]
}

// ------------------------------- .xlsx -------------------------------

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 no admite caracteres de control salvo tab y saltos.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

fn column_letter(index: usize) -> char {
    (b'A' + index as u8) as char
}

fn text_cell(column: usize, row: usize, value: &str, style: u8) -> String {
    format!(
        r#"<c r="{}{row}" t="inlineStr" s="{style}"><is><t xml:space="preserve">{}</t></is></c>"#,
        column_letter(column),
        xml_escape(value)
    )
}

fn money_cell(column: usize, row: usize, cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!(
        r#"<c r="{}{row}" s="1"><v>{sign}{}.{:02}</v></c>"#,
        column_letter(column),
        abs / 100,
        abs % 100
    )
}

fn sheet_xml(items: &[ProductHistoryItem], header: &HistoryHeader) -> String {
    let mut rows = Vec::new();
    let mut row = 1;
    rows.push(format!(
        r#"<row r="{row}">{}</row>"#,
        text_cell(0, row, "Historial de producto", 2)
    ));
    for (label, value) in header_lines(header) {
        row += 1;
        rows.push(format!(
            r#"<row r="{row}">{}{}</row>"#,
            text_cell(0, row, label, 2),
            text_cell(1, row, &value, 0)
        ));
    }
    row += 2;
    let header_cells: String = COLUMNS
        .iter()
        .enumerate()
        .map(|(column, title)| text_cell(column, row, title, 2))
        .collect();
    rows.push(format!(r#"<row r="{row}">{header_cells}</row>"#));
    for item in items {
        row += 1;
        let texts = row_texts(item);
        let mut cells = String::new();
        for (column, text) in texts.iter().enumerate() {
            match column {
                // Importes como números con formato, para poder sumarlos.
                4 => cells.push_str(&money_cell(column, row, item.unit_price_cents)),
                5 => cells.push_str(&money_cell(column, row, item.total_price_cents)),
                _ => cells.push_str(&text_cell(column, row, text, 0)),
            }
        }
        rows.push(format!(r#"<row r="{row}">{cells}</row>"#));
    }
    let widths = [18, 18, 12, 16, 18, 18, 14, 24];
    let cols: String = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            format!(
                r#"<col min="{0}" max="{0}" width="{width}" customWidth="1"/>"#,
                index + 1
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cols>{cols}</cols><sheetData>{}</sheetData></worksheet>"#,
        rows.join("")
    )
}

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;

const ROOT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const WORKBOOK_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Historial" sheetId="1" r:id="rId1"/></sheets></workbook>"#;

const WORKBOOK_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

// Estilos: 0 normal, 1 importe `#,##0.00 "USD"`, 2 negrita (encabezados).
const STYLES_XML: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00 &quot;USD&quot;"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"##;

/// Bytes del `.xlsx`: SpreadsheetML mínimo en un ZIP sin compresión.
pub fn history_xlsx_bytes(
    items: &[ProductHistoryItem],
    header: &HistoryHeader,
) -> Result<Vec<u8>, AppError> {
    let sheet = sheet_xml(items, header);
    let parts: [(&str, &str); 6] = [
        ("[Content_Types].xml", CONTENT_TYPES_XML),
        ("_rels/.rels", ROOT_RELS_XML),
        ("xl/workbook.xml", WORKBOOK_XML),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS_XML),
        ("xl/styles.xml", STYLES_XML),
        ("xl/worksheets/sheet1.xml", &sheet),
    ];
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in parts {
        writer
            .start_file(name, options)
            .map_err(|_| write_failed("xlsx zip entry"))?;
        writer
            .write_all(content.as_bytes())
            .map_err(|_| write_failed("xlsx zip write"))?;
    }
    let cursor = writer
        .finish()
        .map_err(|_| write_failed("xlsx zip finish"))?;
    Ok(cursor.into_inner())
}

pub fn export_history_xlsx(
    items: &[ProductHistoryItem],
    header: &HistoryHeader,
    destination: &Path,
) -> Result<(), AppError> {
    let bytes = history_xlsx_bytes(items, header)?;
    write_atomically(destination, &bytes, "xlsx")
}

// -------------------------------- .pdf --------------------------------

// A4 apaisado, en milímetros.
const PAGE_WIDTH: f32 = 297.0;
const PAGE_HEIGHT: f32 = 210.0;
const MARGIN: f32 = 12.0;
const ROW_HEIGHT: f32 = 5.5;
const COLUMN_X: [f32; 8] = [12.0, 46.0, 82.0, 106.0, 132.0, 165.0, 198.0, 226.0];
// Caracteres máximos por columna con Helvetica 9 pt.
const COLUMN_CHARS: [usize; 8] = [20, 21, 14, 15, 20, 20, 16, 34];

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        let kept: String = value.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

fn pdf_error(_: printpdf::Error) -> AppError {
    write_failed("pdf generation")
}

struct PdfCursor<'a> {
    document: &'a printpdf::PdfDocumentReference,
    layer: PdfLayerReference,
    y: f32,
}

impl PdfCursor<'_> {
    fn new_page(&mut self) {
        let (page, layer) = self
            .document
            .add_page(Mm(PAGE_WIDTH), Mm(PAGE_HEIGHT), "Capa 1");
        self.layer = self.document.get_page(page).get_layer(layer);
        self.y = PAGE_HEIGHT - MARGIN;
    }

    fn table_header(&mut self, bold: &IndirectFontRef) {
        for (index, title) in COLUMNS.iter().enumerate() {
            self.layer
                .use_text(*title, 9.0, Mm(COLUMN_X[index]), Mm(self.y), bold);
        }
        self.y -= ROW_HEIGHT + 1.0;
    }
}

/// Bytes del `.pdf` con Helvetica integrada (WinAnsi: todo el español y €).
pub fn history_pdf_bytes(
    items: &[ProductHistoryItem],
    header: &HistoryHeader,
) -> Result<Vec<u8>, AppError> {
    let title = format!("Historial de {}", header.product_name);
    let (document, page, layer) =
        PdfDocument::new(&title, Mm(PAGE_WIDTH), Mm(PAGE_HEIGHT), "Capa 1");
    let regular = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(pdf_error)?;
    let bold = document
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(pdf_error)?;

    let mut cursor = PdfCursor {
        document: &document,
        layer: document.get_page(page).get_layer(layer),
        y: PAGE_HEIGHT - MARGIN,
    };
    cursor
        .layer
        .use_text(truncate(&title, 80), 14.0, Mm(MARGIN), Mm(cursor.y), &bold);
    cursor.y -= 8.0;
    for (label, value) in header_lines(header) {
        cursor.layer.use_text(
            truncate(&format!("{label}: {value}"), 120),
            10.0,
            Mm(MARGIN),
            Mm(cursor.y),
            &regular,
        );
        cursor.y -= 5.0;
    }
    cursor.y -= 4.0;
    cursor.table_header(&bold);

    for item in items {
        if cursor.y < MARGIN {
            cursor.new_page();
            cursor.table_header(&bold);
        }
        for (index, text) in row_texts(item).iter().enumerate() {
            cursor.layer.use_text(
                truncate(text, COLUMN_CHARS[index]),
                9.0,
                Mm(COLUMN_X[index]),
                Mm(cursor.y),
                &regular,
            );
        }
        cursor.y -= ROW_HEIGHT;
    }

    document.save_to_bytes().map_err(pdf_error)
}

pub fn export_history_pdf(
    items: &[ProductHistoryItem],
    header: &HistoryHeader,
    destination: &Path,
) -> Result<(), AppError> {
    let bytes = history_pdf_bytes(items, header)?;
    write_atomically(destination, &bytes, "pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::io::Read;

    fn item(movement_type: &str, invoice: Option<&str>, cents: i64) -> ProductHistoryItem {
        ProductHistoryItem {
            id: 1,
            created_at: "2026-10-04T12:00:00Z".to_owned(),
            movement_type: movement_type.to_owned(),
            quantity: "-1,5".to_owned(),
            stock_after: "8,5".to_owned(),
            unit_price_cents: cents,
            total_price_cents: cents * 2,
            invoice_number: invoice.map(str::to_owned),
            invoice_type: invoice.map(|_| "V".to_owned()),
            user_name: "María Ñúñez".to_owned(),
            product_id_original: 7,
        }
    }

    fn header(name: &str) -> HistoryHeader {
        HistoryHeader {
            product_name: name.to_owned(),
            from_date: Some("2026-10-01".to_owned()),
            to_date: None,
            types: vec!["saleOut".to_owned(), "loss".to_owned()],
            generated_at: Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn money_matches_the_es_es_screen_format() {
        assert_eq!(format_money(0), "0,00 USD");
        assert_eq!(format_money(123_450), "1234,50 USD");
        assert_eq!(format_money(1_234_567), "12.345,67 USD");
        assert_eq!(format_money(123_456_789_012), "1.234.567.890,12 USD");
        assert_eq!(format_money(-505), "-5,05 USD");
    }

    #[test]
    fn xlsx_contains_the_header_columns_and_rows_as_valid_parts() {
        let items = vec![
            item("saleOut", Some("V-00001"), 49_380),
            item("loss", None, 100),
        ];
        let bytes = history_xlsx_bytes(&items, &header("Café & <Leche>")).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
            "xl/styles.xml",
        ] {
            assert!(archive.by_name(part).is_ok(), "falta {part}");
        }
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        // Encabezado con filtros; el nombre se escapa como XML.
        assert!(sheet.contains("Café &amp; &lt;Leche&gt;"));
        assert!(sheet.contains("Desde 01/10/2026"));
        assert!(sheet.contains("Salida Venta, Pérdida"));
        for column in COLUMNS {
            assert!(sheet.contains(column), "columna {column}");
        }
        // Filas: tipo traducido, cantidad con signo, factura, usuario e importes numéricos.
        assert!(sheet.contains("Salida Venta"));
        assert!(sheet.contains("-1,5"));
        assert!(sheet.contains("V-00001"));
        assert!(sheet.contains("María Ñúñez"));
        assert!(sheet.contains(r#"<v>493.80</v>"#));
        assert!(sheet.contains(r#"<v>987.60</v>"#));
        assert_eq!(sheet.matches("<row ").count(), 1 + 4 + 1 + items.len());
    }

    /// Texto que escribe el PDF, decodificado con la codificación de su fuente.
    fn pdf_text(bytes: Vec<u8>) -> (String, Vec<String>) {
        use printpdf::lopdf::{content::Content, Document, Object};
        let document = Document::load_mem(&bytes).unwrap();
        let mut text = String::new();
        for page_id in document.get_pages().values() {
            let content = document.get_page_content(*page_id).unwrap();
            for operation in Content::decode(&content).unwrap().operations {
                if operation.operator == "Tj" {
                    if let Some(Object::String(raw, _)) = operation.operands.first() {
                        text.push_str(&Document::decode_text(Some("WinAnsiEncoding"), raw));
                        text.push('\n');
                    }
                }
            }
        }
        let fonts = document
            .objects
            .values()
            .filter_map(|object| object.as_dict().ok())
            .filter(|dict| {
                dict.get(b"Type")
                    .and_then(|value| value.as_name_str())
                    .is_ok_and(|name| name == "Font")
            })
            .map(|dict| {
                format!(
                    "{}/{}",
                    dict.get(b"BaseFont")
                        .and_then(|value| value.as_name_str())
                        .unwrap_or("?"),
                    dict.get(b"Encoding")
                        .and_then(|value| value.as_name_str())
                        .unwrap_or("?")
                )
            })
            .collect();
        (text, fonts)
    }

    #[test]
    fn pdf_paints_spanish_and_euro_with_builtin_helvetica_winansi() {
        // Spike D-18: Helvetica integrada con WinAnsi cubre todo el español.
        let bytes = history_pdf_bytes(
            &[item("warrantyOut", Some("GG-00001"), 100)],
            &header("á é í ó ú ü ñ ¿ ¡ €"),
        )
        .unwrap();
        let (text, fonts) = pdf_text(bytes);
        assert!(text.contains("á é í ó ú ü ñ ¿ ¡ €"), "{text}");
        assert!(text.contains("Salida Garantía"));
        assert!(text.contains("María Ñúñez"));
        assert!(
            fonts.contains(&"Helvetica/WinAnsiEncoding".to_owned()),
            "{fonts:?}"
        );
        assert!(
            fonts.contains(&"Helvetica-Bold/WinAnsiEncoding".to_owned()),
            "{fonts:?}"
        );
    }

    #[test]
    fn pdf_contains_header_and_rows_and_paginates_long_histories() {
        let items: Vec<_> = (0..80)
            .map(|index| item("entry", Some(&format!("E-{index:05}")), 100 + index))
            .collect();
        let bytes = history_pdf_bytes(&items, &header("Cable")).unwrap();
        let pages = printpdf::lopdf::Document::load_mem(&bytes)
            .unwrap()
            .get_pages()
            .len();
        let (text, _) = pdf_text(bytes);
        assert!(text.contains("Historial de Cable"));
        assert!(text.contains("Producto: Cable"));
        assert!(text.contains("E-00000") && text.contains("E-00079"));
        assert!(pages > 1, "80 filas no caben en una página");
        // La cabecera de la tabla se repite en cada página.
        assert_eq!(text.matches("Stock Resultante").count(), pages);
    }

    #[test]
    fn export_replaces_an_existing_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("goyma-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("historial.pdf");
        std::fs::write(&destination, b"viejo").unwrap();
        export_history_pdf(&[item("entry", None, 1)], &header("X"), &destination).unwrap();
        assert!(std::fs::read(&destination).unwrap().starts_with(b"%PDF"));
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("historial.pdf")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_failure_maps_to_a_clear_internal_error() {
        let missing = std::env::temp_dir()
            .join("goyma-export-no-existe")
            .join("sub")
            .join("historial.xlsx");
        for result in [
            export_history_xlsx(&[item("entry", None, 1)], &header("X"), &missing),
            export_history_pdf(&[item("entry", None, 1)], &header("X"), &missing),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.code, "internal");
            assert_eq!(error.message, "No se pudo guardar el archivo");
            assert!(!error.cause.contains("goyma-export-no-existe"));
        }
    }
}
