//! Impresión y PDF de facturas (spec-04 IM-01, IM-02). Un único modelo de
//! documento alimenta la vista previa HTML y el PDF del backend, para que no
//! diverjan (D-05). El documento **no muestra el estado** (RF-129).

use std::path::Path;

use chrono::NaiveDate;
use printpdf::{
    BuiltinFont, ColorBits, ColorSpace, Image, ImageFilter, ImageTransform, ImageXObject,
    IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference, Px,
};
use serde::Serialize;

use crate::commands::settings::jpeg_dimensions;
use crate::errors::AppError;
use crate::export::format_money;
use crate::models::invoice::InvoiceDetail;
use crate::models::settings::CompanyProfile;

/// Contacto tal como se guardó en la factura (copia, `spec-03` §7.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintContact {
    pub label: String,
    pub name: String,
    pub company: Option<String>,
    pub national_id: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintLine {
    pub name: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price_cents: i64,
    /// Bruto de la línea; con descuento, el importe es este menos el descuento.
    pub subtotal_cents: i64,
    /// spec-06 PF-02: se imprime en una fila propia debajo de la línea.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_cents: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicePrintModel {
    pub title: String,
    pub number: String,
    pub date: String,
    pub company: CompanyProfile,
    pub contact: Option<PrintContact>,
    pub price_label: String,
    pub lines: Vec<PrintLine>,
    pub total_cents: i64,
    pub notes_label: String,
    pub notes: Option<String>,
    pub associated: Option<String>,
    pub reserved_until: Option<String>,
}

fn title_for(invoice_type: &str) -> &'static str {
    match invoice_type {
        "E" => "Factura de Compra",
        "V" => "Factura de Venta",
        "S" => "Factura de Servicio",
        "GG" => "Factura de Garantía",
        "GI" => "Factura de Insumos",
        _ => "Factura de Pérdida",
    }
}

fn type_label(invoice_type: &str) -> &'static str {
    match invoice_type {
        "V" => "Venta",
        "S" => "Servicio",
        _ => "Factura",
    }
}

fn display_date(value: &str) -> String {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| date.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|_| value.to_owned())
}

/// Arma el documento con los datos guardados de la factura: reimprimir no
/// recalcula nada (RF-131).
pub fn build_invoice_document(
    detail: &InvoiceDetail,
    company: &CompanyProfile,
) -> InvoicePrintModel {
    let invoice_type = detail.invoice_type.as_str();
    let contact = detail.contact_full_name.as_ref().map(|name| PrintContact {
        label: if invoice_type == "E" {
            "Proveedor"
        } else {
            "Cliente"
        }
        .to_owned(),
        name: name.clone(),
        company: detail
            .contact_company
            .clone()
            .filter(|value| !value.is_empty()),
        national_id: detail.contact_national_id.clone(),
        phone: detail.contact_phone.clone(),
        email: detail
            .contact_email
            .clone()
            .filter(|value| !value.is_empty()),
    });
    let lines = detail
        .lines
        .iter()
        // CV-01: se imprime lo escrito ("2 Caja"), no lo que se movió.
        .map(|line| match &line.entered {
            Some(entered) => PrintLine {
                name: line.product_name.clone(),
                quantity: entered.quantity.clone(),
                unit: entered.abbreviation.clone(),
                unit_price_cents: entered.unit_price_cents,
                subtotal_cents: line.gross_cents.unwrap_or(line.subtotal_cents),
                discount_cents: line.discount_cents,
            },
            None => PrintLine {
                name: line.product_name.clone(),
                quantity: line.quantity.clone(),
                unit: line
                    .unit_abbreviation
                    .clone()
                    .or_else(|| line.unit_name.clone())
                    .unwrap_or_default(),
                unit_price_cents: line.unit_price_cents,
                subtotal_cents: line.gross_cents.unwrap_or(line.subtotal_cents),
                discount_cents: line.discount_cents,
            },
        })
        .collect();
    InvoicePrintModel {
        title: title_for(invoice_type).to_owned(),
        number: detail.number.clone(),
        date: display_date(&detail.date),
        company: company.clone(),
        contact,
        price_label: if matches!(invoice_type, "V" | "S") {
            "Precio unitario"
        } else {
            "Costo unitario"
        }
        .to_owned(),
        lines,
        total_cents: detail.total_cents,
        notes_label: if invoice_type == "E" {
            "Observaciones"
        } else {
            "Descripción"
        }
        .to_owned(),
        notes: detail.notes.clone().filter(|value| !value.is_empty()),
        associated: detail
            .associated_invoice
            .as_ref()
            .map(|invoice| format!("{} ({})", invoice.number, type_label(&invoice.invoice_type))),
        reserved_until: detail.reserved_until.as_deref().map(display_date),
    }
}

