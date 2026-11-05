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
- [x] Línea base: `cargo test` (373), `npm test` (303), clippy, fmt y prettier en verde antes de tocar nada.
  - Tomada del commit `8af651e` (sin cambios de código desde entonces): 373/373, 303/303, clippy, fmt y prettier limpios.
- [x] Inventario con `grep` (sin números de línea): manejadores inline y `innerHTML` con datos en `dashboard-unificado.html` y `dashboard.js`; mensajes `form-feedback`/`showMessage`/`showStatus`/`showFeedback` por vista; datos simulados de Notificaciones, Dashboard y Configuración en `dashboard.js`.
  - 2026-10-28: **55** manejadores `on*` en `dashboard-unificado.html` (0 en `index.html` y `recovery.html`). `innerHTML` con asignación en `dashboard.js` (17), `unidadesView` (12), `categoriasView` (6), `proveedoresView` (6), `clientesView` (4), `usuarios` (4) y `dashboardView` (3). Mensajes de texto en `facturasView`, `productosView`, `clientesView`, `proveedoresView`, `historialView`, `usuarios`, `perfil`, `loginView`, `recoveryView`, `categoriasView`, `unidadesView`, `formDialog` y `cleanupWarning`. Datos simulados: `monedasData`, `historialTasasData` y `rateCardsData` (`notificacionesData` ya arranca vacío).
- [x] Comprobar que `printpdf` 0.7 con `embedded_images` e `image` 0.25 (`png`, `jpeg`) compilan sin warnings y sin arrastrar `tokio` (spike, `plan-04` §1).
  - **Resultado:** no hace falta. `embedded_images` arrastra `image` 0.24 con ocho formatos; en cambio, `ImageXObject` con `ImageFilter::DCT` incrusta un JPEG sin decodificarlo y no depende de esa *feature*. Decisión D-17 de `plan-04`: logo siempre en JPEG, convertido en el frontend con un `<canvas>`. Se comprobará al implementar IM-02 (fase 3).
- [ ] Comprobar en `tauri:dev` que `window.print()` abre el diálogo de impresión del sistema en WebView2 (spike de D-06). Si no, anotar la alternativa antes de la fase 3.
  - **Pendiente del dueño:** necesita operar la ventana de la app; no bloquea las fases 1 y 2.

## 1. Fase 1 — Base: migraciones, toasts y endurecimiento

### 1.1 Migraciones (MG-01)

- [x] Test (Rust): una base vacía queda en la última versión; una base de las specs 1 a 3 (`user_version = 0` con `users`) se marca como versión 1 y migra; ejecutar dos veces no cambia nada.
- [x] Test (Rust): una migración que falla deja la base en la versión anterior y devuelve `migrationFailed`; una base con versión mayor devuelve `schemaTooNew`.
- [x] Implementación: `db/migrations.rs` (`run_migrations`, lista ordenada), llamada en el `setup` después de abrir la conexión; pantalla de error si falla.
  - `initialize_database` aplica `run_migrations` tras `init_schema`; si falla, registra la causa en `errors.log` y el diálogo de arranque muestra el mensaje de `migrationFailed`/`schemaTooNew`. `setup_test_db` también migra. Tests: 4 en `db/migrations.rs`; `cargo test` 377/377.
- [x] Constitution v1.3.0: §4 con migraciones versionadas y entrada en el historial de versiones (`spec-04` §7.8).
  - Hecho; `Agents.md` alineado con v1.3.0 y una regla nueva "Cambios de esquema".

### 1.2 Toasts (TO-01)

