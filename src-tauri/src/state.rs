use crate::errors::{invalid_state, AppError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceStatus {
    Pending,
    Completed,
    Cancelled,
}

impl InvoiceStatus {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Result<Self, AppError> {
        match value {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(invalid_state(
                "El estado de la factura no es válido",
                "unknown invoice status",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceType {
    Entry,
    Sale,
    Service,
    Guarantee,
    Installation,
    Loss,
}

impl InvoiceType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Result<Self, AppError> {
        match value {
            "E" => Ok(Self::Entry),
            "V" => Ok(Self::Sale),
            "S" => Ok(Self::Service),
            "GG" => Ok(Self::Guarantee),
            "GI" => Ok(Self::Installation),
            "P" => Ok(Self::Loss),
            _ => Err(invalid_state(
                "El tipo de factura no es válido",
                "unknown invoice type",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entry => "E",
            Self::Sale => "V",
            Self::Service => "S",
            Self::Guarantee => "GG",
            Self::Installation => "GI",
            Self::Loss => "P",
        }
    }
}

pub fn validate_transition(
    invoice_type: InvoiceType,
    from: InvoiceStatus,
    to: InvoiceStatus,
) -> Result<(), AppError> {
    if from == to {
        return Err(invalid_state(
            "La factura ya está en ese estado",
            "invoice status did not change",
        ));
    }
    if from == InvoiceStatus::Cancelled {
        return Err(invalid_state(
            "Una factura cancelada no se puede reactivar ni editar",
            "cancelled invoice transition attempted",
        ));
    }

    let allowed = matches!(
        (invoice_type, from, to),
        (_, InvoiceStatus::Pending, InvoiceStatus::Cancelled)
            | (_, InvoiceStatus::Completed, InvoiceStatus::Cancelled)
            | (
                InvoiceType::Entry | InvoiceType::Sale | InvoiceType::Service,
                InvoiceStatus::Pending,
                InvoiceStatus::Completed
            )
            | (
                InvoiceType::Entry | InvoiceType::Sale | InvoiceType::Service,
                InvoiceStatus::Completed,
                InvoiceStatus::Pending
            )
            | (
                InvoiceType::Guarantee | InvoiceType::Installation,
                InvoiceStatus::Pending,
                InvoiceStatus::Completed
            )
    );

    if allowed {
        Ok(())
    } else {
        Err(invalid_state(
            "La transición de estado no es válida para este tipo de factura",
            "invoice status transition is not allowed",
        ))
    }
}

pub fn validate_edit_allowed(
    invoice_type: InvoiceType,
    status: InvoiceStatus,
) -> Result<(), AppError> {
    match (invoice_type, status) {
        (_, InvoiceStatus::Cancelled) => Err(invalid_state(
            "Una factura cancelada no se puede reactivar ni editar",
            "cancelled invoice edit attempted",
        )),
        (_, InvoiceStatus::Pending) => Ok(()),
        (InvoiceType::Entry, InvoiceStatus::Completed) => Err(invalid_state(
            "Solo se pueden editar las facturas que están Por Pagar",
            "completed entry invoice edit attempted",
        )),
        (InvoiceType::Sale | InvoiceType::Service, InvoiceStatus::Completed) => Err(invalid_state(
            "Solo se pueden editar las facturas que están Por Cobrar",
            "completed outgoing invoice edit attempted",
        )),
        (InvoiceType::Guarantee | InvoiceType::Installation, InvoiceStatus::Completed) => {
            Err(invalid_state(
                "Solo se pueden editar las facturas que están Pendiente",
                "completed expense invoice edit attempted",
            ))
        }
        (InvoiceType::Loss, _) => Err(invalid_state(
            "Las facturas de pérdida no se pueden editar",
            "loss invoice edit attempted",
        )),
    }
}

pub fn validate_status_payload(
    invoice_type: InvoiceType,
    status: InvoiceStatus,
    payment_method: Option<&str>,
    paid_at: Option<&str>,
) -> Result<(), AppError> {
    let supports_payment = matches!(
        invoice_type,
        InvoiceType::Entry | InvoiceType::Sale | InvoiceType::Service
    );

    if payment_method.is_some()
        && (!supports_payment || status == InvoiceStatus::Pending || paid_at.is_none())
    {
        return Err(invalid_state(
            "El método de pago no es válido para este estado de factura",
            "payment method is incompatible with invoice status",
        ));
    }

    if paid_at.is_some() && (!supports_payment || status == InvoiceStatus::Pending) {
        return Err(invalid_state(
            "La fecha de pago no es válida para este estado de factura",
            "payment timestamp is incompatible with invoice status",
        ));
    }

    if supports_payment && status == InvoiceStatus::Completed && paid_at.is_none() {
        return Err(invalid_state(
            "La fecha de pago o cobro es obligatoria",
            "completed invoice is missing its payment timestamp",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        validate_edit_allowed, validate_status_payload, validate_transition, InvoiceStatus,
        InvoiceType,
    };

    #[test]
    fn invoice_state_types_parse_and_round_trip() {
        for value in ["pending", "completed", "cancelled"] {
            assert_eq!(InvoiceStatus::from_str(value).unwrap().as_str(), value);
        }
        for value in ["E", "V", "S", "GG", "GI", "P"] {
            assert_eq!(InvoiceType::from_str(value).unwrap().as_str(), value);
        }
        assert!(InvoiceStatus::from_str("x").is_err());
        assert!(InvoiceType::from_str("X").is_err());
    }

    #[test]
    fn invoice_state_transitions_follow_spec03_matrix() {
        use InvoiceStatus::{Cancelled, Completed, Pending};
        use InvoiceType::{Entry, Guarantee, Installation, Sale, Service};
        for invoice_type in [Entry, Sale, Service] {
            assert!(validate_transition(invoice_type, Pending, Completed).is_ok());
            assert!(validate_transition(invoice_type, Completed, Pending).is_ok());
            assert!(validate_transition(invoice_type, Pending, Cancelled).is_ok());
            assert!(validate_transition(invoice_type, Completed, Cancelled).is_ok());
        }
        for invoice_type in [Guarantee, Installation] {
            assert!(validate_transition(invoice_type, Pending, Completed).is_ok());
            assert!(validate_transition(invoice_type, Pending, Cancelled).is_ok());
            assert!(validate_transition(invoice_type, Completed, Cancelled).is_ok());
            assert_eq!(
                validate_transition(invoice_type, Completed, Pending)
                    .unwrap_err()
                    .code,
                "invalidState"
            );
        }
        assert_eq!(
            validate_transition(Entry, Pending, Pending)
                .unwrap_err()
                .message,
            "La factura ya está en ese estado"
        );
        assert_eq!(
            validate_transition(Sale, Cancelled, Pending)
                .unwrap_err()
                .message,
            "Una factura cancelada no se puede reactivar ni editar"
        );
    }

    #[test]
    fn only_pending_invoices_can_be_edited_with_type_specific_messages() {
        use InvoiceStatus::{Cancelled, Completed, Pending};
        use InvoiceType::{Entry, Guarantee, Installation, Sale, Service};
        assert!(validate_edit_allowed(Entry, Pending).is_ok());
        assert!(validate_edit_allowed(Sale, Pending).is_ok());
        assert!(validate_edit_allowed(Service, Pending).is_ok());
        assert!(validate_edit_allowed(Guarantee, Pending).is_ok());
        assert!(validate_edit_allowed(Installation, Pending).is_ok());
        assert_eq!(
            validate_edit_allowed(Entry, Completed).unwrap_err().message,
            "Solo se pueden editar las facturas que están Por Pagar"
        );
        assert_eq!(
            validate_edit_allowed(Sale, Completed).unwrap_err().message,
            "Solo se pueden editar las facturas que están Por Cobrar"
        );
        assert_eq!(
            validate_edit_allowed(Guarantee, Completed)
                .unwrap_err()
                .message,
            "Solo se pueden editar las facturas que están Pendiente"
        );
        assert!(validate_edit_allowed(Entry, Cancelled).is_err());
    }

    #[test]
    fn invoice_payment_payload_matches_status_and_type() {
        use InvoiceStatus::{Cancelled, Completed, Pending};
        use InvoiceType::{Entry, Guarantee, Sale};
        assert!(validate_status_payload(Entry, Pending, None, None).is_ok());
        assert!(validate_status_payload(
            Sale,
            Completed,
            Some("cash"),
            Some("2026-10-06T12:00:00Z")
        )
        .is_ok());
        assert!(
            validate_status_payload(Entry, Completed, None, Some("2026-10-06T12:00:00Z")).is_ok()
        );
        assert_eq!(
            validate_status_payload(Entry, Pending, Some("cash"), None)
                .unwrap_err()
                .code,
            "invalidState"
        );
        assert_eq!(
            validate_status_payload(Entry, Completed, None, None)
                .unwrap_err()
                .code,
            "invalidState"
        );
        assert_eq!(
            validate_status_payload(Guarantee, Completed, Some("cash"), None)
                .unwrap_err()
                .code,
            "invalidState"
        );
        assert!(validate_status_payload(
            Entry,
            Cancelled,
            Some("cash"),
            Some("2026-10-06T12:00:00Z")
        )
        .is_ok());
    }
}