// ---------- PDF A4 (IM-02) ----------

const PAGE_WIDTH: f32 = 210.0;
const PAGE_HEIGHT: f32 = 297.0;
const MARGIN: f32 = 15.0;
const ROW_HEIGHT: f32 = 6.0;
const LOGO_MAX_WIDTH: f32 = 40.0;
const LOGO_MAX_HEIGHT: f32 = 25.0;
// Producto, cantidad, unidad, precio/costo y subtotal.
const COLUMN_X: [f32; 5] = [15.0, 100.0, 122.0, 142.0, 172.0];
const NAME_CHARS: usize = 46;

fn export_failed(cause: &str) -> AppError {
    // La causa no incluye la ruta: puede contener el nombre del usuario.
    AppError::new(
        "exportFailed",
        "No se pudo guardar el archivo",
        &format!("invoice pdf export failed: {cause}"),
    )
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        let kept: String = value.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

/// Parte un texto largo en renglones de como máximo `width` caracteres.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&truncate(word, width));
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

struct Pdf<'a> {
    document: &'a PdfDocumentReference,
    layer: PdfLayerReference,
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    y: f32,
}

impl Pdf<'_> {
    fn text(&self, value: &str, size: f32, x: f32, bold: bool) {
        let font = if bold { &self.bold } else { &self.regular };
        self.layer.use_text(value, size, Mm(x), Mm(self.y), font);
    }

    fn line(&mut self, value: &str, size: f32, bold: bool) {
        self.ensure_room(size * 0.5);
        self.text(value, size, MARGIN, bold);
        self.y -= size * 0.5;
    }

    fn ensure_room(&mut self, height: f32) {
        if self.y - height < MARGIN {
            let (page, layer) = self
                .document
                .add_page(Mm(PAGE_WIDTH), Mm(PAGE_HEIGHT), "Capa 1");
            self.layer = self.document.get_page(page).get_layer(layer);
            self.y = PAGE_HEIGHT - MARGIN;
        }
    }

    fn table_header(&mut self, price_label: &str) {
        for (index, title) in ["Producto", "Cantidad", "Unidad", price_label, "Subtotal"]
            .iter()
            .enumerate()
        {
            self.text(title, 9.5, COLUMN_X[index], true);
        }
        self.y -= ROW_HEIGHT;
    }
}

/// Dibuja el logo JPEG sin decodificarlo (filtro DCT, D-17) y devuelve el
/// alto que ocupa.
fn draw_logo(layer: &PdfLayerReference, jpeg: &[u8], top: f32) -> Option<f32> {
    let (width, height, components) = jpeg_dimensions(jpeg)?;
    let (width, height) = (f32::from(width), f32::from(height));
    // Puntos por pulgada para que quepa en 40 × 25 mm sin deformarse.
    let dpi = (width * 25.4 / LOGO_MAX_WIDTH).max(height * 25.4 / LOGO_MAX_HEIGHT);
    let drawn_height = height * 25.4 / dpi;
    Image::from(ImageXObject {
        width: Px(width as usize),
        height: Px(height as usize),
        color_space: if components == 3 {
            ColorSpace::Rgb
        } else {
            ColorSpace::Greyscale
        },
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: jpeg.to_vec(),
        image_filter: Some(ImageFilter::DCT),
        smask: None,
        clipping_bbox: None,
    })
    .add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(MARGIN)),
            translate_y: Some(Mm(top - drawn_height)),
            dpi: Some(dpi),
            ..ImageTransform::default()
        },
    );
    Some(drawn_height)
}

