# Tareas — Cierre del MVP

- **Basado en:** `spec-04.md` y `plan-04.md` (esta misma carpeta), `Constitution.md` (v1.2.0 → v1.3.0 en la fase 1), `Agents.md`, `spec-01` a `spec-03` y `pendientes.md`.
- **Estado:** **Confirmado por el dueño (2026-10-28). Listo para implementar.**
- **Enfoque:** por fases (`spec-04` §3.3) y, dentro de cada fase, vertical: test de Rust → comando → servicio y vista JS → test de interfaz sobre el HTML de producción.
- **Convenciones:** las de `task-03`: comandos en `camelCase` (wrapper JS) / `snake_case` (Rust), envoltorio delgado sobre `xxx_inner`, cada comando en `generate_handler!` y en la matriz de autorización en la misma tarea que lo implementa, sin `#[allow(non_snake_case)]`.
- **Test-first** (`Constitution.md` §2.III): cada test falla antes de implementar y ninguna implementación hace trampa.
- **Garantía:** no se reabre ninguna tarea [x] de `task-01` a `task-03`; los cambios a módulos existentes se hacen desde aquí.
- **Cierre de cada fase:** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`, `npm test`, `npx prettier --check .`, actualizar `pendientes.md` y commit (`Agents.md` §8). Las suites se ejecutan en un solo hilo (la máquina se comparte con otro proyecto).

---

## 0. Antes de empezar

- [x] Confirmar con el dueño los supuestos S-01 a S-11 de `spec-04` §11 y anotar las respuestas en la spec.
  - Confirmados tal como están (2026-10-28).
- [ ] Línea base: `cargo test` (373), `npm test` (303), clippy, fmt y prettier en verde antes de tocar nada.
- [ ] Inventario con `grep` (sin números de línea): manejadores inline y `innerHTML` con datos en `dashboard-unificado.html` y `dashboard.js`; mensajes `form-feedback`/`showMessage`/`showStatus`/`showFeedback` por vista; datos simulados de Notificaciones, Dashboard y Configuración en `dashboard.js`.
- [ ] Comprobar que `printpdf` 0.7 con `embedded_images` e `image` 0.25 (`png`, `jpeg`) compilan sin warnings y sin arrastrar `tokio` (spike, `plan-04` §1).
- [ ] Comprobar en `tauri:dev` que `window.print()` abre el diálogo de impresión del sistema en WebView2 (spike de D-06). Si no, anotar la alternativa antes de la fase 3.

## 1. Fase 1 — Base: migraciones, toasts y endurecimiento

### 1.1 Migraciones (MG-01)

- [ ] Test (Rust): una base vacía queda en la última versión; una base de las specs 1 a 3 (`user_version = 0` con `users`) se marca como versión 1 y migra; ejecutar dos veces no cambia nada.
- [ ] Test (Rust): una migración que falla deja la base en la versión anterior y devuelve `migrationFailed`; una base con versión mayor devuelve `schemaTooNew`.
- [ ] Implementación: `db/migrations.rs` (`run_migrations`, lista ordenada), llamada en el `setup` después de abrir la conexión; pantalla de error si falla.
- [ ] Constitution v1.3.0: §4 con migraciones versionadas y entrada en el historial de versiones (`spec-04` §7.8).

### 1.2 Toasts (TO-01)

- [ ] Test (Vitest): `showToast` crea la región una sola vez; cada tipo tiene su icono, clase y rol (`status`/`alert`); éxito/info se cierran a los ~4 s, advertencia a los ~7 s, error/crítico no (temporizadores falsos); `mouseenter`/`focusin` pausan; máximo 4 visibles; botón cerrar con `aria-label`; el mensaje con HTML se pinta como texto; sin animación con `prefers-reduced-motion`.
- [ ] Test (Vitest): `utils/feedback.js` traduce un `AppError` de validación a su mensaje, un error interno a uno genérico, y `setFieldError` pone el texto bajo el campo con `aria-describedby` y `aria-invalid`; `clearFieldError` lo quita al escribir.
- [ ] Implementación: `components/toast.js`, `css/toast.css` (paleta de `dashboard.css`, S-01), `utils/feedback.js`.
- [ ] Migración de vistas, una casilla por vista y su test (resultado como toast, error de campo junto al campo, aviso de purga como toast de advertencia):
  - [ ] `loginView`, `recoveryView` (solo errores generales; los de campo se conservan).
  - [ ] `usuarios`, `perfil`.
  - [ ] `categoriasView`, `unidadesView`, `productosView`.
  - [ ] `clientesView`, `proveedoresView`, `historialView`.
  - [ ] `facturasView` (los seis submódulos y la lista general), `quickContactDialog`, `quickProductDialog`.
- [ ] Test (Vitest, `environment.test.js`): ninguna vista de producción usa ya `.form-feedback` para resultados generales.

### 1.3 Endurecimiento y CSP (SG-01)

- [ ] Test (Vitest, `environment.test.js`): cero atributos `on*` en todo `dashboard-unificado.html`, `index.html` y `recovery.html`; ningún `innerHTML =` con plantilla o concatenación en el JS de producción; `tauri.conf.json` tiene la CSP de `plan-04` §1.
- [ ] Implementación: sidebar con `data-page` y un listener; `irAFacturaDesdeGeneral` sin `a[onclick*=…]`; resto de `onclick` del mockup a listeners; `innerHTML` con datos a DOM/`textContent` (incluidos `clientesView` y `proveedoresView`, que aún lo usan en sus listados).
- [ ] Implementación: activar la CSP. Comprobar en `tauri:dev` que la app arranca y navega sin errores de CSP en la consola.

### 1.4 Cierre de la fase 1

- [ ] Suites, lint, formato y commit.

## 2. Fase 2 — Notificaciones y reservas

### 2.1 Esquema (migraciones 2 y 3)

- [ ] Test (Rust): `notifications` acepta `entityType`/`entityId` juntos o ninguno y `audience` `all`/`admin`; `notificationWatches` rechaza duplicados; `invoices.reservedUntil` solo en V/S y posterior a la emisión.
- [ ] Implementación: migraciones 2 y 3.

### 2.2 Generación y "una vez por situación" (NT-02, NT-03)

- [ ] Test (Rust): `watch_enter`/`watch_leave`: entrar avisa una vez; seguir no repite; salir y volver avisa de nuevo; stock bajo → agotado genera N-02.
- [ ] Test (Rust), una casilla por aviso con su mensaje exacto y prioridad:
  - [ ] N-02 al quedar en 0 y al intentar vender un agotado (se inserta aunque la venta se rechace).
  - [ ] N-03 con el umbral por defecto y con uno cambiado.
  - [ ] N-04 (hoy a hoy + 29) y N-07 (< 10 días).
  - [ ] N-05 (< 3 días) y N-06 (vencida).
  - [ ] N-10 (compra Por Pagar y venta Por Cobrar con más de los días configurados).
  - [ ] N-11 (sin copia en los días configurados).
  - [ ] N-12 (purga) y N-13 (usuarios, `audience='admin'`).
- [ ] Implementación: `db/notifications.rs` (`notify`, `watch_*`), `automation.rs` (`run_startup_processes`, `before_operation`, `after_inventory_change`) y las llamadas desde productos, facturas y usuarios.

### 2.3 Comandos y pestaña (NT-01, NT-04, TO-02)

- [ ] Test (Rust): los 7 comandos de `notifications.rs` (listar con filtros, contar, desde un id, marcar una y todas, eliminar una y las filtradas); el Contador no ve ni cuenta las de `audience='admin'`; purga a los 90 días. Filas de la matriz de autorización.
- [ ] Test (Vitest, HTML de producción): la pestaña lista datos reales con prioridad y fecha; "Todas"/"No leídas", buscador y fecha filtran; clic marca leída y abre el producto o la factura (o avisa si ya no existe); "Marcar todas", eliminar una y todas sin confirmación; contador de cabecera y pestaña se actualizan.
- [ ] Test (Vitest): tras un comando de escritura, `notificationFeed` pide las nuevas y muestra un toast por cada una con su prioridad; al iniciar sesión, un único toast resumen.
- [ ] Implementación: `notificationsService.js`, `notificationFeed.js`, evento `command:success` en `invoke.js` (con su test sin romper el existente), `notificacionesView.js`; retirar `notificacionesData` y `renderNotificaciones` de `dashboard.js`.

### 2.4 Reservas (RS-01 a RS-03)

- [ ] Test (Rust): crear y editar venta/servicio con `reservedUntil` válido; rechazos (antes o igual a la emisión, más de 365 días, garantía/insumos/entrada/pérdida, estado no pendiente); cobrar o cancelar la desactiva; quitar la fecha la convierte en venta normal.
- [ ] Test (Rust): `expire_reservations` cancela solo las vencidas, devuelve los consumos a sus lotes, registra la Reversión con "Sistema", genera N-06 y la entrada `expire`, y respeta el invariante de cachés; se ejecuta al arrancar y en `before_operation`.
- [ ] Test (Rust): `listInvoices` con `status='reserved'`; `getInvoice` y `InvoiceListItem` con `reservedUntil`; `getProduct` con `reservedQuantity`.
- [ ] Test (Vitest): campo "Reservar hasta" en venta y servicio (oculto si el estado no es Por Cobrar), se envía al crear y editar; filtro "Reservas activas" y columna de fecha límite; detalle con "Reservada hasta"; detalle del producto con la cantidad reservada y enlaces.
- [ ] Implementación en `commands/invoices.rs`, `commands/products.rs`, `facturasView.js`, `productosView.js` y el HTML.

### 2.5 Cierre de la fase 2

- [ ] Suites, lint, formato, `pendientes.md` (PEN-01, PEN-02, PEN-14, PEN-18) y commit.

## 3. Fase 3 — Configuración, impresión y conversiones

### 3.1 Esquema (migraciones 4 y 5)

- [ ] Test (Rust): claves cerradas y rangos de `settings`; `userPreferences` por usuario y en cascada al borrar el usuario; tasas > 0; historial inmutable; columnas de unidad por línea juntas o ninguna.
- [ ] Implementación: migraciones 4 y 5 con los valores por defecto.

### 3.2 Ajustes, monedas y tasas (CF-01, CF-02, CF-04)

- [ ] Test (Rust): `getSettings`/`updateSettings` (rangos; cambiar el umbral reevalúa N-03 y cambia el filtro "Stock bajo" y DASH-01); `getUserPreferences`/`updateUserPreferences` (monedas válidas, tamaño 10/20/50/100, cada usuario los suyos); `updateExchangeRate` (6 decimales, > 0) con historial y actividad; `listExchangeRateHistory`. Matriz de autorización.
- [ ] Test (Vitest): pestañas de Configuración con datos reales (tarjetas de tasas, historial con orden, monedas por usuario, tamaño de página desde la base); el detalle del producto muestra los equivalentes de las monedas elegidas con la tasa actual; una moneda sin tasa no se muestra y Configuración lo avisa.
- [ ] Implementación: `commands/settings.rs`, `settingsService.js`, `configuracionView.js`; el paginador global lee el tamaño de la base; retirar `monedasData` y los datos simulados de tasas de `dashboard.js`.

### 3.3 Empresa y logo (CF-03)

- [ ] Test (Rust): `updateCompanyProfile` (longitudes); `saveCompanyLogo` acepta PNG/JPG reales de hasta 2 MB, rechaza otro contenido con extensión falsa, sustituye y borra el anterior, y con `null` lo quita; `getCompanyProfile` devuelve el logo como `data:` URL. Actividad `config`.
- [ ] Test (Vitest): pestaña "Empresa" con los cuatro datos y el logo (elegido con el diálogo nativo; sin `onclick` ni la imagen de `placehold.co`); "Datos de Factura" según S-06.
- [ ] Implementación en `settings.rs`, `configuracionView.js` y el HTML.

### 3.4 Impresión y PDF (IM-01, IM-02)

- [ ] Test (Rust): `build_invoice_document` para los seis tipos (título por tipo, contacto, líneas en la unidad escrita, total, asociada, "Reservada hasta", **sin estado**); `exportInvoicePdf` escribe un PDF válido con el logo y falla con `exportFailed` si no puede escribir. Matriz de autorización de `exportInvoicePdf` y `getInvoicePrintModel`.
- [ ] Test (Vitest): "Imprimir" en los seis detalles abre la vista previa con el modelo y llama a `window.print()`; "Exportar a PDF" abre "Guardar como" y llama al comando; cancelar no hace nada; error como toast.
- [ ] Implementación: `invoice_print.rs`, `printPreview.js`, `css/print.css`, botones en los seis detalles; retirar `exportarFactura*PDF` de `dashboard.js` y jsPDF.

### 3.5 Conversiones en facturas (CV-01)

- [ ] Test (Rust): `listUnitOptionsForProduct` (directa e inversa de un paso); compra de 2 Caja (×12) a 30,00 crea un lote de 24 u a 2,50; venta de 1 Caja descuenta 12 u al precio vigente × 12; redondeos de la cantidad convertida; sin conversión → `validationFailed`; el historial guarda la cantidad convertida; edición y anulación devuelven lo convertido.
- [ ] Test (Vitest): selector de unidad por línea en los seis formularios (en la pérdida, junto a la cantidad); el total preliminar usa lo escrito; se envía `unitId`; el detalle y la impresión muestran "2 Caja".
- [ ] Implementación en `inventory.rs`, `commands/invoices.rs`, `commands/products.rs`, `facturasView.js`.

### 3.6 Cierre de la fase 3

- [ ] Suites, lint, formato, `pendientes.md` (PEN-06, PEN-07, PEN-19, PEN-20) y commit.

## 4. Fase 4 — Dashboard

- [ ] Test (Rust): `getDashboardKpis` con un escenario fijo (ventas y servicios cobrados y por cobrar, en días distintos; garantías, insumos y pérdidas; productos con margen distinto; un cliente eliminado) que verifica a mano cada cifra de DB-02 a DB-07 para cada período (hoy, semana, mes, 3 m, 6 m, 1 año, rango); ingresos por fecha de cobro; costo real FIFO; Pareto 80/20 de productos y clientes; lento movimiento con un producto nunca vendido (S-09). Matriz de autorización.
- [ ] Test (Vitest, HTML de producción): con un DTO fijo, cada id del mockup (`data-kpi`, `dash-gasto-*`, `dash-estrella-*`, `dash-frecuentes`, `dash-vendidos-*`, `dash-lento`, `dash-chart`) muestra su valor; el selector de período tiene las siete opciones y recarga; los equivalentes en otras monedas aparecen según las preferencias; estado de error con "Reintentar"; tarjeta de reservas activas con enlaces; se recalcula al volver al dashboard tras guardar.
- [ ] Implementación: `commands/dashboard.rs`, `dashboardService.js`, `dashboardView.js` y el HTML (opciones de período y tarjeta de reservas sin cambiar el diseño).
- [ ] Cierre: suites, lint, formato, `pendientes.md` (PEN-16) y commit.

## 5. Fase 5 — Borradores, registro de actividad, CSV y respaldo

### 5.1 Esquema (migraciones 6 a 8)

- [ ] Test (Rust): `drafts` único por (usuario, formulario, registro) y en cascada; `activityLog` inmutable y sin borrado; `application_id` fijado.
- [ ] Implementación: migraciones 6, 7 y 8.

### 5.2 Borradores (BR-01 a BR-03)

- [ ] Test (Rust): `saveDraft` hace `UPSERT`; cada usuario ve solo los suyos; `formKey` fuera de la lista se rechaza; un payload con claves de contraseña se rechaza; caducidad a los 30 días al arrancar. Matriz de autorización de los 4 comandos.
- [ ] Test (Vitest): `attachDraft` guarda con *debounce* de 1 s, nunca incluye campos `password` ni `file`, se borra al guardar o cancelar; al abrir un formulario con borrador pregunta y "Continuar" lo carga / "Descartar" lo borra; una factura recuperada revalida contacto y productos; toast al iniciar sesión; sección "Borradores" lista, abre y elimina.
- [ ] Implementación: `commands/drafts.rs`, `draftsService.js`, `utils/drafts.js`, `borradoresView.js`, y `attachDraft` en cada formulario (una casilla por vista, como en §1.2).

### 5.3 Registro de actividad (RA-01, RA-02)

- [ ] Test (Rust), una casilla por grupo: eliminaciones de cada entidad; cancelaciones (manual y por vencimiento, con "Sistema"); ediciones de facturas; cambios de configuración; respaldos, restauraciones e importaciones; purga. Cada entrada se escribe en la misma transacción (si la acción falla, no hay entrada) y sin datos personales.
- [ ] Test (Rust): `listActivityLog` con filtros y orden; Contador → rechazado.
- [ ] Test (Vitest): la sección solo aparece al Administrador; filtros, búsqueda y orden; texto seguro.
- [ ] Implementación: `activity.rs`, llamadas desde todos los comandos de RA-01, `activityService.js`, `actividadView.js`.

### 5.4 CSV (CSV-01, CSV-02)

- [ ] Test (Rust): `exportCsv` por cada tipo respeta los filtros, escribe BOM, `;`, decimales con coma, fechas `dd/mm/aaaa` y neutraliza fórmulas.
- [ ] Test (Rust): `importCsv` de productos (crea categoría y unidad que falten, lote inicial y movimiento Inicial), categorías, unidades, clientes y proveedores; filas duplicadas o inválidas se omiten con número y motivo y las válidas se guardan; archivo vacío, encabezados desconocidos o más de 5 000 filas se rechazan; actividad `import`. Matriz de autorización.
- [ ] Test (Vitest): botones de exportar en los listados (con "Guardar como") y de importar en Configuración (con "Abrir"); resumen del resultado como toast con detalle de las filas omitidas.
- [ ] Implementación: `csv_io.rs`, `csvService.js`, botones en las vistas.

### 5.5 Respaldo y restauración (RP-01 a RP-03)

- [ ] Test (Rust): `createBackup` produce una base válida con todos los datos, actualiza `lastBackupAt` y registra actividad; la copia diaria crea una al día y conserva 7; `listAutomaticBackups`.
- [ ] Test (Rust): `restoreBackup` con contraseña incorrecta no toca nada; con un archivo ajeno, corrupto o de versión mayor → `invalidBackup` sin tocar nada; con una copia de versión menor la migra; hace copia previa de lo actual, sustituye la base, borra las sesiones y registra la restauración. Matriz de autorización.
- [ ] Test (Vitest): pestaña "Respaldo": crear copia (aviso de que no está cifrada), lista de automáticas, restaurar con confirmación y contraseña y vuelta al login.
- [ ] Implementación: `backup.rs` y su llamada en `run_startup_processes`, `backupService.js`, `configuracionView.js`.

### 5.6 Cierre del módulo y del MVP

- [ ] Revisión contra `spec-04` requisito por requisito (MG, TO, SG, NT, RS, CF, IM, CV, DB, BR, RA, CSV, RP) y contra la trazabilidad de la §12.
- [ ] Matriz de autorización: los 31 comandos nuevos con su fila; `listActivityLog` con la de Contador rechazado.
- [ ] Contrato `invoke ↔ generate_handler!`: **107 comandos**, sin `pendingWrapper`.
- [ ] `environment.test.js`: sin manejadores inline, sin `innerHTML` con datos, sin jsPDF, sin datos simulados en `dashboard.js`, CSP presente.
- [ ] Documentación: `Agents.md` §1 (las cuatro specs implementadas) y §7; `pendientes.md` (PEN-01, 02, 06, 07, 12 a 16, 18 a 22 resueltos o con su estado); Constitution v1.3.0 en vigor.
- [ ] **Verificación manual de extremo a extremo** de `spec-01`, `spec-03` y `spec-04` en `tauri:dev` y `tauri:build`, como Administrador y como Contador: toasts en cada flujo; notificaciones y reservas (incluido un vencimiento cambiando la fecha del equipo o la fecha límite); imprimir en papel y exportar a PDF; adjuntar un PDF de compra con el diálogo y arrastrándolo; compra con producto nuevo y con conversión; dashboard con datos; borrador recuperado tras cerrar la app a la fuerza; CSV abierto y guardado en Excel en español e importado; crear una copia y restaurarla en otra carpeta de datos.
- [ ] Suites finales con recuentos, lint, formato y commit.
