# Plan: Cierre del MVP — Notificaciones, Dashboard, Configuración, Impresión, Borradores y Respaldo

- **Feature:** `cierre-del-mvp`
- **Basado en:** `spec-04.md` (esta misma carpeta), `Constitution.md` v1.2.0 → v1.3.0, `Agents.md`, `plan-01` a `plan-03`, `pendientes.md` y el código real (backend Rust en `src-tauri/src`, frontend en `src/`).
- **Estado:** **Confirmado por el dueño (2026-10-28)**, junto con `spec-04` y sus supuestos S-01 a S-11. Las decisiones técnicas D-01 a D-16 (§10) están en vigor.
- **Nota:** reutiliza sin cambios la infraestructura de las specs anteriores: conexión compartida (`Mutex<Connection>`), `AppError`, `require_session`/`require_admin`, `setup_test_db`, el patrón `xxx_inner` con envoltorio delgado, `purge_best_effort`, la matriz de autorización, `combobox.js`, `confirmDialog.js`, `formDialog.js` y `utils/dateSort.js`.

---

## 1. Dependencias y configuración

| Dependencia | Para qué | Justificación |
|---|---|---|
| `csv` 1.x (sin `serde` derive obligatorio) | Leer y escribir CSV (CSV-01, CSV-02) | Comillas, separadores dentro de campos y saltos de línea hacen frágil un parser propio; el crate es Rust puro, pequeño y maduro. Se configura con `;` y se escribe el BOM a mano. |
| ~~`image`~~ **descartada en el spike de §0 (D-17)** | — | `printpdf` 0.7 incrusta un JPEG tal cual (`ImageXObject` con `ImageFilter::DCT`) sin la *feature* `embedded_images`, que además arrastraría `image` 0.24 con ocho formatos. |
| `printpdf` 0.7 (existente, sin *features* nuevas) | PDF formal de facturas (IM-02) | Ya se usa para el historial (`plan-03` §3.10, D-18). |
| `tauri-plugin-dialog` (existente) | "Guardar como" (PDF, CSV, copia) y "Abrir" (logo, CSV, copia) | Ya se usa para adjuntos y exportación del historial. |

- **Se retira jsPDF** (`src/js/libs/jspdf.umd.min.js`) y su `<script>`: lo sustituye IM-02.
- **Sin dependencias nuevas en el frontend:** toasts, vista de impresión y borradores son JS/CSS propios.
- **CSP** (SG-01, `tauri.conf.json`): `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'`. Se admite `'unsafe-inline'` **solo en estilos** porque los `style=` del mockup no ejecutan código y reescribirlos todos no aporta seguridad; los scripts quedan sin `unsafe-inline` ni `unsafe-eval` (D-01).

## 2. Esquema de base de datos (migraciones 2 a 8)

Todas las tablas nuevas siguen las reglas de §4 de la Constitution (restricciones en el esquema, `camelCase`, `createdAt`/`updatedAt` con el mismo `CHECK` de formato UTC que `plan-03`).

### 2.1 Versionado (MG-01)

- La versión vive en `PRAGMA user_version`; la identidad de la base, en `PRAGMA application_id` (constante propia de Goyma), para reconocer una copia válida (RP-03).
- `db/migrations.rs`: lista ordenada de migraciones `(versión, fn(&Transaction) -> rusqlite::Result<()>)`. `run_migrations(conn)` aplica las pendientes, cada una en su transacción, y sube `user_version` al final de cada una.
- **Versión 1 = esquema actual** (`init_schema` de las specs 1 a 3). Una base con `user_version = 0` y la tabla `users` se marca como versión 1 sin tocarla; una base vacía se crea con `init_schema` y luego migra. `init_schema` deja de cambiarse: todo cambio futuro es una migración.
- Una base con `user_version` mayor que la última conocida → `AppError` `schemaTooNew` y la app no arranca su lógica (pantalla de error).

### 2.2 Migración 2 — Notificaciones (NT)

