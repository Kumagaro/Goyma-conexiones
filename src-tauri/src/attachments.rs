use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use uuid::Uuid;

use crate::errors::{append_error_log, AppError};

const MAX_ATTACHMENT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAttachment {
    pub file_name: String,
    pub original_name: String,
    pub size_bytes: u64,
    pub uploaded_at: String,
}

pub fn validate_pdf_header(path: &Path) -> Result<(), AppError> {
    let mut file = fs::File::open(path).map_err(|_| {
        AppError::new(
            "validationFailed",
            "El archivo debe ser un PDF de hasta 5 MB",
            "cannot open attachment",
        )
    })?;
    let mut header = [0_u8; 5];
    use std::io::Read;
    file.read_exact(&mut header).map_err(|_| {
        AppError::new(
            "validationFailed",
            "El archivo debe ser un PDF de hasta 5 MB",
            "attachment has no PDF header",
        )
    })?;
    if &header != b"%PDF-" {
        return Err(AppError::new(
            "validationFailed",
            "El archivo debe ser un PDF de hasta 5 MB",
            "attachment signature is not PDF",
        ));
    }
    Ok(())
}

pub fn validate_attachment_size(path: &Path) -> Result<u64, AppError> {
    let size = fs::metadata(path)
        .map_err(|_| {
            AppError::new(
                "validationFailed",
                "El archivo debe ser un PDF de hasta 5 MB",
                "cannot read attachment metadata",
            )
        })?
        .len();
    if size == 0 || size > MAX_ATTACHMENT_BYTES {
        return Err(AppError::new(
            "validationFailed",
            "El archivo debe ser un PDF de hasta 5 MB",
            "attachment size is outside supported range",
        ));
    }
    Ok(size)
}

pub fn validate_original_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > 100
        || trimmed.contains(['/', '\\'])
        || trimmed.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "validationFailed",
            "El nombre del archivo adjunto no es válido",
            "invalid original attachment name",
        ));
    }
    Ok(())
}

pub fn store_attachment(app_data_dir: &Path, source: &Path) -> Result<StoredAttachment, AppError> {
    validate_pdf_header(source)?;
    let size_bytes = validate_attachment_size(source)?;
    let original_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "El nombre del archivo adjunto no es válido",
                "attachment name is not valid utf-8",
            )
        })?;
    validate_original_name(original_name)?;

    let attachment_dir = app_data_dir.join("attachments");
    fs::create_dir_all(&attachment_dir)
        .map_err(|_| internal_error("failed to create attachment directory"))?;
    let file_name = format!("{}.pdf", Uuid::new_v4());
    fs::copy(source, attachment_dir.join(&file_name))
        .map_err(|_| internal_error("failed to copy attachment"))?;
    Ok(StoredAttachment {
        file_name,
        original_name: original_name.to_owned(),
        size_bytes,
        uploaded_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

pub fn delete_attachment(app_data_dir: &Path, file_name: &str) -> Result<(), AppError> {
    let path = safe_attachment_path(app_data_dir, file_name)?;
    if let Err(error) = fs::remove_file(path) {
        append_error_log(app_data_dir, &format!("attachment delete failed: {error}"));
    }
    Ok(())
}

pub fn sweep_orphans(app_data_dir: &Path, connection: &Connection) -> Result<usize, AppError> {
    let directory = app_data_dir.join("attachments");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            append_error_log(
                app_data_dir,
                &format!("attachment sweep listing failed: {error}"),
            );
            return Ok(0);
        }
    };
    let referenced: Vec<String> = match connection
        .prepare("SELECT attachmentFileName FROM invoices WHERE attachmentFileName IS NOT NULL")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()
        }) {
        Ok(names) => names,
        Err(error) => {
            append_error_log(
                app_data_dir,
                &format!("attachment sweep query failed: {error}"),
            );
            return Ok(0);
        }
    };
    let referenced: std::collections::HashSet<String> = referenced.into_iter().collect();
    let mut deleted = 0;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                append_error_log(
                    app_data_dir,
                    &format!("attachment sweep entry failed: {error}"),
                );
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_orphan = entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
            && name.ends_with(".pdf")
            && !referenced.contains(&name);
        if is_orphan {
            match fs::remove_file(entry.path()) {
                Ok(()) => deleted += 1,
                Err(error) => append_error_log(
                    app_data_dir,
                    &format!("attachment orphan delete failed: {error}"),
                ),
            }
        }
    }
    Ok(deleted)
}

pub(crate) fn attachment_path(app_data_dir: &Path, file_name: &str) -> Result<PathBuf, AppError> {
    safe_attachment_path(app_data_dir, file_name)
}