/// Bytes del PDF A4 con el mismo contenido que la vista previa.
pub fn invoice_pdf_bytes(
    model: &InvoicePrintModel,
    logo: Option<&[u8]>,
) -> Result<Vec<u8>, AppError> {
    let title = format!("{} {}", model.title, model.number);
    let (document, page, layer) =
        PdfDocument::new(&title, Mm(PAGE_WIDTH), Mm(PAGE_HEIGHT), "Capa 1");
    let regular = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|_| export_failed("font"))?;
    let bold = document
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|_| export_failed("font"))?;
    let mut pdf = Pdf {
        document: &document,
        layer: document.get_page(page).get_layer(layer),
        regular,
        bold,
        y: PAGE_HEIGHT - MARGIN,
    };

    // Encabezado: logo a la izquierda y datos de la empresa a su derecha.
    let top = pdf.y;
    let logo_height = logo.and_then(|bytes| draw_logo(&pdf.layer, bytes, top));
    let company_x = if logo_height.is_some() {
        MARGIN + LOGO_MAX_WIDTH + 5.0
    } else {
        MARGIN
    };
    pdf.y = top - 4.0;
    let company = &model.company;
    let labeled = |label: &str, value: &str| {
        if value.is_empty() {
            String::new()
        } else {
            format!("{label}: {value}")
        }
    };
    let company_lines = [
        (company.name.clone(), 12.0, true),
        (company.address.clone(), 9.5, false),
        (labeled("Teléfono", &company.phone), 9.5, false),
        (labeled("NIT", &company.tax_id), 9.5, false),
    ];
    for (value, size, is_bold) in company_lines
        .iter()
        .filter(|(value, _, _)| !value.is_empty())
    {
        pdf.text(&truncate(value, 70), *size, company_x, *is_bold);
        pdf.y -= 5.0;
    }
    pdf.y = pdf.y.min(top - logo_height.unwrap_or(0.0)) - 8.0;

    pdf.line(&model.title, 16.0, true);
    pdf.y -= 1.0;
    pdf.line(
        &format!("No. {}    Fecha: {}", model.number, model.date),
        10.0,
        false,
    );
    if let Some(reserved) = &model.reserved_until {
        pdf.line(&format!("Reservada hasta: {reserved}"), 10.0, false);
    }
    if let Some(associated) = &model.associated {
        pdf.line(&format!("Factura asociada: {associated}"), 10.0, false);
    }
    pdf.y -= 3.0;

    if let Some(contact) = &model.contact {
        pdf.line(
            &truncate(&format!("{}: {}", contact.label, contact.name), 90),
            10.5,
            true,
        );
        for value in [
            contact.company.clone(),
            contact.national_id.as_ref().map(|id| format!("CI: {id}")),
            contact
                .phone
                .as_ref()
                .map(|phone| format!("Teléfono: {phone}")),
            contact
                .email
                .as_ref()
                .map(|email| format!("Correo: {email}")),
        ]
        .into_iter()
        .flatten()
        {
            pdf.line(&truncate(&value, 90), 9.5, false);
        }
        pdf.y -= 4.0;
    }

    pdf.ensure_room(ROW_HEIGHT * 2.0);
    pdf.table_header(&model.price_label);
    for line in &model.lines {
        if pdf.y - ROW_HEIGHT < MARGIN {
            pdf.ensure_room(ROW_HEIGHT * 2.0);
            pdf.table_header(&model.price_label);
        }
        let cells = [
            truncate(&line.name, NAME_CHARS),
            line.quantity.clone(),
            truncate(&line.unit, 10),
            format_money(line.unit_price_cents),
            format_money(line.subtotal_cents),
        ];
        for (index, cell) in cells.iter().enumerate() {
            pdf.text(cell, 9.5, COLUMN_X[index], false);
        }
        pdf.y -= ROW_HEIGHT;
        if let Some(discount) = line.discount_cents {
            if pdf.y - ROW_HEIGHT < MARGIN {
                pdf.ensure_room(ROW_HEIGHT * 2.0);
            }
            pdf.text("   Descuento", 9.0, COLUMN_X[0], false);
            pdf.text(
                &format!("-{}", format_money(discount)),
                9.0,
                COLUMN_X[4],
                false,
            );
            pdf.y -= ROW_HEIGHT;
        }
    }
    pdf.y -= 2.0;
    pdf.ensure_room(8.0);
    pdf.text(
        &format!("Total: {}", format_money(model.total_cents)),
        12.0,
        COLUMN_X[3],
        true,
    );
    pdf.y -= 10.0;

    if let Some(notes) = &model.notes {
        pdf.line(&format!("{}:", model.notes_label), 10.0, true);
        for row in wrap(notes, 95) {
            pdf.line(&row, 9.5, false);
        }
    }

    document
        .save_to_bytes()
        .map_err(|_| export_failed("pdf generation"))
}