- `notifications` gana: `entityType TEXT CHECK (entityType IN ('product','invoice'))`, `entityId INTEGER`, `audience TEXT NOT NULL DEFAULT 'all' CHECK (audience IN ('all','admin'))`, `CHECK ((entityType IS NULL) = (entityId IS NULL))`, y `priority` con `CHECK (priority IN ('CRÍTICO','ADVERTENCIA','INFORMATIVO'))` (los valores actuales ya cumplen). Índice `(isRead, createdAt)`.
- `notificationWatches`: `(kind TEXT CHECK (kind IN ('N-02','N-03','N-04','N-05','N-07','N-10','N-11')), entityKey TEXT, createdAt, PRIMARY KEY (kind, entityKey))`. Una fila = "esta situación ya avisó" (NT-03). Al salir de la situación se borra la fila.

### 2.3 Migración 3 — Reservas (RS)

- `invoices.reservedUntil TEXT` con `CHECK (reservedUntil IS NULL OR (type IN ('V','S') AND date(reservedUntil) IS reservedUntil AND reservedUntil > invoiceDate))`.
- Índice parcial `invoicesActiveReservations ON invoices (reservedUntil) WHERE reservedUntil IS NOT NULL AND status = 'pending'`.

### 2.4 Migración 4 — Configuración (CF)

- `settings`: `(key TEXT PRIMARY KEY CHECK (key IN ('lowStockThreshold','staleInvoiceDays','backupReminderDays','companyName','companyAddress','companyPhone','companyTaxId','companyLogoFile','lastBackupAt')), value TEXT NOT NULL, updatedAt)`. Los valores numéricos se validan en Rust y con `CHECK` por clave (`CASE key WHEN 'lowStockThreshold' THEN CAST(value AS INTEGER) BETWEEN 1 AND 9999 …`). Se siembran los valores por defecto. Las copias automáticas (RP-02) no tienen tabla: son archivos en `app_data_dir/backups/goyma-AAAA-MM-DD.sqlite3` y la fecha de la última copia correcta vive en `lastBackupAt`.
- `userPreferences`: `(userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE, key TEXT NOT NULL CHECK (key IN ('visibleCurrencies','pageSize')), value TEXT NOT NULL, updatedAt, PRIMARY KEY (userId, key))`.
- `exchangeRates`: `(currency TEXT PRIMARY KEY CHECK (currency IN ('EUR','CUP','MLC','SALDO')), rateMicro INTEGER NOT NULL CHECK (rateMicro BETWEEN 1 AND 999999999999999), updatedAt, updatedByName TEXT NOT NULL)`.
- `exchangeRateHistory`: `(id, createdAt, currency, previousRateMicro INTEGER NULL, newRateMicro INTEGER NOT NULL, userId NULL REFERENCES users ON DELETE SET NULL, userName TEXT NOT NULL)`, con triggers de inmutabilidad como `productMovements`.

### 2.5 Migración 5 — Unidad por línea (CV)

- `invoiceLines` gana: `enteredUnitId INTEGER REFERENCES units(id) ON DELETE SET NULL`, `enteredUnitName TEXT`, `enteredUnitAbbreviation TEXT`, `enteredQuantityMilli INTEGER`, `conversionFactorMicro INTEGER`. `CHECK ((enteredQuantityMilli IS NULL) = (conversionFactorMicro IS NULL))`. Las líneas existentes quedan con `NULL` (= en la unidad del producto).

### 2.6 Migración 6 — Borradores (BR)

- `drafts`: `(id, userId NOT NULL REFERENCES users ON DELETE CASCADE, formKey TEXT NOT NULL CHECK (length(formKey) BETWEEN 1 AND 60), recordId INTEGER, payload TEXT NOT NULL CHECK (length(payload) <= 200000), createdAt, updatedAt)`, con `UNIQUE (userId, formKey, recordId)` (índice único con `COALESCE(recordId, 0)` para tratar "nuevo" como un solo registro).

### 2.7 Migración 7 — Registro de actividad (RA)

- `activityLog`: `(id, createdAt, userId NULL REFERENCES users ON DELETE SET NULL, userName TEXT NOT NULL, action TEXT NOT NULL CHECK (action IN ('delete','cancel','edit','config','backup','restore','import','purge','expire')), entityType TEXT NOT NULL CHECK (entityType IN ('product','category','unit','conversion','client','supplier','user','invoice','settings','exchangeRate','backup','csv')), entityLabel TEXT NOT NULL, details TEXT)`. Triggers `activityLogImmutable` y `activityLogNoDelete`. Índice `(createdAt)`.