fn safe_attachment_path(app_data_dir: &Path, file_name: &str) -> Result<PathBuf, AppError> {
    validate_original_name(file_name)?;
    if file_name.contains(['/', '\\'])
        || Path::new(file_name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(file_name)
    {
        return Err(AppError::new(
            "validationFailed",
            "El nombre del archivo adjunto no es válido",
            "attachment path traversal rejected",
        ));
    }
    Ok(app_data_dir.join("attachments").join(file_name))
}

fn internal_error(cause: &str) -> AppError {
    AppError::new("internal", "Ocurrió un error inesperado", cause)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::test_utils::setup_test_db;

    use super::{
        delete_attachment, store_attachment, sweep_orphans, validate_attachment_size,
        validate_original_name, validate_pdf_header,
    };

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "goyma-attachment-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn pdf_validation_checks_signature_and_size() {
        let dir = temp_dir("validate");
        fs::create_dir_all(&dir).unwrap();
        let valid = dir.join("valid.pdf");
        fs::write(&valid, b"%PDF-1.7 test").unwrap();
        assert!(validate_pdf_header(&valid).is_ok());
        assert!(validate_attachment_size(&valid).is_ok());

        let text = dir.join("text.pdf");
        fs::write(&text, b"not a pdf").unwrap();
        assert_eq!(
            validate_pdf_header(&text).unwrap_err().code,
            "validationFailed"
        );
        let empty = dir.join("empty.pdf");
        fs::write(&empty, []).unwrap();
        assert_eq!(
            validate_pdf_header(&empty).unwrap_err().code,
            "validationFailed"
        );

        let large = dir.join("large.pdf");
        fs::File::create(&large)
            .unwrap()
            .set_len(5 * 1024 * 1024 + 1)
            .unwrap();
        assert_eq!(
            validate_attachment_size(&large).unwrap_err().code,
            "validationFailed"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn original_attachment_names_reject_paths_empty_and_overlong_names() {
        assert!(validate_original_name("factura.pdf").is_ok());
        for name in [
            "",
            "folder/factura.pdf",
            "folder\\factura.pdf",
            &"x".repeat(101),
        ] {
            assert_eq!(
                validate_original_name(name).unwrap_err().code,
                "validationFailed"
            );
        }
    }

    #[test]
    fn store_delete_and_sweep_manage_uuid_files_without_deleting_references() {
        let app_dir = temp_dir("storage");
        fs::create_dir_all(&app_dir).unwrap();
        let source = app_dir.join("Factura Enero.pdf");
        fs::write(&source, b"%PDF-1.7 content").unwrap();
        let stored = store_attachment(&app_dir, &source).unwrap();
        assert!(stored.file_name.ends_with(".pdf"));
        assert_ne!(stored.file_name, "Factura Enero.pdf");
        assert_eq!(stored.original_name, "Factura Enero.pdf");
        assert_eq!(
            fs::read(app_dir.join("attachments").join(&stored.file_name)).unwrap(),
            b"%PDF-1.7 content"
        );

        let attachment_dir = app_dir.join("attachments");
        let referenced = attachment_dir.join("referenced.pdf");
        let orphan = attachment_dir.join("orphan.pdf");
        fs::write(&referenced, b"%PDF-ref").unwrap();
        fs::write(&orphan, b"%PDF-orphan").unwrap();
        let connection = setup_test_db();
        connection.execute(
            "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
             VALUES ('Juan', 'Pérez', '12345678901', '+5355551234', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            [],
        ).unwrap();
        let supplier_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName, contactFullNameNormalized,
                contactNationalId, contactPhone, createdByName, attachmentFileName, attachmentOriginalName,
                attachmentSizeBytes, attachmentUploadedAt, createdAt, updatedAt)
             VALUES ('E', 'E-00001', '2026-10-03', 'pending', ?1, 'Juan Pérez', 'juan perez', '12345678901', '+5355551234',
                'admin', 'referenced.pdf', 'Factura.pdf', 8, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            [supplier_id],
        ).unwrap();
        assert_eq!(sweep_orphans(&app_dir, &connection).unwrap(), 2);
        assert!(referenced.exists());
        assert!(!orphan.exists());
        assert!(!app_dir.join("attachments").join(&stored.file_name).exists());

        delete_attachment(&app_dir, &stored.file_name).unwrap();
        assert!(!app_dir.join("attachments").join(&stored.file_name).exists());
        assert!(delete_attachment(&app_dir, "missing.pdf").is_ok());
        fs::remove_dir_all(app_dir).unwrap();
    }

    #[test]
    fn sweep_orphans_removes_unreferenced_uuid_named_uploads() {
        let app_dir = temp_dir("uuid-orphan");
        let attachment_dir = app_dir.join("attachments");
        fs::create_dir_all(&attachment_dir).unwrap();
        let orphan = attachment_dir.join("550e8400-e29b-41d4-a716-446655440000.pdf");
        fs::write(&orphan, b"%PDF-orphan").unwrap();
        let connection = setup_test_db();

        assert_eq!(sweep_orphans(&app_dir, &connection).unwrap(), 1);
        assert!(!orphan.exists());
        fs::remove_dir_all(app_dir).unwrap();
    }
}