- [x] Test (Vitest): `showToast` crea la región una sola vez; cada tipo tiene su icono, clase y rol (`status`/`alert`); éxito/info se cierran a los ~4 s, advertencia a los ~7 s, error/crítico no (temporizadores falsos); `mouseenter`/`focusin` pausan; máximo 4 visibles; botón cerrar con `aria-label`; el mensaje con HTML se pinta como texto; sin animación con `prefers-reduced-motion`.
- [x] Test (Vitest): `utils/feedback.js` traduce un `AppError` de validación a su mensaje, un error interno a uno genérico, y `setFieldError` pone el texto bajo el campo con `aria-describedby` y `aria-invalid`; `clearFieldError` lo quita al escribir.
- [x] Implementación: `components/toast.js`, `css/toast.css` (paleta de `dashboard.css`, S-01), `utils/feedback.js`.
- [x] Migración de vistas, una casilla por vista y su test (resultado como toast, error de campo junto al campo, aviso de purga como toast de advertencia):
  - [x] `loginView`, `recoveryView` (solo errores generales; los de campo se conservan).
  - [x] `usuarios`, `perfil`.
  - [x] `categoriasView`, `unidadesView`, `productosView`.
  - [x] `clientesView`, `proveedoresView`, `historialView`.
  - [x] `facturasView` (los seis submódulos y la lista general), `quickContactDialog`, `quickProductDialog`.
- [x] Test (Vitest, `environment.test.js`): ninguna vista de producción usa ya `.form-feedback` para resultados generales. Hecho: se quitaron además los contenedores sin uso (`productos-feedback`, `producto-*-feedback`, `usuario(s)-feedback`, `perfil-feedback`, `config-paginacion-feedback`); se conservan como ayudas junto al campo los avisos en vivo de `categoria-feedback`/`unidad-feedback` y la vista previa del precio. `formDialog` (altas rápidas) marca el campo y avisa por toast. Suite: 329/329.

### 1.3 Endurecimiento y CSP (SG-01)

- [x] Test (Vitest, `environment.test.js`): cero atributos `on*` en todo `dashboard-unificado.html`, `index.html` y `recovery.html`; ningún `innerHTML =` con plantilla o concatenación en el JS de producción; `tauri.conf.json` tiene la CSP de `plan-04` §1.
  - Hecho: además, cero `on*` en las plantillas del JS, ningún `<script>` inline ni recurso externo (`http(s)://`) en los tres HTML. Los `innerHTML` con literal fijo (iconos de `usuarios.js`) se admiten.
  - **Desviación (justificada):** los renderizadores de datos de prueba de `dashboard.js` quedan en una lista de pendientes del test, cada uno con la fase que lo sustituye al conectar datos reales: monedas y tasas (fase 3, CF-02), notificaciones (fase 2, NT-01), KPIs/listas/gastos del dashboard (fase 4, DB-01). Hoy solo pintan constantes del propio código, sin datos del usuario; convertirlos ahora se reescribiría en su fase. El test falla si aparece un `innerHTML` nuevo con datos o si una línea pendiente desaparece sin quitarla de la lista, así que cada fase debe vaciar la suya.
- [x] Implementación: sidebar con `data-page` y un listener; `irAFacturaDesdeGeneral` sin `a[onclick*=…]`; resto de `onclick` del mockup a listeners; `innerHTML` con datos a DOM/`textContent` (incluidos `clientesView` y `proveedoresView`, que aún lo usan en sus listados).
  - Hecho: los 55 `on*` del HTML y los 6 de las plantillas de `dashboard.js` pasan a `data-page`/`data-goto-page`/`data-action`/`data-input-action`/`data-change-action` con listeners delegados en `dashboard.js`; fuera `removeInlineHandlers`/`removeLegacyInlineHandlers`. Nuevo `utils/dom.js` (`el`, `messageRow`, `setOptions`) sustituye las cinco copias de `escapeHtml`: listados de clientes, proveedores, categorías (y su detalle), unidades, conversiones y sus selectores, `dashboardView` y el paginador global se construyen con DOM.
- [ ] Implementación: activar la CSP. Comprobar en `tauri:dev` que la app arranca y navega sin errores de CSP en la consola.
  - Hecho (falta la comprobación en la app): CSP de `plan-04` §1 en `tauri.conf.json`; el script inline de `index.html` (mostrar contraseña) pasa a `loginView.js`; el logo de ejemplo de Configuración deja `placehold.co` por un SVG `data:` (la fase 3 pone el logo real). Chart.js y jsPDF no usan `eval`/`new Function`. **Pendiente: arrancar `npm run tauri:dev` y revisar la consola en login, recuperación y cada página del menú.**