### 2.8 Migración 8 — Identidad de la base

- Fija `PRAGMA application_id` (no transaccional: se ejecuta fuera de la transacción, idempotente).

## 3. Reglas transversales en código

### 3.1 Procesos automáticos (`automation.rs`, nuevo)

- `run_startup_processes(conn, now, today, app_data_dir)` en el `setup` de Tauri, después de `run_migrations`: copia diaria → purga de vencidos (con N-12 y RA `purge`) → `expire_reservations` → `evaluate_all_watches` → purga de notificaciones (> 90 días) y borradores (> 30 días). Cada paso es best-effort: un fallo se registra y el siguiente sigue.
- `before_operation(tx, now, today)`: se llama donde hoy se llama `purge_best_effort` (`spec-02` §7.6) y añade `expire_reservations`. Mantiene el contrato de `expiredCleanupWarning`.
- `after_inventory_change(tx, now, today, product_ids)`: al final de cada transacción que cambie lotes o caducidades (los `create/update/delete/changeStatus` de facturas, `createProduct`, `updateProduct`, activar/descontinuar), reevalúa N-02/N-03/N-04/N-07 **solo** para esos productos. Los cambios de umbrales llaman a `evaluate_all_watches`.

### 3.2 Autor de un movimiento o de una entrada del registro (D-02)

- Hoy `insert_reversal_movement` y similares reciben `&Actor`. Para el vencimiento automático de reservas y la purga se introduce `enum Author<'a> { User(&'a Actor), System }`; `System` escribe `userId = NULL` y `userName = 'Sistema'`. **`Actor` y `AdminActor` siguen obteniéndose solo con `require_session`/`require_admin`** (`Agents.md` §3): `Author::System` no da permiso para mutar `users` ni `sessions`.

### 3.3 Notificaciones (`db/notifications.rs`, ampliación)

- `notify(tx, kind, priority, message, entity, audience)` y `watch_enter(tx, kind, key) -> bool` / `watch_leave(tx, kind, key)`: un aviso de situación solo se inserta si `watch_enter` devuelve `true` (no existía la fila).
- Claves: producto → `p:{id}`; factura → `i:{id}`; respaldo → `backup`.
- N-02 "al intentar vender" se inserta en una **transacción aparte** después del rollback de la venta rechazada, para que el `stockInsuficiente` no se lo lleve.

### 3.4 Reservas (`commands/invoices.rs`, ampliación)

- `reservedUntil` opcional en `CreateSaleInvoiceInput`, `CreateServiceInvoiceInput` y sus `Update*`. Validado en Rust (fecha válida, > emisión, ≤ emisión + 365 días, estado `pending`).
- `expire_reservations(tx, now, today)`: selecciona `status = 'pending' AND reservedUntil < today`, y para cada una reutiliza el camino de `change_invoice_status` a `cancelled` con `Author::System`, `N-06` y RA `expire`.
- `listInvoices` admite `status = 'reserved'` (activas); `InvoiceListItem` y `InvoiceDetail` ganan `reservedUntil`. `ProductDetail` gana `reservedQuantity` (suma de `quantityMilli` de líneas en reservas activas).

### 3.5 Conversiones (`inventory.rs` + `commands/invoices.rs`)

- Las líneas de entrada (`EntryInvoiceLineInput`, `UpdateEntryInvoiceLineInput`), de salida y de pérdida ganan `unitId` opcional. Si difiere de la unidad del producto, se busca la conversión directa o inversa (`spec-02` UM-06) y se calcula `quantityMilli = round_half_up(enteredQuantityMilli × factorMicro / 1_000_000)`; sin conversión → `validationFailed` "No hay una conversión entre esas unidades".
- Compras: `unitPrice` escrito por la unidad elegida → costo del lote por unidad del producto = `round_half_up(unitCents × 1_000_000 / factorMicro)`. Ventas: precio de la línea = precio vigente × factor. El subtotal se calcula con la cantidad escrita × el precio escrito (lo que se ve), y el historial guarda la cantidad convertida.
- Comando nuevo `listUnitOptionsForProduct(productId)` → `[{unitId, name, abbreviation, factor: string}]` (la del producto con factor 1).

