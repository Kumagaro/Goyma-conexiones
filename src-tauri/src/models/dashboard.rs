use serde::{Deserialize, Serialize};

/// Período global del dashboard (DB-01): `today`, `week`, `month`, `3m`,
/// `6m`, `1y` o `custom` con sus dos fechas locales.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DashboardPeriod {
    pub preset: String,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
}

/// Una tarjeta: valor del período, el del período anterior de igual duración
/// y la serie del minigráfico. Sin historial (`previous: None`, serie vacía)
/// en lo que solo se conoce hoy, como el valor del inventario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiSeries {
    pub value: i64,
    pub previous: Option<i64>,
    pub trend: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeBucket {
    pub label: String,
    pub sales_cents: i64,
    pub services_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestSeller {
    pub name: String,
    pub quantity: String,
    pub margin_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequentClient {
    pub name: String,
    pub purchases: i64,
    pub total_cents: i64,
}

/// Elemento del Pareto 80/20 con su porcentaje entero del total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarItem {
    pub name: String,
    pub percent: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowMover {
    pub name: String,
    pub sold: String,
    pub selling_price_cents: i64,
    pub days_without_sale: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiringReservation {
    pub id: i64,
    pub number: String,
    pub invoice_type: String,
    pub contact_name: String,
    pub reserved_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationsSummary {
    pub count: i64,
    pub amount_cents: i64,
    pub expiring: Vec<ExpiringReservation>,
}

/// Todas las cifras de DB-02 a DB-07 en centavos de USD, salvo donde el
/// nombre indique otra escala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardKpis {
    pub from_date: String,
    pub to_date: String,
    pub income: KpiSeries,
    pub sales: KpiSeries,
    pub daily_sales_milli: KpiSeries,
    pub daily_sales_amount_cents: i64,
    pub net_profit: KpiSeries,
    pub margin_hundredths: KpiSeries,
    pub gross_margin_cents: i64,
    pub average_profit_hundredths: i64,
    pub expenses: KpiSeries,
    pub inventory_value: KpiSeries,
    pub out_of_stock: KpiSeries,
    pub inventory_quantity: String,
    pub buckets: Vec<IncomeBucket>,
    pub guarantee_cents: i64,
    pub installation_cents: i64,
    pub loss_cents: i64,
    pub best_sellers: Vec<BestSeller>,
    pub frequent_clients: Vec<FrequentClient>,
    pub star_products: Vec<StarItem>,
    pub star_clients: Vec<StarItem>,
    pub slow_movers: Vec<SlowMover>,
    pub reservations: ReservationsSummary,
}