### 1.4 Cierre de la fase 1

- [x] Suites, lint, formato y commit. `cargo test` 377/377, `npm test` 332/332 (38 archivos), `clippy --all-targets -D warnings` y `cargo fmt --check` limpios, `prettier --check .` limpio (`dashboard-unificado.html`, que ya venía sin formatear, queda formateado). Falta la comprobación de la CSP en `tauri:dev` (§1.3), que se hará junto a la verificación manual final.

## 2. Fase 2 — Notificaciones y reservas

### 2.1 Esquema (migraciones 2 y 3)

- [x] Test (Rust): `notifications` acepta `entityType`/`entityId` juntos o ninguno y `audience` `all`/`admin`; `notificationWatches` rechaza duplicados; `invoices.reservedUntil` solo en V/S y posterior a la emisión.
- [x] Implementación: migraciones 2 y 3. La 2 reconstruye `notifications` (SQLite no añade `CHECK` a columnas existentes) conservando ids y datos; la 3 es un `ADD COLUMN` con su `CHECK`.

### 2.2 Generación y "una vez por situación" (NT-02, NT-03)

- [x] Test (Rust): `watch_enter`/`watch_leave`: entrar avisa una vez; seguir no repite; salir y volver avisa de nuevo; stock bajo → agotado genera N-02.
- [x] Test (Rust), una casilla por aviso con su mensaje exacto y prioridad:
  - [x] N-02 al quedar en 0 y al intentar vender un agotado (se inserta aunque la venta se rechace). Vender un producto en 0 se rechaza con `validationFailed` ("agotado"), no con `stockInsuficiente`: la evaluación aparte se hace tras cualquier salida rechazada (`after_rejected_outgoing`), y NT-03 evita repeticiones.
  - [x] N-03 con el umbral por defecto y con uno cambiado.
  - [x] N-04 (hoy a hoy + 29) y N-07 (< 10 días).
  - [x] N-05 (< 3 días) y N-06 (vencida). N-06 se prueba con `expire_reservations` (§2.4).
  - [x] N-10 (compra Por Pagar y venta Por Cobrar con más de los días configurados).
  - [x] N-11 (sin copia en los días configurados). `evaluate_backup_watch` está hecho y probado; **se conecta en la fase 5**, cuando existan las copias y `lastBackupAt`: llamarlo antes avisaría de copias que todavía no se pueden hacer.
  - [x] N-12 (purga) y N-13 (usuarios, `audience='admin'`). N-13 en crear, editar (por el Administrador) y eliminar; el alta pasa a una transacción para que usuario y aviso sean atómicos.
- [x] Implementación: `db/notifications.rs` (`notify`, `watch_*`), `automation.rs` (`run_startup_processes`, `before_operation`, `after_inventory_change`) y las llamadas desde productos, facturas y usuarios.
  - Avance: `notify`, `watch_enter`/`watch_leave`, `automation.rs` (`evaluate_watches` por conjuntos en SQL, `evaluate_watches_best_effort` con `SAVEPOINT`, `after_rejected_outgoing`, `run_startup_processes` con purga de notificaciones de más de 90 días). La evaluación va antes del commit de éxito de 9 operaciones de facturas y 4 de productos (no al recalcular la caché, para no avisar de un 0 transitorio). Umbrales por defecto (5 unidades, 30 días) hasta la fase 3 (`thresholds()`, marcado `shortcut:`). `before_operation`: `purge_best_effort` llama además a `expire_reservations_best_effort`, así que las reservas vencen al arrancar y antes de cada operación sin tocar cada comando.

### 2.3 Comandos y pestaña (NT-01, NT-04, TO-02)