### 3.6 Dashboard (`commands/dashboard.rs`, ampliación)

- `getDashboardKpis(period)` con `period = { preset: 'today'|'week'|'month'|'3m'|'6m'|'1y'|'custom', fromDate?, toDate? }` → un DTO `DashboardKpis` con todas las cifras de DB-02 a DB-07, en centavos y milésimas. Rangos en hora local, convertidos a UTC para `paidAt` (`timestamps.rs`).
- Costo de lo vendido: `SUM(consumptions.quantityMilli × unitCostCents / 1000)` de líneas de ventas y servicios cobrados en el período, con un único redondeo por línea como `line_unit_cost_cents`.
- `getDashboardSummary` (DASH-01) se conserva y usa el umbral de `settings`.
- Los equivalentes en otras monedas los calcula el frontend con las tasas de `listExchangeRates` (solo presentación, CF-01).

### 3.7 Impresión y PDF (`invoice_print.rs`, nuevo)

- `build_invoice_document(detail, company) -> InvoiceDocument`: modelo intermedio (títulos por tipo, encabezado, filas, totales) que usan **los dos** caminos: el PDF de `printpdf` y la vista previa HTML (vía el comando `getInvoicePrintModel`), para que impresión y PDF no diverjan (D-05).
- `exportInvoicePdf(invoiceId, destinationPath)`: escribe el PDF A4 con Helvetica integrada y el logo JPEG incrustado sin decodificar (D-17). Fallo de escritura → `exportFailed`.
- Vista previa: `views/printPreview.js` monta el documento en `#print-area` y llama a `window.print()`; `css/print.css` con `@media print` oculta todo salvo `#print-area`. El logo llega como `data:` URL desde `getCompanyProfile`.

### 3.8 Borradores (`commands/drafts.rs`)

- `saveDraft({formKey, recordId?, payload})` hace `UPSERT`; `getDraft(formKey, recordId?)`; `listDrafts()`; `deleteDraft(formKey, recordId?)`. Todos con `require_session`, filtrando por el usuario de la sesión.
- Rust valida que `payload` sea JSON y que `formKey` esté en una lista cerrada (`DRAFT_FORMS`), y rechaza cualquier payload con claves de contraseña (`password`, `newPassword`, `currentPassword`, `confirmPassword`) como defensa en profundidad.

### 3.9 Registro de actividad (`activity.rs`, nuevo)

- `record_activity(tx, author, action, entity_type, entity_label, details)` dentro de la misma transacción de la acción (spec §7.5). Se añade en: `delete*` de todas las entidades, `changeInvoiceStatus` a `cancelled`, `update*Invoice`, `update*Settings`/tasas/empresa/logo, `createBackup`, `restoreBackup`, `importCsv`, purga y vencimiento.
- `listActivityLog(filters)` con `require_admin`.

### 3.10 CSV (`csv_io.rs`, nuevo)

- `exportCsv(kind, filters, destinationPath)`: `kind ∈ {products, categories, units, clients, suppliers, invoices}`; reutiliza las consultas de los listados (`list_*_inner`) para respetar filtros; escribe BOM + `;` + decimales con coma; neutraliza fórmulas anteponiendo `'` a los campos que empiezan por `=`, `+`, `-`, `@`.
- `importCsv(kind, sourcePath)` → `ImportSummary { imported, skipped: [{row, reason}] }`. Cada fila llama al mismo `create_*_inner` que el formulario (validación autoritativa única), dentro de un `SAVEPOINT` por fila: una fila que falla se revierte sola y se anota; las válidas se confirman juntas. Máximo 5 000 filas.

### 3.11 Respaldo (`backup.rs`, nuevo)