/// Escribe el PDF en un temporal junto al destino y lo renombra: si algo
/// falla, el destino no queda a medias.
pub fn export_invoice_pdf(
    model: &InvoicePrintModel,
    logo: Option<&[u8]>,
    destination: &Path,
) -> Result<(), AppError> {
    let bytes = invoice_pdf_bytes(model, logo)?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| export_failed("destination has no file name"))?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(".tmp");
    let temp = destination.with_file_name(temp_name);
    let result = std::fs::write(&temp, &bytes)
        .and_then(|()| std::fs::rename(&temp, destination))
        .map_err(|error| export_failed(&format!("write: {:?}", error.kind())));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{build_invoice_document, export_invoice_pdf, invoice_pdf_bytes};
    use crate::models::invoice::{AssociatedInvoice, InvoiceDetail};
    use crate::models::invoice_line::InvoiceLineDetail;
    use crate::models::settings::CompanyProfile;

    fn company(logo: bool) -> CompanyProfile {
        CompanyProfile {
            name: "Goyma Conexiones".into(),
            address: "Calle 23, Vedado".into(),
            phone: "+53 7 8300000".into(),
            tax_id: "".into(),
            logo_data_url: logo.then(|| "data:image/jpeg;base64,/9j/".into()),
        }
    }

    fn detail(invoice_type: &str) -> InvoiceDetail {
        let has_contact = invoice_type != "P";
        InvoiceDetail {
            id: 7,
            number: format!("{invoice_type}-00007"),
            invoice_type: invoice_type.into(),
            contact_name: has_contact.then(|| "Rosa Díaz".into()),
            date: "2026-11-04".into(),
            total_cents: 6_000,
            associated_invoice: matches!(invoice_type, "GG" | "GI").then(|| AssociatedInvoice {
                number: "S-00003".into(),
                invoice_type: "S".into(),
            }),
            status: Some("pending".into()),
            notes: Some("Entrega en tienda".into()),
            payment_method: None,
            paid_at: None,
            contact_id: Some(1),
            associated_invoice_id: None,
            contact_full_name: has_contact.then(|| "Rosa Díaz".into()),
            contact_company: Some(String::new()),
            contact_national_id: has_contact.then(|| "90010112345".into()),
            contact_phone: has_contact.then(|| "+5355550000".into()),
            contact_email: None,
            attachment_file_name: None,
            attachment_original_name: None,
            attachment_size_bytes: None,
            attachment_uploaded_at: None,
            created_by_name: "ana".into(),
            created_at: "2026-11-04T12:00:00Z".into(),
            updated_at: "2026-11-04T12:00:00Z".into(),
            reserved_until: (invoice_type == "V").then(|| "2026-11-10".into()),
            lines: vec![InvoiceLineDetail {
                list_price_cents: None,
                gross_cents: None,
                discount_kind: None,
                discount_value: None,
                discount_cents: None,
                delivered_quantity: "0".into(),
                position: 1,
                product_id_original: 3,
                product_name: "Cable UTP".into(),
                unit_name: Some("Metro".into()),
                unit_abbreviation: Some("m".into()),
                quantity: "24".into(),
                unit_price_cents: 250,
                subtotal_cents: 6_000,
                category_name: None,
                cost_at_sale_cents: None,
                product_status: Some("active".into()),
                remaining_quantity: None,
                consumed_quantity: None,
                entered: None,
            }],
        }
    }

    #[test]
    fn the_document_has_the_title_of_each_type_and_never_the_status() {
        for (invoice_type, title, price, contact) in [
            (
                "E",
                "Factura de Compra",
                "Costo unitario",
                Some("Proveedor"),
            ),
            ("V", "Factura de Venta", "Precio unitario", Some("Cliente")),
            (
                "S",
                "Factura de Servicio",
                "Precio unitario",
                Some("Cliente"),
            ),
            (
                "GG",
                "Factura de Garantía",
                "Costo unitario",
                Some("Cliente"),
            ),
            (
                "GI",
                "Factura de Insumos",
                "Costo unitario",
                Some("Cliente"),
            ),
            ("P", "Factura de Pérdida", "Costo unitario", None),
        ] {
            let model = build_invoice_document(&detail(invoice_type), &company(false));
            assert_eq!(model.title, title);
            assert_eq!(model.price_label, price);
            assert_eq!(model.contact.as_ref().map(|c| c.label.as_str()), contact);
            assert_eq!(model.date, "04/11/2026");
            assert_eq!(model.lines[0].unit, "m");
            assert_eq!(model.total_cents, 6_000);
            let json = serde_json::to_string(&model).unwrap();
            assert!(
                !json.contains("pending") && !json.contains("status"),
                "{json}"
            );
        }
        let sale = build_invoice_document(&detail("V"), &company(false));
        assert_eq!(sale.reserved_until.as_deref(), Some("10/11/2026"));
        assert_eq!(sale.contact.unwrap().company, None);
        let guarantee = build_invoice_document(&detail("GG"), &company(false));
        assert_eq!(guarantee.associated.as_deref(), Some("S-00003 (Servicio)"));
        assert_eq!(
            build_invoice_document(&detail("E"), &company(false)).notes_label,
            "Observaciones"
        );
    }

    fn pdf_text(bytes: &[u8]) -> String {
        use printpdf::lopdf::{content::Content, Document, Object};
        let document = Document::load_mem(bytes).unwrap();
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
        text
    }

    /// JPEG mínimo con un SOF0 de 4 × 2 px en color.
    fn jpeg() -> Vec<u8> {
        let mut bytes = vec![
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x04, 0x03,
        ];
        bytes.extend_from_slice(&[0; 9]);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn the_pdf_has_the_header_contact_lines_and_total_and_embeds_the_jpeg_logo() {
        let model = build_invoice_document(&detail("V"), &company(true));
        let bytes = invoice_pdf_bytes(&model, Some(&jpeg())).unwrap();
        let text = pdf_text(&bytes);
        for expected in [
            "Goyma Conexiones",
            "Teléfono: +53 7 8300000",
            "Factura de Venta",
            "No. V-00007    Fecha: 04/11/2026",
            "Reservada hasta: 10/11/2026",
            "Cliente: Rosa Díaz",
            "CI: 90010112345",
            "Cable UTP",
            "2,50 USD",
            "Total: 60,00 USD",
            "Entrega en tienda",
        ] {
            assert!(text.contains(expected), "falta «{expected}» en:\n{text}");
        }
        assert!(!text.contains("Por Cobrar") && !text.contains("Cobrada"));
        assert!(
            String::from_utf8_lossy(&bytes).contains("/DCTDecode"),
            "el logo se incrusta como JPEG"
        );

        // Con algo que no es JPEG como logo, el PDF se genera sin imagen.
        let without = invoice_pdf_bytes(&model, Some(b"no es jpeg")).unwrap();
        assert!(!String::from_utf8_lossy(&without).contains("/DCTDecode"));
    }

    #[test]
    fn a_failed_write_is_export_failed_and_leaves_no_file() {
        let model = build_invoice_document(&detail("E"), &company(false));
        let missing = std::env::temp_dir()
            .join(format!("goyma-no-existe-{}", uuid::Uuid::new_v4()))
            .join("factura.pdf");
        let error = export_invoice_pdf(&model, None, &missing).unwrap_err();
        assert_eq!(error.code, "exportFailed");
        assert_eq!(error.message, "No se pudo guardar el archivo");
        assert!(!missing.exists());

        let dir = std::env::temp_dir().join(format!("goyma-pdf-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("factura.pdf");
        export_invoice_pdf(&model, None, &destination).unwrap();
        assert!(std::fs::read(&destination).unwrap().starts_with(b"%PDF"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }
}