- [x] Test (Rust): los 7 comandos de `notifications.rs` (listar con filtros, contar, desde un id, marcar una y todas, eliminar una y las filtradas); el Contador no ve ni cuenta las de `audience='admin'`; purga a los 90 días. Filas de la matriz de autorización.
- [ ] Test (Vitest, HTML de producción): la pestaña lista datos reales con prioridad y fecha; "Todas"/"No leídas", buscador y fecha filtran; clic marca leída y abre el producto o la factura (o avisa si ya no existe); "Marcar todas", eliminar una y todas sin confirmación; contador de cabecera y pestaña se actualizan.
- [ ] Test (Vitest): tras un comando de escritura, `notificationFeed` pide las nuevas y muestra un toast por cada una con su prioridad; al iniciar sesión, un único toast resumen.
- [x] Implementación: `notificationsService.js`, `notificationFeed.js`, evento `command:success` en `invoke.js` (con su test sin romper el existente), `notificacionesView.js`; retirar `notificacionesData` y `renderNotificaciones` de `dashboard.js`.
  - Hecho: `commands/notifications.rs` (7 comandos, búsqueda sin acentos y fecha local en Rust, "Eliminar todas" = las visibles con los filtros) y su fila en la matriz; purga de 90 días en `run_startup_processes`. En la pestaña: prioridad como etiqueta y color del icono (clases nuevas en `toast.css`), botón eliminar por fila y "Eliminar todas" sin confirmación, filas accesibles con teclado. El botón "Guardar tasas" del mockup, que creaba una notificación simulada, pasa a `avisoDemo` hasta la fase 3.

### 2.4 Reservas (RS-01 a RS-03)

- [x] Test (Rust): crear y editar venta/servicio con `reservedUntil` válido; rechazos (antes o igual a la emisión, más de 365 días, garantía/insumos/entrada/pérdida, estado no pendiente); cobrar o cancelar la desactiva; quitar la fecha la convierte en venta normal.
- [x] Test (Rust): `expire_reservations` cancela solo las vencidas, devuelve los consumos a sus lotes, registra la Reversión con "Sistema", genera N-06 y la entrada `expire`, y respeta el invariante de cachés; se ejecuta al arrancar y en `before_operation`.
- [x] Test (Rust): `listInvoices` con `status='reserved'`; `getInvoice` y `InvoiceListItem` con `reservedUntil`; `getProduct` con `reservedQuantity`.
- [x] Test (Vitest): campo "Reservar hasta" en venta y servicio (oculto si el estado no es Por Cobrar), se envía al crear y editar; filtro "Reservas activas" y columna de fecha límite; detalle con "Reservada hasta"; detalle del producto con la cantidad reservada y enlaces.
- [x] Implementación en `commands/invoices.rs`, `commands/products.rs`, `facturasView.js`, `productosView.js` y el HTML.
  - Hecho: `validate_reserved_until` (fecha válida, > emisión, ≤ emisión + 365, solo Por Cobrar); al editar, la fecha solo cambia si la factura sigue Por Cobrar (se conserva como dato histórico). `Author::{User, System}` en `auth.rs` (D-02) para la Reversión firmada por "Sistema" sin fabricar un `Actor`. `InvoiceListItem`/`InvoiceDetail` con `reservedUntil`; `ProductDetail` con `reservedQuantity` y `reservedInvoices` (enlaces). En la interfaz: campo "Reservar hasta" (oculto si no está Por Cobrar), filtro "Reservas activas", columna "Reservada hasta", tarjeta en el detalle de la factura y tarjeta "Reservado" en el detalle del producto.
  - **Pendiente para la fase 5:** la entrada `expire` del registro de actividad (RS-02) llega con la migración 7 (`activityLog`).

### 2.5 Cierre de la fase 2

- [x] Suites, lint, formato, `pendientes.md` (PEN-01, PEN-02, PEN-14, PEN-18) y commit. `cargo test` 399/399, `npm test` 347/347 (39 archivos), `clippy --all-targets -D warnings`, `cargo fmt --check` y `prettier --check .` limpios. En `pendientes.md` además: PEN-08 resuelto (N-02 al intentar vender) y PEN-12 parcialmente resuelto (fase 1).