- `createBackup(destinationPath)`: `VACUUM INTO ?1` (copia consistente sin dependencias). Actualiza `lastBackupAt` y RA `backup`.
- Copia diaria: igual a `app_data_dir/backups/`, borra las más antiguas que las 7 últimas.
- `restoreBackup({sourcePath | automaticFileName, password})`: verifica la contraseña del usuario (bcrypt), abre la copia en solo lectura y comprueba `application_id`, `user_version ≤ última` e `integrity_check`; hace una copia de lo actual; dentro del `Mutex` sustituye la conexión por una en memoria, reemplaza el archivo, reabre, ejecuta `run_migrations` y borra las sesiones; RA `restore` (en la base restaurada). Devuelve `{ sessionEnded: true }` y el frontend vuelve al login.

### 3.12 Toasts y mensajes (frontend)

- `components/toast.js`: `showToast({ type: 'success'|'info'|'warning'|'error'|'critical', title, message, action? })`. Región `#toast-region` (creada al cargar), `role="status"`/`role="alert"` según el tipo, duraciones de TO-01, pausa con `mouseenter`/`focusin`, máximo 4 visibles (S-01). Estilos en `css/toast.css` (archivo nuevo; no se toca `dashboard.css`, Constitution §2.V).
- `utils/feedback.js`: `notifySuccess`, `notifyError(error, fallback)` (traduce `AppError` con el mismo criterio que hoy: mensaje del backend si es de validación o negocio, genérico si es interno) y `setFieldError(input, message)` / `clearFieldError(input)` (texto bajo el campo + `aria-describedby` + `aria-invalid`).
- Cada vista sustituye su `showMessage`/`showFeedback`/`showStatus` por `feedback.js`; `showCleanupWarning` pasa a un toast de advertencia (se conserva su contrato con `expiredCleanupWarning`).
- **Notificaciones como toast** (TO-02): `services/notificationFeed.js` guarda el último id visto; `invoke.js` emite un evento `command:success` tras cada comando que no empieza por `get_`, `list_`, `search_` ni `count_`, y el feed llama a `listNotificationsSince(lastId)` y muestra un toast por cada nueva. Al iniciar sesión, un toast resumen si hay no leídas.

### 3.13 Borradores (frontend)

- `utils/drafts.js`: `attachDraft(form, { formKey, recordId, serialize?, restore? })` serializa los campos con `name`/`id` (sin `type=password` ni `type=file`), guarda con `debounce` de 1 s y expone `clear()` al guardar o cancelar. Los formularios con estado propio (líneas de factura, combobox) aportan `serialize`/`restore` (por ejemplo, `facturasView.js` serializa contacto, líneas y fecha).
- Al iniciar sesión: `listDrafts` → toast informativo con enlace a `views/borradoresView.js`.

## 4. Comandos Tauri

### 4.1 Índice de comandos nuevos (31; 76 + 31 = **107** en `generate_handler!`)

| Módulo | Comandos | Rol |
|---|---|---|
| `notifications.rs` (7) | `listNotifications`, `countUnreadNotifications`, `listNotificationsSince`, `markNotificationRead`, `markAllNotificationsRead`, `deleteNotification`, `deleteNotifications` | Ambos (filtran `audience='admin'` al Contador) |
| `settings.rs` (10) | `getSettings`, `updateSettings`, `getUserPreferences`, `updateUserPreferences`, `listExchangeRates`, `updateExchangeRate`, `listExchangeRateHistory`, `getCompanyProfile`, `updateCompanyProfile`, `saveCompanyLogo` | Ambos |
| `invoices.rs` (+2) | `exportInvoicePdf`, `getInvoicePrintModel` | Ambos |
| `products.rs` (+1) | `listUnitOptionsForProduct` | Ambos |
| `dashboard.rs` (+1) | `getDashboardKpis` | Ambos |
| `drafts.rs` (4) | `saveDraft`, `getDraft`, `listDrafts`, `deleteDraft` | Ambos (los propios) |
| `activity.rs` (1) | `listActivityLog` | **Solo Administrador** |
| `csv_io.rs` (2) | `exportCsv`, `importCsv` | Ambos |
| `backup.rs` (2) | `createBackup`, `listAutomaticBackups` | Ambos |
| `backup.rs` (+1) | `restoreBackup` | Ambos, con contraseña |

Cada uno lleva su fila en la matriz de autorización (`Agents.md` §3); `listActivityLog` además la fila "Contador → rechazado".