## 3. Fase 3 — Configuración, impresión y conversiones

### 3.1 Esquema (migraciones 4 y 5)

- [x] Test (Rust): claves cerradas y rangos de `settings`; `userPreferences` por usuario y en cascada al borrar el usuario; tasas > 0; historial inmutable; columnas de unidad por línea juntas o ninguna.
- [x] Implementación: migraciones 4 y 5 con los valores por defecto. Los rangos de `settings` se declaran con un `CHECK` por clave (entero canónico); las monedas de `userPreferences`, con un `CHECK` que solo admite EUR, CUP, MLC y SALDO; la regla "juntas o ninguna" de la unidad por línea va en el `CHECK` de la última columna añadida (un `ALTER TABLE` no admite restricciones de tabla).

### 3.2 Ajustes, monedas y tasas (CF-01, CF-02, CF-04)

- [x] Test (Rust): `getSettings`/`updateSettings` (rangos; cambiar el umbral reevalúa N-03 y cambia el filtro "Stock bajo" y DASH-01); `getUserPreferences`/`updateUserPreferences` (monedas válidas, tamaño 10/20/50/100, cada usuario los suyos); `updateExchangeRate` (6 decimales, > 0) con historial y actividad; `listExchangeRateHistory`. Matriz de autorización.
- [x] Test (Vitest): pestañas de Configuración con datos reales (tarjetas de tasas, historial con orden, monedas por usuario, tamaño de página desde la base); el detalle del producto muestra los equivalentes de las monedas elegidas con la tasa actual; una moneda sin tasa no se muestra y Configuración lo avisa.
- [x] Implementación: `commands/settings.rs`, `settingsService.js`, `configuracionView.js`; el paginador global lee el tamaño de la base; retirar `monedasData` y los datos simulados de tasas de `dashboard.js`.
  - Hecho: `thresholds()` lee `settings`; el filtro "Stock bajo", `isLowStock` y N-03 usan el umbral guardado. El tamaño de página deja `localStorage`: `configuracionView.js` lo lee de la base y lo aplica con `establecerTamPagina` de `dashboard.js`. Tarjeta "Equivalentes" en el detalle del producto (`utils/currency.js`, mismo formato que `formatMoney`). Retirados `monedasData`, `historialTasasData`, `rateCardsData`, sus renderizadores, las acciones `avisoDemo`/`activarMoneda`/`actualizarEquivalencia` y los listeners de campo que ya no usaba nadie.
  - **Pendiente para la fase 5:** las entradas `config` del registro de actividad (tasas, umbrales, empresa, logo) llegan con la migración 7.

### 3.3 Empresa y logo (CF-03)

- [x] Test (Rust): `updateCompanyProfile` (longitudes); `saveCompanyLogo` acepta PNG/JPG reales de hasta 2 MB, rechaza otro contenido con extensión falsa, sustituye y borra el anterior, y con `null` lo quita; `getCompanyProfile` devuelve el logo como `data:` URL. Actividad `config`.
- [x] Test (Vitest): pestaña "Empresa" con los cuatro datos y el logo (elegido con el diálogo nativo; sin `onclick` ni la imagen de `placehold.co`); "Datos de Factura" según S-06.
- [x] Implementación en `settings.rs`, `configuracionView.js` y el HTML.
  - Hecho: S-06 aplicado (se elimina "Datos de Factura"); la pestaña "Facturación" pasa a "Empresa" y se añade "Umbrales". El logo se elige con `<input type="file">` (que abre el diálogo nativo del sistema), se convierte a JPEG en un `canvas` (D-17) y el backend lo valida por su contenido (firma y SOF). La `data:` URL se arma con un codificador base64 propio de 15 líneas para no añadir una dependencia directa.

### 3.4 Impresión y PDF (IM-01, IM-02)

- [x] Test (Rust): `build_invoice_document` para los seis tipos (título por tipo, contacto, líneas en la unidad escrita, total, asociada, "Reservada hasta", **sin estado**); `exportInvoicePdf` escribe un PDF válido con el logo y falla con `exportFailed` si no puede escribir. Matriz de autorización de `exportInvoicePdf` y `getInvoicePrintModel`.
- [x] Test (Vitest): "Imprimir" en los seis detalles abre la vista previa con el modelo y llama a `window.print()`; "Exportar a PDF" abre "Guardar como" y llama al comando; cancelar no hace nada; error como toast.
- [x] Implementación: `invoice_print.rs`, `printPreview.js`, `css/print.css`, botones en los seis detalles; retirar `exportarFactura*PDF` de `dashboard.js` y jsPDF.
  - Hecho: `invoice_print.rs` con un único `InvoicePrintModel` (D-05) para la vista previa y el PDF; el JPEG del logo se incrusta sin decodificar con `ImageFilter::DCT` (D-17) leyendo sus medidas del SOF (`jpeg_dimensions`, compartido con la validación del logo). Comandos `getInvoicePrintModel` y `exportInvoicePdf` (error `exportFailed`, que ya estaba en el catálogo). En la interfaz: "Imprimir" (vista previa A4 en `#print-area`, Escape la cierra, `window.print()` desde ella; `css/print.css` oculta lo demás al imprimir) y "Exportar a PDF" en los seis detalles. Retirados `jspdf.umd.min.js`, su `<script>`, el bloque `exportarFactura*PDF` y su test (`facturasPdf.test.js`); el entorno comprueba que no vuelva. Con §3.5 la impresión muestra la unidad escrita.

### 3.5 Conversiones en facturas (CV-01)

- [x] Test (Rust): `listUnitOptionsForProduct` (directa e inversa de un paso); compra de 2 Caja (×12) a 30,00 crea un lote de 24 u a 2,50; venta de 1 Caja descuenta 12 u al precio vigente × 12; redondeos de la cantidad convertida; sin conversión → `validationFailed`; el historial guarda la cantidad convertida; edición y anulación devuelven lo convertido.
- [x] Test (Vitest): selector de unidad por línea en los seis formularios (en la pérdida, junto a la cantidad); el total preliminar usa lo escrito; se envía `unitId`; el detalle y la impresión muestran "2 Caja".
- [x] Implementación en `inventory.rs`, `commands/invoices.rs`, `commands/products.rs`, `facturasView.js`.
  - Hecho: `resolve_line_unit`/`LineUnit` en `inventory.rs` (fracción exacta, directa o inversa de un paso; S-08). `quantityMilli` y `unitPriceCents` de la línea siguen en la unidad del producto, así que FIFO, lotes, historial, ediciones y cancelaciones no cambian; la línea guarda además unidad, cantidad escrita y factor (`store_line_unit`), el subtotal sale de lo escrito y el precio que se muestra es subtotal ÷ cantidad escrita. Las siete rutas (crear compra, venta/servicio, garantía/insumos y pérdida; editar compra, salidas con precio y a costo) convierten al leer la cantidad. `InvoiceLineDetail.entered` lleva lo escrito; el detalle, la edición y la impresión lo muestran. Una línea de producto eliminado se edita en la unidad base. Selector en el editor de líneas (solo si el producto tiene conversiones) y en la pérdida, junto a la cantidad.

### 3.6 Cierre de la fase 3

- [x] Suites, lint, formato, `pendientes.md` (PEN-06, PEN-07, PEN-19, PEN-20) y commit. `cargo test` 421/421, `npm test` 358/358 (39 archivos), `clippy --all-targets -D warnings`, `cargo fmt --check` y `prettier --check .` limpios. Pendiente para la verificación manual: el logo (conversión con `canvas` en el webview), el diálogo de impresión del sistema, el PDF con un logo real y el selector de unidad en `tauri:dev`.

## 4. Fase 4 — Dashboard