### 4.2 Comandos existentes que cambian

- **Facturas:** `create/update` de venta y servicio (`reservedUntil`); todas las líneas (`unitId`); `listInvoices` (`status='reserved'`); `getInvoice` (`reservedUntil`, unidad escrita por línea); `changeInvoiceStatus`, `deleteInvoice`, `update*Invoice` (registro de actividad y `after_inventory_change`).
- **Productos, categorías, unidades, conversiones, contactos:** los `delete*` registran actividad; `getProduct` gana `reservedQuantity`; `listProducts` usa el umbral de `settings` para "Stock bajo".
- **Usuarios:** `createUser`, `updateUser`, `deleteUser` generan N-13 (`audience='admin'`) y actividad.
- **Historial:** sin cambios de contrato.

### 4.3 DTOs nuevos (resumen; `camelCase`)

- `NotificationItem { id, type, priority, message, entityType, entityId, isRead, createdAt }`, `NotificationFilters { unreadOnly, search, date }`.
- `Settings { lowStockThreshold, staleInvoiceDays, backupReminderDays, lastBackupAt }`, `UserPreferences { visibleCurrencies: string[], pageSize }`, `ExchangeRate { currency, rate: string, updatedAt, updatedByName }`, `ExchangeRateChange { createdAt, currency, previousRate, newRate, userName }`, `CompanyProfile { name, address, phone, taxId, logoDataUrl }`.
- `InvoicePrintModel { title, number, date, company, contact, lines[{ name, quantity, unit, unitPriceCents, subtotalCents }], totalCents, notes, associated, reservedUntil }`.
- `DashboardKpis` (DB-02 a DB-07), `DraftItem { formKey, recordId, updatedAt }`, `ActivityItem`, `ImportSummary`, `BackupItem { fileName, createdAt, sizeBytes }`.

### 4.4 Catálogo de errores nuevos

| Código | Texto | Se registra |
|---|---|---|
| `schemaTooNew` | "Esta base de datos es de una versión más nueva de la aplicación" | Sí |
| `migrationFailed` | "No se pudo actualizar la base de datos. La aplicación no puede continuar" | Sí |
| `backupFailed` | "No se pudo crear la copia de seguridad" | Sí |
| `invalidBackup` | "El archivo elegido no es una copia de seguridad válida" | Sí |
| `restoreFailed` | "No se pudo restaurar la copia. Tus datos actuales no han cambiado" | Sí |
| `invalidCredentials` (reutilizado) | "La contraseña no es correcta" | No |
| `validationFailed` (reutilizado) | Mensajes específicos: logo, tasa, umbral, fecha de reserva, conversión, CSV | No |

## 5. Frontend: servicios, vistas y componentes

### 5.1 Servicios nuevos

`notificationsService.js`, `settingsService.js`, `draftsService.js`, `activityService.js`, `csvService.js`, `backupService.js`; ampliaciones en `invoicesService.js` (`exportInvoicePdf`, `getInvoicePrintModel`), `productsService.js` (`listUnitOptionsForProduct`) y `dashboardService.js` (`getDashboardKpis`).

### 5.2 Vistas nuevas o reescritas

- `views/notificacionesView.js` (sustituye `renderNotificaciones` y `notificacionesData` de `dashboard.js`).
- `views/dashboardView.js` (amplía la existente: período global y KPIs reales en los ids del mockup: `dash-gasto-*`, `dash-estrella-*`, `dash-frecuentes`, `dash-vendidos-*`, `dash-lento`, `dash-chart`, tarjetas `data-kpi`).
- `views/configuracionView.js` (sustituye `renderMonedas`, `renderHistorialTasas`, `renderRateCards` y los datos simulados `monedasData`/tasas de `dashboard.js`; pestañas `cfgtab-*` y nuevas "Umbrales" y "Respaldo").
- `views/borradoresView.js`, `views/actividadView.js` (solo Admin; el enlace del sidebar se oculta al Contador como "Usuarios").
- `views/printPreview.js`.

### 5.3 Componentes

`components/toast.js`, `utils/feedback.js`, `utils/drafts.js`, `services/notificationFeed.js`; el selector de unidad por línea se añade al editor de líneas de `facturasView.js` (un `<select>` por fila con las opciones de `listUnitOptionsForProduct`).

### 5.4 Endurecimiento (SG-01)

- Inventario con `grep` de `onclick=`/`oninput=`/`onchange=`/`onsubmit=` y de `innerHTML` con interpolación en `dashboard-unificado.html` y `dashboard.js`; cada uno pasa a listener o a DOM con `textContent`. Test en `environment.test.js`: cero manejadores inline en todo el HTML y cero `innerHTML =` con plantillas en el JS de producción.
- Sidebar: `showPage` se enlaza con `data-page` en lugar de `onclick="return showPage(...)"`; `irAFacturaDesdeGeneral` deja de buscar `a[onclick*=…]` y usa `[data-page]`.
- CSP de §1 activada al final de la fase 1, con test que lee `tauri.conf.json`.

## 6. Estructura de carpetas (ampliación)

```
src-tauri/src/
  automation.rs   activity.rs   backup.rs   csv_io.rs   invoice_print.rs
  db/migrations.rs
  commands/notifications.rs   commands/settings.rs   commands/drafts.rs
src/
  css/toast.css   css/print.css
  js/components/toast.js
  js/utils/feedback.js   js/utils/drafts.js
  js/services/notificationsService.js   settingsService.js   draftsService.js
             activityService.js   csvService.js   backupService.js   notificationFeed.js
  js/views/notificacionesView.js   configuracionView.js   borradoresView.js
           actividadView.js   printPreview.js
```

## 7. Adaptación de interfaz existente

- Mensajes de todas las vistas → toast (TO-01). Los errores por campo usan `setFieldError`.
- Facturas: "Reservar hasta" (venta y servicio), filtro "Reservas activas" y columna de fecha límite; selector de unidad por línea; botones "Imprimir" y "Exportar a PDF" en los seis detalles (sustituyen a "Exportar Factura" de jsPDF).
- Productos: "Cantidad reservada" en el detalle; filtro "Stock bajo" con el umbral configurado.
- Configuración: pestaña "Facturación" pasa a "Empresa" (datos y logo; se elimina "Datos de Factura" si se confirma S-06), "Moneda" (por usuario), "Tasas" (tarjetas + historial), "General" (tamaño de página), y nuevas "Umbrales" y "Respaldo" (crear, copias automáticas, restaurar, importar/exportar CSV).
- Sidebar: "Borradores" (ambos roles) y "Registro de actividad" (solo Admin).

## 8. Estrategia de testing (test-first)

- **Rust:** cada migración con su test (de una base versión 1 real a la última, idempotencia, `schemaTooNew`); `notify`/`watch_*` (una vez por situación, reaparición); `expire_reservations` (cancela, devuelve lotes, Reversión con "Sistema", N-06, RA); conversiones (compra y venta en Caja, redondeos, sin conversión); `getDashboardKpis` con un escenario fijo de ventas, cobros, gastos y pérdidas que verifique cada cifra a mano; borradores (por usuario, sin contraseñas, caducidad); actividad (inmutable, en la misma transacción, `require_admin`); CSV (BOM, `;`, fórmulas neutralizadas, filas omitidas con motivo, `SAVEPOINT` por fila, límite); respaldo (copia consistente, rechazo de archivo ajeno o más nuevo, restauración con migración, contraseña incorrecta, sesiones borradas); PDF de factura (se escribe, contiene título por tipo y no contiene el estado). Matriz de autorización de los 31 comandos.
- **Vitest + jsdom (HTML de producción):** toasts (tipos, duraciones con temporizadores falsos, pausa, máximo, roles ARIA, texto seguro, `prefers-reduced-motion`); cada vista migrada muestra su resultado como toast y los errores de campo junto al campo; pestaña de notificaciones (contador, leer, eliminar, navegar); reservas en el formulario y el filtro; selector de unidad; dashboard con un DTO fijo pinta cada id del mockup; configuración; vista previa de impresión (título por tipo, sin estado); borradores (guarda con *debounce*, no guarda contraseñas, recupera, descarta); actividad oculta al Contador; CSV y respaldo (diálogos, resumen); `environment.test.js` (sin inline, sin `innerHTML` con datos, CSP, contrato `invoke ↔ generate_handler!` con 107 comandos, sin jsPDF).
- **Verificación manual** (fin de la fase 5, con la de `spec-01` y `spec-03`): `tauri:dev` y `tauri:build`, como Administrador y como Contador; imprimir en papel y a PDF; restaurar una copia en otra carpeta; importar un CSV abierto y guardado con Excel en español.