- [x] Test (Rust): `getDashboardKpis` con un escenario fijo (ventas y servicios cobrados y por cobrar, en días distintos; garantías, insumos y pérdidas; productos con margen distinto; un cliente eliminado) que verifica a mano cada cifra de DB-02 a DB-07 para cada período (hoy, semana, mes, 3 m, 6 m, 1 año, rango); ingresos por fecha de cobro; costo real FIFO; Pareto 80/20 de productos y clientes; lento movimiento con un producto nunca vendido (S-09). Matriz de autorización.
- [x] Test (Vitest, HTML de producción): con un DTO fijo, cada id del mockup (`data-kpi`, `dash-gasto-*`, `dash-estrella-*`, `dash-frecuentes`, `dash-vendidos-*`, `dash-lento`, `dash-chart`) muestra su valor; el selector de período tiene las siete opciones y recarga; los equivalentes en otras monedas aparecen según las preferencias; estado de error con "Reintentar"; tarjeta de reservas activas con enlaces; se recalcula al volver al dashboard tras guardar.
- [x] Implementación: `commands/dashboard.rs`, `dashboardService.js`, `dashboardView.js` y el HTML (opciones de período y tarjeta de reservas sin cambiar el diseño).
- **Decisiones de implementación:** el total de "Resumen de Gastos" incluye las pérdidas (línea aparte) para coincidir con la tarjeta "Total de Gastos"; el valor del inventario y los agotados no tienen historial, así que no muestran variación ni minigráfico; con período anterior en cero se muestra "Sin comparación" en vez de un porcentaje; los renderizadores pasan de `dashboard.js` a `dashboardView.js`, con DOM.
- [x] Cierre: suites, lint, formato, `pendientes.md` (PEN-16) y commit.

## 5. Fase 5 — Borradores, registro de actividad, CSV y respaldo

### 5.1 Esquema (migraciones 6 a 8)

- [x] Test (Rust): `drafts` único por (usuario, formulario, registro) y en cascada; `activityLog` inmutable y sin borrado; `application_id` fijado.
- [x] Implementación: migraciones 6, 7 y 8.

### 5.2 Borradores (BR-01 a BR-03)

- [x] Test (Rust): `saveDraft` hace `UPSERT`; cada usuario ve solo los suyos; `formKey` fuera de la lista se rechaza; un payload con claves de contraseña se rechaza; caducidad a los 30 días al arrancar. Matriz de autorización de los 4 comandos.
- [x] Test (Vitest): `attachDraft` guarda con *debounce* de 1 s, nunca incluye campos `password` ni `file`, se borra al guardar o cancelar; al abrir un formulario con borrador pregunta y "Continuar" lo carga / "Descartar" lo borra; una factura recuperada revalida contacto y productos; toast al iniciar sesión; sección "Borradores" lista, abre y elimina.
- [x] Implementación: `commands/drafts.rs`, `draftsService.js`, `utils/drafts.js`, `borradoresView.js`, y `attachDraft` en cada formulario (una casilla por vista, como en §1.2).

### 5.3 Registro de actividad (RA-01, RA-02)

- [x] Test (Rust), una casilla por grupo: eliminaciones de cada entidad; cancelaciones (manual y por vencimiento, con "Sistema"); ediciones de facturas; cambios de configuración; respaldos, restauraciones e importaciones; purga. Cada entrada se escribe en la misma transacción (si la acción falla, no hay entrada) y sin datos personales.
- [x] Test (Rust): `listActivityLog` con filtros y orden; Contador → rechazado.
- [x] Test (Vitest): la sección solo aparece al Administrador; filtros, búsqueda y orden; texto seguro.
- [x] Implementación: `activity.rs`, llamadas desde todos los comandos de RA-01, `activityService.js`, `actividadView.js`.

### 5.4 CSV (CSV-01, CSV-02)

- **Desviación de D-14 (justificada):** no hay `SAVEPOINT` por fila ni confirmación conjunta. Cada `create_*_inner` abre su propia transacción y SQLite no admite un `BEGIN` dentro de un `SAVEPOINT`; cada fila es atómica por sí sola y una omitida no deja la categoría o unidad que creó. Si la aplicación se cierra a mitad de una importación, las filas ya procesadas quedan guardadas.
- **Decisión:** el CSV de productos no trae abreviatura; la unidad nueva toma los 10 primeros caracteres de su nombre. Los archivos solo se aceptan en UTF-8 ("CSV UTF-8" en Excel).