## 9. Complexity Tracking (desviaciones de `Constitution.md`)

| Desviación | Motivo | Alternativa descartada |
|---|---|---|
| `Constitution.md` §4 cambia a v1.3.0 (migraciones) | Respaldo y restauración entre versiones (PEN-15) | Restaurar solo en la misma versión: rompe al actualizar la app. |
| Dependencia `csv` | Ver §1 | Parser CSV propio: más código y más riesgo. |
| `style-src 'unsafe-inline'` en la CSP | Los `style=` del mockup no ejecutan código | Reescribir cientos de estilos inline sin beneficio de seguridad. |
| Cambio visual transversal (toasts) | Decisión del dueño; Constitution §2.V lo permite si se documenta | Mantener el texto plano. |
| Excepción a `spec-03` §3.2 (las reservas vencen) | RF-119, RF-120, decisión del dueño | Solo avisar sin cancelar. |
| `Author::System` para movimientos y actividad automáticos | El vencimiento y la purga no tienen sesión | Fabricar un `Actor`: lo prohíbe `Agents.md` §3. |

## 10. Decisiones técnicas (D-01 a D-16)

| # | Decisión |
|---|---|
| D-01 | CSP estricta para scripts, `unsafe-inline` solo en estilos (§1). |
| D-02 | `Author::{User, System}` (§3.2). |
| D-03 | Versión del esquema en `user_version`; identidad en `application_id`; versión 1 = esquema actual (§2.1). |
| D-04 | Notificaciones como toast vía `command:success` + `listNotificationsSince`, sin temporizadores (§3.12). |
| D-05 | Un modelo de documento común para la vista previa y el PDF (§3.7). |
| D-06 | Vista previa con `window.print()` y `@media print`; sin iframe ni ventana nueva. |
| D-07 | Logo como `data:` URL hacia el frontend; archivo en `app_data_dir/company/`. |
| D-08 | Reserva = `invoices.reservedUntil`, sin tipo nuevo ni estado nuevo (§2.3). |
| D-09 | Vencimiento de reservas al arrancar y en `before_operation` (S-04). |
| D-10 | Conversión de un paso, directa o inversa; cantidad convertida a 3 decimales (§3.5). |
| D-11 | Un solo comando `getDashboardKpis` por período; monedas en el frontend (§3.6). |
| D-12 | Borradores en la base como JSON, `UPSERT` por (usuario, formulario, registro) (§3.8). |
| D-13 | Registro de actividad en la misma transacción que la acción (§3.9). |
| D-14 | CSV: `SAVEPOINT` por fila y el mismo `create_*_inner` que el formulario (§3.10). |
| D-15 | Copias con `VACUUM INTO`; restauración sustituyendo la conexión dentro del `Mutex` (§3.11). |
| D-16 | Preferencias por usuario y ajustes globales en tablas separadas con claves cerradas (§2.4). |
| D-17 | **Logo siempre en JPEG** (spike de `task-04` §0): el frontend lee el PNG o JPG elegido con `<input type="file">` y `FileReader`, lo dibuja en un `<canvas>` (fondo blanco) y envía el JPEG en base64 a `saveCompanyLogo`. Rust valida la firma y el marcador SOF (ancho, alto, 1 o 3 componentes), el tamaño (≤ 2 MB) y guarda `app_data_dir/company/logo.jpg`. El PDF lo incrusta con `ImageXObject` + `ImageFilter::DCT`. Sin dependencias nuevas. |

## 11. Siguiente paso

Con la spec confirmada, `task-04.md` se ejecuta por fases (spec §3.3). Al cerrar cada fase: suites completas, `clippy --all-targets -D warnings`, `cargo fmt --check`, `prettier --check .` y commit. La verificación manual de las tres specs se hace al final de la fase 5.