- [x] Test (Rust): `exportCsv` por cada tipo respeta los filtros, escribe BOM, `;`, decimales con coma, fechas `dd/mm/aaaa` y neutraliza fórmulas.
- [x] Test (Rust): `importCsv` de productos (crea categoría y unidad que falten, lote inicial y movimiento Inicial), categorías, unidades, clientes y proveedores; filas duplicadas o inválidas se omiten con número y motivo y las válidas se guardan; archivo vacío, encabezados desconocidos o más de 5 000 filas se rechazan; actividad `import`. Matriz de autorización.
- [x] Test (Vitest): botones de exportar en los listados (con "Guardar como") y de importar en Configuración (con "Abrir"); resumen del resultado como toast con detalle de las filas omitidas.
- [x] Implementación: `csv_io.rs`, `csvService.js`, botones en las vistas.

### 5.5 Respaldo y restauración (RP-01 a RP-03)

- **Decisiones:** la restauración prepara la copia en un archivo aparte (migración, sesiones y entrada del registro) y solo entonces sustituye la base, así un fallo no toca los datos actuales. La copia previa (`goyma-previa-*`) aparece en la lista para poder volver atrás y no cuenta entre las 7 automáticas. La copia diaria también actualiza `lastBackupAt`; N-11 se evalúa al arrancar y tras cada copia.

- [x] Test (Rust): `createBackup` produce una base válida con todos los datos, actualiza `lastBackupAt` y registra actividad; la copia diaria crea una al día y conserva 7; `listAutomaticBackups`.
- [x] Test (Rust): `restoreBackup` con contraseña incorrecta no toca nada; con un archivo ajeno, corrupto o de versión mayor → `invalidBackup` sin tocar nada; con una copia de versión menor la migra; hace copia previa de lo actual, sustituye la base, borra las sesiones y registra la restauración. Matriz de autorización.
- [x] Test (Vitest): pestaña "Respaldo": crear copia (aviso de que no está cifrada), lista de automáticas, restaurar con confirmación y contraseña y vuelta al login.
- [x] Implementación: `backup.rs` y su llamada en `run_startup_processes`, `backupService.js`, `configuracionView.js`.

### 5.6 Cierre del módulo y del MVP

- [x] Revisión contra `spec-04` requisito por requisito (MG, TO, SG, NT, RS, CF, IM, CV, DB, BR, RA, CSV, RP) y contra la trazabilidad de la §12.
- [x] Matriz de autorización: los 31 comandos nuevos con su fila; `listActivityLog` con la de Contador rechazado.
- [x] Contrato `invoke ↔ generate_handler!`: **107 comandos**, sin `pendingWrapper`.
- [x] `environment.test.js`: sin manejadores inline, sin `innerHTML` con datos, sin jsPDF, sin datos simulados en `dashboard.js`, CSP presente.
- [x] Documentación: `Agents.md` §1 (las cuatro specs implementadas) y §7; `pendientes.md` (PEN-01, 02, 06, 07, 12 a 16, 18 a 22 resueltos o con su estado); Constitution v1.3.0 en vigor.
- [ ] **Verificación manual de extremo a extremo** de `spec-01`, `spec-03` y `spec-04` en `tauri:dev` y `tauri:build`, como Administrador y como Contador: toasts en cada flujo; notificaciones y reservas (incluido un vencimiento cambiando la fecha del equipo o la fecha límite); imprimir en papel y exportar a PDF; adjuntar un PDF de compra con el diálogo y arrastrándolo; compra con producto nuevo y con conversión; dashboard con datos; borrador recuperado tras cerrar la app a la fuerza; CSV abierto y guardado en Excel en español e importado; crear una copia y restaurarla en otra carpeta de datos.
- [ ] Suites finales con recuentos, lint, formato y commit.
