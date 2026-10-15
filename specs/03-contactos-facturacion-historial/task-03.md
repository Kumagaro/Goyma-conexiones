# Tareas — Contactos, Facturación e Historial de Productos

- **Basado en:** `spec-03.md` y `plan-03.md` (esta misma carpeta), `Constitution.md` v1.2.0, `Agents.md`, `spec-01` (autenticación y usuarios), `spec-02` (inventario y productos), `pendientes.md`, y el frontend real `dashboard-unificado.html` + `dashboard.js`.
- **Estado:** **Confirmado / listo para implementar.** Incorpora todas las decisiones del dueño (A1–A2, B1–B12, C1–C12) y las 27 decisiones D-01 a D-27 de `plan-03.md` §10. Correcciones de la primera revisión QA (H-1 a H-13) aplicadas. **Revisiones posteriores:** aplicados H-1 a H-11 y O-1 a O-4 (`plan-03 §10.4`), N-1 a N-4 y O-B (`plan-03 §10.5`), y H-A1 a H-A4 de la auditoría de inventario (`plan-03 §10.6`). Incluye la §1.5 "Esquema", la §1bis.5 "Evento `page:change`", la §5bis "Tests transversales de `expiredCleanupWarning`", los tests del tope de stock, del escenario H-A1 y del cambio de costo por re-FIFO sin Ajuste.
- **Enfoque:** vertical (por funcionalidad, de punta a punta: test de Rust → comando → servicio/vista JS) dentro de cada fase, y por fases según `spec-03` §3.3. La fase 1 no depende de las demás; la fase 2 requiere la 1 (los proveedores son prerequisito de las facturas de entrada); la fase 3 requiere la 2 (las salidas consumen los lotes); la fase 4 es independiente una vez que existe el historial.
- **Convención de nombres:** los comandos se nombran en `camelCase` (nombre lógico / wrapper JS); la función Rust es su equivalente `snake_case` (`createEntryInvoice` → `create_entry_invoice`). Sin `#[allow(non_snake_case)]`.
- **Estructura de los comandos:** cada comando es un envoltorio delgado (lock + `now` + [`today`] + llamada + registro de error) sobre `xxx_inner(conn: &mut Connection, now, [today,] …)`. `today` lo calcula el envoltorio con `chrono::Local::now().date_naive()`; **no es un argumento que envíe el frontend** (`plan-03.md` §4). **Los tests de Rust ejercitan la función interna** sobre `setup_test_db()` (`spec-02` §1.6) o `setup_test_db_v3()` si el test necesita los lotes.
- **Granularidad:** una tarea de test por requisito, cubriendo todos sus escenarios Dado/Cuando/Entonces de la spec. Cuando una tarea agrupa más de ~8 reglas, se divide en **sub-casillas** (`- [ ]` anidadas) por grupo de reglas, y la tarea padre solo se marca al completarlas todas.
- **Garantía sobre lo implementado:** no se reabre ninguna tarea marcada [x] de `tasks-01.md` ni `tasks-02.md`. Los cambios a `spec-02` que `spec-03` ordena se implementan desde aquí (§3.5 y §3.6), no reabriendo las tareas originales.
- **Orden dentro de cada tarea de código:** Test → Implementación → Verificación. Cada test debe fallar antes de implementar el código que lo satisface (`Constitution.md` §2.III), y ninguna implementación hace trampa para pasarlo.
- **Registro de comandos:** cada comando se añade a `generate_handler!` **en el mismo commit** en que se implementa. Son **35 comandos nuevos** (15 de `spec-01` + 26 de `spec-02` = 41 existentes + 35 = **76** al final). Ver §4.1 de `plan-03.md`.
- **Matriz de autorización (`Agents.md` §3):** cada comando trae su fila en la misma tarea que lo implementa: sin sesión → `sessionExpired`; con Administrador y con Contador → permitido. La verificación de que ninguno quedó fuera está en la §6.
- **Nota general:** ninguna tarea se marca completa sin verificar que el código funciona de verdad, no solo que compila o que el test pasa (`Agents.md` §6). El ítem "Actualizar documentación" no se repite en cada sección: se consolida en la §6.
- **Nota de contexto (estado de la interfaz):** hoy Clientes, Proveedores y las 7 páginas de facturas trabajan con datos simulados en `dashboard.js`. Cada sección del módulo elimina **su** mock en el mismo bloque en que conecta la vista real, para que nunca estén activas a la vez la lógica simulada y la real compitiendo por los mismos elementos del DOM. El resto de `dashboard.js` (dashboard, notificaciones, configuración) queda fuera de alcance.

---

## 0. Verificaciones previas y preparación

*(Nada de esto crea funcionalidad; evita arrancar sobre supuestos sin comprobar.)*

- [ ] Decisiones de `plan-03.md` §10 confirmadas por el dueño (A1–A2, B1–B12, C1–C12 y D-01 a D-27). **Ya confirmadas.**
- [ ] Línea base: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check` y `npm test` pasan **antes** de tocar nada. Números de partida esperados: 97 tests Rust de `spec-01` + tests de `spec-02`; 53+ tests Vitest. Anotar los recuentos reales.
- [ ] Fijar versiones y *features* de `rust_xlsxwriter`, `printpdf` y `uuid` en `Cargo.toml`. Comprobar que ninguna arrastra `tokio` ni un motor de imágenes pesado y que `cargo clippy` y `cargo build` siguen sin warnings.
- [ ] Confirmar que `dialog:default` en `capabilities/default.json` incluye `save` y `open`; que `window.__TAURI__.dialog` existe con `withGlobalTauri: true`. Si no, añadir solo `dialog:allow-save` y `dialog:allow-open` (no el plugin completo).
- [ ] **Spike de triggers (D-17):** en una base de prueba, comprobar que (a) `BEFORE UPDATE OF …` sobre `productMovements` **no** se dispara cuando una clave foránea `ON DELETE SET NULL` anula `productId`, `invoiceId` o `userId`; (b) `BEFORE DELETE` bloquea un `DELETE` directo; (c) borrar un producto, una factura y un usuario con movimientos funciona. **Anotar en `plan-03.md` §2** el texto exacto del error de `RAISE(ABORT)` para los tests.
- [ ] Anotar las firmas reales de lo que este módulo reutiliza: `Actor` (campos), `round_half_up`, `purge_best_effort`, `map_db_error`, `validate_name`, `validate_description`, `parse_cost_cents`, `parse_quantity_milli`, `format_milli` (cantidad → texto con coma), `insert_product_fixture`, `assert_requires_session` y `assert_allows_both_roles`. Si `round_half_up` o `format_milli` son privados, pasan a `pub(crate)` sin cambiar su comportamiento.
- [ ] Confirmar la conversión `chrono::Local` ↔ UTC en un día de cambio de hora (hueco y repetición) con `earliest()`/`latest()`. Anotar el resultado.
- [ ] **Spike de `printpdf` (D-18):** generar un PDF de prueba con Helvetica integrada y comprobar que pinta `á é í ó ú ü ñ ¿ ¡ €`. Anotar el resultado en `plan-03.md` §3.10.
- [ ] **Borrar la base de desarrollo** antes de la primera ejecución: los productos creados con `spec-02` no tienen lote inicial, y la igualdad entre las cachés y los lotes (`plan-03.md` §3.2) no se cumpliría. `Constitution.md` §4 lo permite mientras no haya datos reales.
- [ ] **Inventario de `dashboard.js` y `dashboard-unificado.html`** — levantar contra los archivos reales con `grep`. **Sin números de línea** (se desplazan con cualquier edición). Confirmar los IDs y funciones de `spec-03` §9.X.4.a y §9.X.4.b. El `grep` es la fuente de verdad.
- [ ] **Verificación del sidebar de facturas (C-6):** con `grep -n 'data-page' dashboard-unificado.html`, anotar el `id` o `data-page` **real** del enlace del submenú de facturas. No asumir el valor; documentarlo en `plan-03 §7.10`. Se usa en §4.5.9 y §6.
- [ ] **Reconocimiento de `innerHTML` (O-2):** con `grep -c 'innerHTML'` sobre `dashboard.js`, contar las asignaciones de `innerHTML` que interpolan datos del usuario, por vista (facturas, clientes, proveedores, historial). Anotar el número por vista. Alimenta la estimación de esfuerzo de §4.5.7.
- [ ] Confirmar que los módulos de `spec-01`/`spec-02` que este módulo importa existen y están cargados: `services/invoke.js`, `services/authService.js`, `services/usersService.js`, `services/categoriesService.js`, `services/unitsService.js`, `services/productsService.js`, `services/dashboardService.js`, `utils/formatters.js`, `utils/requestGuard.js`, `utils/formValidation.js`, `components/combobox.js`, `components/confirmDialog.js`, `views/categoriasView.js`, `views/unidadesView.js`, `views/productosView.js`, `views/dashboardView.js`, `session.js`.
- [ ] Confirmar los **26 comandos** de `spec-02` implementados y sus tests en verde. Anotar el recuento de tests Rust de `spec-02`.
- [ ] Revisar `services/invoke.js` y su test actual: anotar el comportamiento vigente ("devuelve intactas respuestas/errores de otros códigos") que debe seguir en verde.
- [ ] Revisar el test de contrato `invoke` ↔ `generate_handler!` (`environment.test.js`): confirmar que el recuento esperado se calcula a partir de los archivos y no está fijo en 41. Si está fijo, se actualiza en la §3.9.
- [ ] Actualizar `Agents.md` §7 con los comandos nuevos de desarrollo/build si cambian.

---

## 1. Infraestructura compartida (backend)

*(Se crea antes de la primera tarea funcional: todos los módulos la usan. Orden: Test → Implementación por pieza.)*

### 1.1 `text.rs` — ampliación

- [x] Test (Rust): `is_valid_email("a@b.co")` → true; `is_valid_email("a@b")` → false (sin TLD); `is_valid_email("a b@c.com")` → false (espacio); `is_valid_email("a@b.c")` → false (TLD de 1 carácter); `is_valid_email("a@b.com")` → true; `is_valid_email("")` → false; `is_valid_email` con `á` → false (ASCII); también prueba la frontera de 100 caracteres definida en la spec.
- [x] Implementación: `is_valid_email(raw: &str) -> bool` en `text.rs`. Patrón `^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$`, limitado a 100 caracteres, sin dependencia nueva.

### 1.2 `timestamps.rs` — ampliación

- [x] Test (Rust): `today_local` con un `now` inyectado devuelve la fecha local correcta. `parse_invoice_date("2026-02-30")` → `Err`; `parse_invoice_date("2026-1-1")` → `Err`; `parse_invoice_date("1999-12-31")` → `Err`; `parse_invoice_date("2026-01-15")` → `Ok`. `is_future(2026-01-16, 2026-01-15)` → true.
- [x] Implementación: `today_local`, `parse_invoice_date`, `is_future` en `timestamps.rs`. Se reutilizan `format_utc_timestamp` y `parse_utc_timestamp` de `spec-02`.

### 1.3 `validators.rs` — ampliación

- [x] Test (Rust, `validate_client_input` / `validate_supplier_input`): nombre/apellidos de 2–50; empresa/cargo hasta 100; correo válido con `is_valid_email`; dirección/información de factura hasta 200; CI con `validate_national_id`; teléfono con `validate_phone`; caracteres de control/formato rechazados con el mensaje de `spec-02` §4. `validate_client_input` devuelve el valor limpio y su versión normalizada.
- [x] Test (Rust, `validate_invoice_notes`): vacío → `None`; 1–500 caracteres; > 500 rechazado con el mensaje de `spec-02`.
- [x] Test (Rust, `validate_attachment_original_name`): rechaza nombres con `/` o `\`, longitud > 100 o < 1.
- [x] Implementación: `validate_client_input`, `validate_supplier_input`, `validate_invoice_notes`, `validate_attachment_original_name` en `validators.rs`. Se **amplía**, sin tocar lo existente.

### 1.4 `errors.rs` — ampliación y política de registro con throttle (H-A2)

- [x] Test (Rust): los códigos nuevos (`invalidState`, `stockInsuficiente`, `loteEnUso`, `attachmentInvalid`, `stockLimitExceeded`, `exportFailed`) se serializan como `{ code, message }` sin `cause`. **Los códigos existentes se serializan igual que antes** (test de regresión).
- [x] Test (Rust, **`should_log` ampliado**): `should_log()` devuelve `false` para `invalidCredentials` y `validationFailed`; `true` para `stockInsuficiente`, `loteEnUso`, `attachmentInvalid`, `stockLimitExceeded`, y `true` para `duplicateField`, `notFound`, `invalidState`, `exportFailed`, `internal`.
- [x] Test (Rust, **H-A2 / throttle del registro**): con un reloj inyectado, dos `stockInsuficiente` consecutivos del mismo `(code, userId)` dentro de 60 s producen 1 sola fila en `errorLogs`; a los 61 s, la siguiente ocurrencia vuelve a registrar; con `userId` distinto, se registra; `validationFailed` e `invalidCredentials` no se registran nunca. El `HashMap` del throttle se limpia al superar la ventana o el tamaño máximo (1000 entradas).
- [x] Test (Rust, **H-A2 / mismo comportamiento para los cuatro códigos con throttle**): `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded` comparten el mecanismo; `duplicateField`, `notFound`, `invalidState`, `exportFailed` e `internal` se registran siempre.
- [x] Implementación: ampliar `errors.rs` con los códigos nuevos, las constantes con los textos exactos de `plan-03.md` §4.3, y el `HashMap<(String, i64), DateTime<Utc>>` bajo `Mutex` con el throttle de 60 s. `AppError::should_log()` ampliado: los cuatro códigos con throttle devuelven `true`. La función `log_error` consulta el throttle antes de escribir.

### 1.5 Esquema (`db/schema.rs`)

*(Todas las tablas, índices y triggers de `plan-03.md` §2. Un test por cada `CHECK`, `UNIQUE`, clave foránea y trigger, con un valor que fallaría por `NULL` donde aplique. Los `ts_check`/`ts_check_opt` se prueban en §1.2; aquí se prueba que el esquema los aplica.)*

- [x] Implementación: ampliar `init_schema` con las tablas `clients`, `suppliers`, `invoiceCounters`, `invoices`, `invoiceLines`, `lots`, `consumptions` y `productMovements`; los índices `invoicesTypeDate`, `invoicesClient`, `invoicesSupplier`, `invoiceLinesProduct`, `consumptionsLot`, `consumptionsLine`, `productMovementsProduct`, `lotsOneInitial` y `productsActiveName`; y los triggers `invoiceCountersNoDecrease`, `invoiceCountersNoDelete`, `productMovementsImmutable` y `productMovementsNoDelete`. Todas con `CREATE ... IF NOT EXISTS`. Insertar las seis filas de `invoiceCounters` con `INSERT OR IGNORE`. Sin datos semilla en las demás tablas (7.8).

#### 1.5.1 `clients`

- [x] Test (Rust, SQL puro): `nationalId` duplicado falla por `UNIQUE`; **el mismo `phone` en dos clientes pasa** (el teléfono no es único).
- [x] Test (Rust): `firstName`/`lastName` de 1 y 51 caracteres fallan, de 2 y 50 pasan. `nationalId` de 10, 12 y con letras falla, de 11 dígitos pasa. `phone` sin `+53`, con 7 o 9 dígitos falla. `email` de 3 y 100 caracteres pasa, de 2 y 101 falla. `company`/`position` a 100/101; `address`/`invoiceInfo` a 200/201.
- [x] Test (Rust): `createdAt`/`updatedAt` con formato incorrecto fallan (`GLOB` + `strftime` ida y vuelta, Q-05); con el formato correcto pasan. Incluye fechas imposibles (`2026-13-01`, `2026-02-30`).

#### 1.5.2 `suppliers`

- [x] Test (Rust, SQL puro): igual que `clients` **sin la columna `invoiceInfo`** (verificar que no existe).

#### 1.5.3 `invoiceCounters`

- [x] Test (Rust, SQL puro): tras `init_schema` existen las seis filas con `nextNumber = 1`. `init_schema` dos veces no duplica filas.
- [x] Test (Rust, SQL puro): `type` fuera de la lista falla por `CHECK`. `nextNumber = 0` falla.
- [x] Test (Rust, **trigger `invoiceCountersNoDecrease`**): un `UPDATE` que reduzca `nextNumber` falla con `RAISE(ABORT)`; uno que lo aumente pasa. Anotar el texto exacto.
- [x] Test (Rust, **trigger `invoiceCountersNoDelete`**): un `DELETE` directo falla con `RAISE(ABORT)`. Anotar el texto exacto.

#### 1.5.4 `invoices`

- [x] Test (Rust, SQL puro): `type` fuera de la lista falla. `number` que no empieza por `type || '-'` falla. `number` duplicado falla.
- [x] Test (Rust): `invoiceDate` con formato incorrecto, `2026-02-30`, y anterior a `2000-01-01` fallan; `2000-01-01` y una fecha válida pasan. La "no futura" la valida el código, no el `CHECK`.
- [x] Test (Rust, **`CHECK` de estado/pérdida**): `type = 'P'` con `status = 'pending'` falla; `type = 'P'` con `status = NULL` pasa; `type = 'E'` con `status = NULL` falla; `type = 'E'` con `status = 'x'` falla.
- [x] Test (Rust, **`CHECK` de pago/cobro**): `paidAt` con `type = 'P'` falla; con `type = 'E'` y `status = 'pending'` falla; con `type = 'E'` y `status = 'completed'` pasa; `type = 'E'`, `status = 'completed'`, `paidAt = NULL` falla (tercer `CHECK`).
- [x] Test (Rust, **`CHECK` de contacto**): `type = 'P'` con `contactFullName` no nulo falla; `type = 'E'` con `contactFullName = NULL` falla; `type = 'E'` con `contactFullName`, `contactNationalId` y `contactPhone` no nulos pasa.
- [x] Test (Rust, **`CHECK` de FK**): `clientId` no nulo con `type = 'E'` falla; `supplierId` no nulo con `type = 'V'` falla.
- [x] Test (Rust, **`CHECK` de factura asociada**): `type = 'GG'` sin `associatedNumber` falla; `type = 'GG'` con `associatedType = 'V'` pasa; `type = 'GI'` con `associatedType = 'V'` falla; `type = 'GI'` con `associatedType = 'S'` pasa; `type = 'V'` con `associatedInvoiceId` no nulo falla.
- [x] Test (Rust, **`CHECK` de adjunto**): `attachmentFileName` no nulo con `type = 'V'` falla; con `type = 'E'` y los otros tres campos en `NULL` falla; con los cuatro campos y `attachmentSizeBytes = 5242881` falla; con `5242880` pasa.
- [x] Test (Rust): `createdAt`/`updatedAt` con formato incorrecto fallan; con el correcto pasan.

#### 1.5.5 `invoiceLines`

- [x] Test (Rust, SQL puro): `position` duplicado para la misma `invoiceId` falla por `UNIQUE(invoiceId, position)`. `productIdOriginal` duplicado para la misma `invoiceId` falla por `UNIQUE(invoiceId, productIdOriginal)`.
- [x] Test (Rust): `quantityMilli = 0` falla, `1000000000` falla, `1` y `999999999` pasan. `unitPriceCents = -1` falla, `0` y `9999999999` pasan. `subtotalCents` fuera de rango falla.
- [x] Test (Rust, **`CHECK` de unidad**): `unitName` no nulo con `unitAbbreviation = NULL` falla; los dos en `NULL` pasan; los dos no nulos pasan.
- [x] Test (Rust, **`CHECK` de categoría**): `categoryId` no nulo con `categoryName = NULL` falla; los dos no nulos pasan; los dos en `NULL` pasan.
- [x] Test (Rust, `ON DELETE CASCADE`): borrar la factura borra sus líneas.
- [x] Test (Rust, `ON DELETE SET NULL`): borrar el producto deja `productId = NULL` y conserva `productIdOriginal`.

#### 1.5.6 `lots` y triggers de `productMovements`

- [x] Test (Rust, SQL puro): `initialMilli = 0` falla; `remainingMilli` mayor que `initialMilli` falla; `remainingMilli = -1` falla.
- [x] Test (Rust): `origin = 'entry'` sin `invoiceLineId` falla; `origin = 'initial'` con `invoiceLineId` no nulo falla; `origin = 'restitution'` sin `invoiceLineId` pasa.
- [x] Test (Rust, **`UNIQUE(productId, arrivalOrder)`**): dos lotes del mismo producto con el mismo `arrivalOrder` fallan.
- [x] Test (Rust, **índice `lotsOneInitial`**): dos lotes `origin = 'initial'` para el mismo producto fallan.
- [x] Test (Rust, `ON DELETE CASCADE`): borrar el producto borra sus lotes. Borrar la línea de compra borra su lote.
- [x] Test (Rust, **trigger `productMovementsImmutable`**): un `UPDATE` directo de `createdAt`, `productName`, `type`, `quantityMilli`, `stockAfterMilli`, `unitPriceCents`, `totalPriceCents`, `invoiceNumber`, `invoiceType` o `userName` falla con `RAISE(ABORT)`. **No bloquea** un `UPDATE` de `productId`, `invoiceId` o `userId` (FK `SET NULL`). Anotar el texto exacto.
- [x] Test (Rust, **N-4 / `UPDATE` mixto**): un `UPDATE` mixto (`SET productId = NULL, quantityMilli = 0`) **falla** por la parte inmutable (`quantityMilli`); solo se permite un `UPDATE` que toque exclusivamente `productId`, `invoiceId` o `userId`.
- [x] Test (Rust, **trigger `productMovementsNoDelete`**): un `DELETE` directo falla con `RAISE(ABORT)`. Anotar el texto.
- [x] Test (Rust, **interacción FK/trigger**): borrar un producto con movimientos **funciona** (el trigger no se dispara en el `SET NULL`); borrar una factura con movimientos **funciona**; borrar un usuario con movimientos **funciona**. Cada uno deja el vínculo en `NULL` y la copia intacta.

#### 1.5.7 `consumptions`

- [x] Test (Rust, SQL puro): `quantityMilli = 0` falla; `unitCostCents = 0` falla; valores en rango pasan.
- [x] Test (Rust, `ON DELETE CASCADE`): borrar la línea de salida borra sus consumos.
- [x] Test (Rust, `ON DELETE SET NULL`): borrar el lote deja `lotId = NULL` y conserva `unitCostCents`.

#### 1.5.8 `productMovements` (validaciones de columnas)

- [x] Test (Rust, SQL puro): `type` fuera de la lista falla. `quantityMilli = -999999999` y `999999999` pasan; `-1000000000` y `1000000000` fallan.
- [x] Test (Rust, **`CHECK` de tipo/factura**): `type = 'initial'` con `invoiceNumber` no nulo falla; `type = 'initial'` sin `invoiceNumber` pasa; `type = 'entry'` sin `invoiceNumber` falla.
- [x] Test (Rust, `ON DELETE SET NULL`): borrar el producto, la factura y el usuario dejan los vínculos en `NULL` y las copias (`productIdOriginal`, `productName`, `invoiceNumber`, `invoiceType`, `userName`) intactas.

#### 1.5.9 Verificación de la §1.5

- [x] `init_schema` ejecutado dos veces no falla y no duplica filas en `invoiceCounters`.
- [x] Las tablas de `spec-01` y `spec-02` no se han tocado.
- [x] `init_schema` no inserta datos semilla en ninguna tabla nueva (excepto `invoiceCounters`, que es estructural).
- [x] Arrancar la aplicación con un descontinuado vencido **no** lo purga (P-08, coherente con `spec-02`).

### 1.6 Modelos / DTOs

- [x] Crear `models/client.rs`, `supplier.rs`, `invoice.rs`, `invoice_line.rs`, `lot.rs`, `consumption.rs`, `product_movement.rs` con los DTOs de `plan-03.md` §4.2 (todos con `#[serde(rename_all = "camelCase")]`; los structs de entrada con `#[serde(deny_unknown_fields, rename_all = "camelCase")]`).
- [x] Test (Rust): cada DTO serializa a las claves `camelCase` esperadas; los precios viajan como enteros de centavos; las cantidades como texto con coma; los timestamps como ISO 8601 UTC; las fechas de factura como `YYYY-MM-DD`.
- [x] Test (Rust): los structs de entrada (`CreateEntryInvoiceInput`, `UpdateEntryInvoiceInput`, etc.) **fallan al deserializarse** con campos prohibidos (`sellingPriceCents` al crear entrada; `quantity`, `cost`, `sellingPrice` al editar) y pasan con los válidos.
- [x] Implementación: los structs y DTOs de `plan-03.md` §4.2.

### 1.7 `state.rs` — estados y transiciones

- [x] Test (Rust): `InvoiceStatus::from_str("pending")` → `Ok(Pending)`; `"completed"` → `Ok(Completed)`; `"cancelled"` → `Ok(Cancelled)`; `"x"` → `Err`. `InvoiceType::from_str("E")` → `Ok(E)`; `"P"` → `Ok(P)`; `"X"` → `Err`. `as_str` es inversa de `from_str`.
- [x] Test (Rust, `validate_transition`): todas las transiciones permitidas de `spec-03` §7.5 pasan; las prohibidas devuelven `invalidState` con el mensaje correspondiente. El mismo estado → `invalidState` con "La factura ya está en ese estado". Cancelada → cualquiera → `invalidState` con "Una factura cancelada no se puede reactivar ni editar".
- [x] Test (Rust, `validate_edit_allowed`): entrada `pending` → `Ok`; entrada `completed` → `Err` con "Solo se pueden editar las facturas que están Por Pagar"; venta/servicio `pending` → `Ok`; venta/servicio `completed` → `Err` con "Solo se pueden editar las facturas que están Por Cobrar"; garantía/insumo `pending` → `Ok`; garantía/insumo `completed` → `Err` con "Solo se pueden editar las facturas que están Pendiente"; pérdida (`status = null`) → `Err`.
- [x] Test (Rust, `validate_status_payload`): `paymentMethod` con `status = pending` → `Err`; con `completed` en tipo E/V/S → `Ok`; con `completed` en tipo GG/GI → `Err`; `paidAt` con `status = pending` → `Err`; con `completed` → `Ok`.
- [x] Implementación: `state.rs` con `InvoiceStatus`, `InvoiceType`, `validate_transition`, `validate_edit_allowed`, `validate_status_payload`.

### 1.8 `invoice_counter.rs` — numeración

- [x] Test (Rust): `next_invoice_number(conn, 'E')` con el contador en 1 devuelve `"E-00001"` y deja el contador en 2. Segunda llamada devuelve `"E-00002"`. Contador en 99999 devuelve `"E-99999"` y deja en 100000. Contador en 100000 devuelve `"E-100000"` (6 dígitos). Los contadores de tipos distintos son independientes.
- [x] Test (Rust): el trigger `invoiceCountersNoDecrease` bloquea un `UPDATE` que reduzca `nextNumber`. El trigger `invoiceCountersNoDelete` bloquea un `DELETE`.
- [x] Implementación: `next_invoice_number(conn, type) -> Result<String, AppError>`. Se llama siempre dentro de la transacción que crea la factura.

### 1.9 `inventory.rs` — lotes, FIFO, cachés, tope y reversión

- [x] Test (Rust, `next_arrival_order`): primer lote de un producto → 1; segundo → 2; tras eliminar el último, el siguiente no reutiliza el orden.
- [x] Test (Rust, `next_restitution_order`): con un lote de orden 1 → 0; con un lote de orden 0 → -1; con varios → `MIN - 1`.
- [x] Test (Rust, `consume_fifo`), sub-casillas:
  - [x] Consumo de un solo lote completo.
  - [x] Consumo parcial de un solo lote.
  - [x] Consumo que cruza varios lotes (el ejemplo de LT-02: 10 a $1, 5 a $3, 8 a $2, vender 12 → consume 10 del primero y 2 del segundo).
  - [x] Consumo que agota el último lote disponible.
  - [x] Consumo mayor que el stock total → `Err` con código `stockInsuficiente` y mensaje "No hay stock suficiente de «X» (disponible: N)".
  - [x] Consumo con lotes de orden negativo (restitución) → consume primero el de orden más bajo.
  - [x] Los consumos se insertan en `consumptions` con el `unitCostCents` del lote en ese momento.
- [x] Test (Rust, `line_total_cost_cents`): suma `quantityMilli × unitCostCents` y redondea una vez. Ejemplo del plan: 10 × 100 + 2 × 300 = 1600. Con varios consumos: 5 × 100 + 3 × 250 = 1250. Con redondeo: el resultado es exacto en centavos.
- [x] Test (Rust, `line_unit_cost_cents`): `round_half_up(total / (quantity_milli / 1000))`. Ejemplo: 1600 centavos / (12000/1000) = 133,33 → 133 centavos. Es la **única función** que produce el `unitPriceCents` de líneas de garantía/insumo/pérdida y de los movimientos.
- [x] Test (Rust, `recalculate_product_cost`): con lotes de 10 a $1 y 5 a $3 → costo $1,67; tras consumir 10 del primero, queda 5 a $3 → costo $3; con stock 0 → conserva el último costo; con una compra posterior al agotamiento → pasa al costo de la compra. Sin ventas, coincide con el promedio de `spec-02` ($1,33 con 10 a $1 y 5 a $2).
- [x] Test (Rust, `recalculate_product_quantity`): tras consumir lotes, `products.quantityMilli` = suma de `remainingMilli`.
- [x] Test (Rust, `restore_consumption`): devuelve al mismo lote si existe; con el lote eliminado crea uno de restitución con el costo del consumo original, `arrivalOrder = MIN - 1`; reutiliza un lote de restitución existente con el mismo costo y `remainingMilli > 0`. Llama a `validate_stock_limit` antes de restaurar.
- [x] Test (Rust, **H-A3 / `validate_stock_limit`**): con un producto cuyo stock es 999.999.998, `validate_stock_limit(conn, product_id, 1)` → `Ok`; con 999.999.998 y `+2` → `Err` con `stockLimitExceeded` y el nombre del producto en el mensaje; con 999.999.999 y `+1` → `Err`.
- [x] Test (Rust, **H-A1 / `reverse_lots_of_invoice`**): con dos lotes de la misma factura, uno con 5 u restantes y otro con 8 u consumidas (restantes 0), la función quita las 5 u del primer lote, deja `remainingMilli = 0` en ambos, recalcula el costo del producto y registra una Reversión por línea con la cantidad y el precio unitario correctos. Si un producto ya no existe, se salta esa línea sin abortar.
- [x] Test (Rust, **invariante de caché**): tras cada operación sobre lotes (crear producto, crear/editar/cancelar/eliminar factura de entrada, crear/editar/cancelar/eliminar factura de salida, pérdida, y **eliminar una compra Cancelada con lotes restaurados**), `products.quantityMilli = SUM(lotes.remainingMilli)` y `products.costCents = promedio_ponderado(lotes con existencia)`. Este test se refuerza en §3 y §4 con cada comando.
- [x] Implementación: `inventory.rs` con `consume_fifo`, `line_total_cost_cents`, `line_unit_cost_cents`, `recalculate_product_cost`, `recalculate_product_quantity`, `restore_consumption`, `next_arrival_order`, `next_restitution_order`, `validate_stock_limit` y `reverse_lots_of_invoice`.

### 1.10 `invoice_snapshot.rs` — copias

- [x] Test (Rust): `build_contact_snapshot` extrae los campos de un `Contact` correctamente (nombre completo, empresa, CI, teléfono, correo). Con campos `NULL` devuelve `None` en el snapshot.
- [x] Test (Rust): `build_line_snapshot(product, line, is_sale)` extrae los campos correctos. Con `is_sale = true` incluye `categoryId`, `categoryName` y `costAtSaleCents`. Con `is_sale = false`, esos tres campos son `None`.
- [x] Test (Rust): `build_movement_snapshot` arma todos los campos de `productMovements` correctamente. El `productId` original se conserva aunque el producto se elimine después.
- [x] Implementación: `invoice_snapshot.rs` con los tres constructores.

### 1.11 `search.rs` — búsqueda por relevancia

- [x] Test (Rust, `relevance_rank`), sub-casillas:
  - [x] Exacto: campo igual a consulta → `Some(0)`.
  - [x] Prefijo: campo empieza por alguna palabra → `Some(1)`.
  - [x] Subcadena: campo contiene → `Some(2)`.
  - [x] No coincide → `None`.
  - [x] Con acentos: `cafe` coincide con `Café` (gracias a `normalize_text`).
  - [x] Con `ñ`: `ano` no coincide con `año`.
  - [x] Teléfono: `+53 5555 1234` coincide con `+5355551234`.
- [x] Test (Rust, `search_contacts`): ordena por `(rank, sort_spanish_key(nombre), id)`. El límite (20 por defecto) se aplica.
- [x] Test (Rust, `search_products_for_invoice`): solo activos; excluye agotados y descontinuados; ordena por nombre, luego categoría, luego descripción.
- [x] Test (Rust, `search_invoices`): busca por número y por la copia del contacto (nombre, empresa, CI, teléfono).
- [x] Implementación: `search.rs` con `normalize_query`, `relevance_rank`, `search_contacts`, `search_products_for_invoice`, `search_invoices`.

### 1.12 `attachments.rs` — PDF

- [x] Test (Rust, `validate_pdf_header`): archivo con cabecera `%PDF-` → `Ok`; archivo de texto → `Err`; archivo vacío → `Err`.
- [x] Test (Rust, `validate_attachment_size`): archivo de 1 MB → `Ok` con `1048576`; archivo de 6 MB → `Err`.
- [x] Test (Rust, `validate_original_name`): nombre con `/` → `Err`; nombre con `\` → `Err`; nombre vacío → `Err`; nombre de 101 caracteres → `Err`; nombre válido → `Ok`.
- [x] Test (Rust, `store_attachment`): copia el archivo a `app_data_dir/attachments/{uuid}.pdf`; el UUID es v4 y único; el `original_name` se conserva como metadato.
- [x] Test (Rust, `delete_attachment`): borra el archivo; un fallo (archivo inexistente) se registra pero no aborta.
- [x] Test (Rust, `sweep_orphans`): con 2 archivos referenciados y 1 huérfano, borra el huérfano. Con un fallo de listado, no aborta y registra en `errors.log`.
- [x] Implementación: `attachments.rs` con las funciones anteriores, incluidas `store_attachment` (UUID v4 + copia) y `sweep_orphans` (best-effort).

### 1.13 `errors.rs` — log de arranque y purga

- [x] Test (Rust): `log_error` con un módulo/operación/causa escribe en `errorLogs`; con un fallo de escritura usa `append_error_log` de `errors.log`. No hay panic si todo falla.
- [x] Implementación: `AppError::should_log()` ampliado con los códigos nuevos (según §1.4). La función de registro no cambia salvo la consulta al throttle.

### 1.14 Fixtures compartidas de test

- [x] Crear `insert_client_fixture`, `insert_supplier_fixture(conn, first_name, last_name, national_id, phone, ...) -> i64`: insertan filas válidas en `clients` / `suppliers` y devuelven el `id`.
- [x] Crear `insert_invoice_fixture(conn, type, number, invoice_date, status, ...) -> i64`: inserta una factura sin líneas, sin lotes ni consumos. Se usa para tests que solo verifican la cabecera o el listado.
- [x] Crear `insert_lot_fixture(conn, product_id, origin, arrival_order, initial_milli, remaining_milli, unit_cost_cents) -> i64`.
- [x] Crear `insert_consumption_fixture(conn, invoice_line_id, lot_id, quantity_milli, unit_cost_cents) -> i64`.
- [x] Crear `insert_movement_fixture(conn, product_id, product_id_original, product_name, type, quantity_milli, stock_after_milli, ...) -> i64`.
- [x] Todos bajo `#[cfg(test)]`. Se usan desde §2 en adelante.

### 1.15 Verificación de la infraestructura

- [x] `cargo test` pasa con los tests de la §1. `cargo clippy` sin warnings. `cargo fmt --check` pasa.
- [x] `cargo build` sigue funcionando. Los tres crates nuevos compilan sin warnings.

---

## 1 bis. Infraestructura compartida (frontend)

### 1 bis.1 `services/invoke.js`

- [x] Test (Vitest): el test existente ("devuelve intactas respuestas/errores de otros códigos") sigue en verde sin modificarse. No se toca `invoke.js`.
- [x] Verificación: `invoke.js` no cambia en este módulo. Se reutiliza el de `spec-02` tal cual.

### 1 bis.2 `utils/formatters.js`

- [x] Test (Vitest): `formatMoney(1500)` → `"15,00 USD"` (ya existe, se verifica). `formatId(1)` → `"#00001"`. `formatQuantity(1500)` → `"1,5"`. `formatDate("2026-01-15")` → `"15/01/2026"` **sin conversión por zona horaria**. `formatTimestamp("2026-01-15T14:30:00Z")` → hora local.
- [x] Implementación: ampliar `formatters.js` si falta `formatTimestamp`. `formatDate` ya existe.

### 1 bis.3 `views/productosView.js` y `views/categoriasView.js` (de `spec-02`)

- [x] Test (Vitest): `productosView.js` dispara el evento `product:viewHistory` con `{ productId }` al pulsar el botón "Ver Historial" del detalle.
- [x] Test (Vitest): `categoriasView.js` muestra la columna "Ingresos" (quita `hidden` y pinta `ingresosCents`) cuando el backend lo devuelve.
- [x] Test (Vitest): `productosView.js` muestra la columna "Valor Total" (quita `hidden` y pinta `valorTotalCents`).
- [x] Implementación: en `productosView.js`, al pulsar "Ver Historial", `dispatchEvent(new CustomEvent('product:viewHistory', { detail: { productId } }))`. En `categoriasView.js`, quitar el `hidden` de la columna "Ingresos" y pintar el campo. Igual en `productosView.js` para "Valor Total".
- [x] Verificación: los tests existentes de `spec-02` para estas vistas siguen en verde.

### 1 bis.4 `<script type="module">` en `dashboard-unificado.html` (H-4)

- [x] **Implementación:** añadir en `dashboard-unificado.html`, **después** de los `<script type="module">` de `spec-01` y `spec-02` y **antes** del `<script>` clásico de `dashboard.js`, los cuatro módulos nuevos en este orden: `clientesView.js`, `proveedoresView.js`, `facturasView.js`, `historialView.js`.
- [x] Test (Vitest + jsdom): cargar `dashboard-unificado.html` con `parseHTML` y verificar el orden relativo de los `<script>` en el documento.
- [ ] Verificación manual: `tauri:dev` carga sin errores de consola. Pendiente hasta que existan `clientesView.js`, `proveedoresView.js`, `facturasView.js` e `historialView.js`; sus tareas de implementación aún no se han ejecutado.

### 1 bis.5 Evento `page:change` (N-3)

*(Infraestructura compartida del frontend: `dashboard.js` despacha este evento para que las vistas se inicialicen sin depender de funciones globales. Sustituye al test que estaba en §2.3.2.)*

- [x] Test (Vitest + jsdom): `dashboard.js` despacha `new CustomEvent('page:change', { detail: { pageId } })` al cambiar de página; el evento se dispara con el `pageId` correcto (el id de la página destino).
- [x] Implementación: en `showPage()`, tras cambiar la vista, `document.dispatchEvent(new CustomEvent('page:change', { detail: { pageId } }))`.
- [ ] Verificación: `clientesView.js`, `proveedoresView.js` y `facturasView.js` lo escuchan y se inicializan cuando `pageId` coincide con su sección. Pendiente hasta implementar esas vistas en sus tareas posteriores; los archivos aún no existen.

---

## 2. Fase 1 — Contactos

*(Clientes y proveedores. No depende de facturas: los indicadores que dependan de ellas se activan en la Fase 3.)*

### 2.1 `commands/clients.rs` — CRUD y búsqueda

#### 2.1.1 `listClients` y `searchClients`

- [x] Test (Rust, `list_clients_inner`): devuelve todos los clientes como `ContactListItem[]`, ordenados por apellidos y luego nombre con `sort_spanish`. Estado vacío: arreglo vacío.
- [x] Test (Rust, `search_clients_inner`): busca por nombre, apellidos, nombre completo, CI, teléfono (normalizado sin espacios ni `+`) y empresa. Relevancia exacto > prefijo > subcadena. Límite 20. Con la consulta vacía devuelve los primeros 20.
- [x] Test (Rust): filas de la matriz de autorización de `listClients` y `searchClients`.
- [x] Implementación: `listClients`, `searchClients` en `commands/clients.rs`. Registrarlos en `generate_handler!`.

#### 2.1.2 `createClient`

- [x] Test (Rust, `create_client_inner`): creación válida con todos los campos; con solo los obligatorios; con campos opcionales vacíos → `NULL`; el `createdAt` se genera con `now` inyectado.
- [x] Test (Rust): CI duplicado → `duplicateField` con "Ya existe un cliente con ese carnet de identidad"; el teléfono repetido **se acepta**.
- [x] Test (Rust): nombre/apellidos < 2 o > 50 → `validationFailed`; empresa/cargo > 100; correo inválido; dirección > 200; información de factura > 200; caracteres de control rechazados.
- [x] Test (Rust): fila de la matriz de autorización de `createClient`.
- [x] Implementación: `createClient(input)` con struct `deny_unknown_fields`. Devuelve `ContactListItem`.

#### 2.1.3 `updateClient`

- [x] Test (Rust, `update_client_inner`): editar sin cambios → `Ok`; editar solo un campo → `Ok`; editar el CI a uno que ya tiene otro cliente → `duplicateField`; editar el CI al mismo que ya tiene → `Ok` (excluye la propia fila).
- [x] Test (Rust): cliente inexistente → `notFound` con "El cliente ya no existe".
- [x] Test (Rust): las validaciones de texto son las mismas que al crear.
- [x] Test (Rust): editar un cliente con facturas **no modifica** las facturas (verifica que la copia de los datos en las facturas no cambia).
- [x] Test (Rust): fila de la matriz de autorización de `updateClient`.
- [x] Implementación: `updateClient(clientId, input)`.

#### 2.1.4 `deleteClient`

- [x] Test (Rust, `delete_client_inner`): elimina físicamente; las facturas que referencian al cliente quedan con `clientId = NULL` y conservan la copia.
- [x] Test (Rust): cliente con facturas → la operación **no** falla por integridad referencial (el `SET NULL` es por FK).
- [x] Test (Rust): cliente inexistente → `notFound`.
- [x] Test (Rust): fila de la matriz de autorización de `deleteClient`.
- [x] Implementación: `deleteClient(clientId)`.

#### 2.1.5 `getClientDetail`

- [x] Test (Rust, `get_client_detail_inner`): devuelve perfil completo + indicadores (en la Fase 1, todos los indicadores son 0 porque no hay facturas) + `invoiceHistory: []`.
- [x] Test (Rust): `createdAt` (fecha de alta) se devuelve en ISO 8601 UTC.
- [x] Test (Rust): cliente inexistente → `notFound`.
- [x] Test (Rust): fila de la matriz de autorización de `getClientDetail`.
- [x] Implementación: `getClientDetail(clientId)`. **Los indicadores se implementan de verdad en la Fase 3** (este test verifica que el DTO se serializa bien con arreglos vacíos).

### 2.2 `commands/suppliers.rs` — CRUD y búsqueda

*(Análogo a `clients.rs` sin `invoiceInfo`.)*

- [x] Test (Rust, `list_suppliers_inner`): idéntico a `listClients`.
- [x] Test (Rust, `search_suppliers_inner`): idéntico a `searchClients`.
- [x] Test (Rust, `create_supplier_inner`): idéntico a `createClient`, sin `invoiceInfo`. CI duplicado → "Ya existe un proveedor con ese carnet de identidad".
- [x] Test (Rust, `update_supplier_inner`): idéntico a `updateClient`.
- [x] Test (Rust, `delete_supplier_inner`): idéntico a `deleteClient`.
- [x] Test (Rust, `get_supplier_detail_inner`): idéntico a `getClientDetail`, sin `invoiceInfo`.
- [x] Test (Rust): filas de la matriz de autorización de los 6 comandos.
- [x] Implementación: los 6 comandos en `commands/suppliers.rs`. Registrarlos en `generate_handler!`.

### 2.3 Interfaz de Contactos

#### 2.3.1 Servicios

- [x] Test (Vitest + jsdom, `clientsService.js`): cada wrapper `camelCase` llama a su comando `snake_case` con los argumentos correctos. El nombre `snake_case` solo aparece aquí.
- [x] Test (Vitest + jsdom, `suppliersService.js`): idéntico.
- [x] Implementación: `clientsService.js`, `suppliersService.js`.

#### 2.3.2 Vista de clientes

- [x] Test (Vitest + jsdom, `clientesView.js`): pinta solo lo que devuelve `listClients` (sin datos de ejemplo). Estados de carga, vacío ("Aún no hay clientes") y error.
- [x] Test (Vitest + jsdom): el buscador `#cliente-search` con `debounce(300)` llama a `searchClients` sin disparar una llamada por cada tecla.
- [x] Test (Vitest + jsdom): el tab-button `#tab-cliente-list` / `#tab-cliente-create` se registra por listener (sin `onclick` inline).
- [x] Test (Vitest + jsdom): crear un cliente llama a `createClient` y recarga el listado.
- [x] Test (Vitest + jsdom): editar un cliente llama a `updateClient`; el CI duplicado muestra el mensaje del backend.
- [x] Test (Vitest + jsdom): eliminar un cliente usa `confirmDialog`; el mensaje indica cuántas facturas conserva (si las tiene).
- [x] Test (Vitest + jsdom): el detalle muestra perfil, indicadores (0 en Fase 1) y el historial (vacío).
- [x] Test (Vitest + jsdom): `#cliente-fecha-entrada` **no existe** en el formulario (eliminado).
- [x] Test (Vitest + jsdom): `#cliente-ver-fecha-entrada` en el detalle se puebla con `getClientDetail().createdAt` formateado con `formatters.formatDate`.
- [x] Test (Vitest + jsdom, **salida segura**): un nombre `<img src=x onerror=…>` se muestra como texto y no crea ningún elemento.
- [x] Verificación: `clientesView.js` escucha el evento `page:change` (ver §1bis.5) y se inicializa cuando `pageId === 'clientes'`.
- [x] Implementación: `clientesView.js`. Registra listeners (no `onclick`). Elimina `#cliente-fecha-entrada` y su `<label>` del HTML. Usa `debounce` de `requestGuard.js`.

#### 2.3.3 Vista de proveedores

- [x] Test (Vitest + jsdom, `proveedoresView.js`): idéntico a `clientesView.js`, incluida la pestaña "Historial de Compras" que en Fase 1 muestra estado vacío (aún no hay facturas de entrada).
- [x] Test (Vitest + jsdom): el botón "Historial" de una fila abre la pestaña con el proveedor seleccionado.
- [x] Test (Vitest + jsdom, **salida segura, N-2**): un nombre de empresa `<img src=x onerror=…>` se muestra como texto y no crea ningún elemento.
- [x] Implementación: `proveedoresView.js`. Registra listeners.

#### 2.3.4 Eliminar el mock de `dashboard.js`

- [x] Eliminar de `dashboard.js`: `clientesData`, `nextClienteSeq`, `editingClienteId`, `clienteVerId`, `showClienteTab`, `cancelarClienteForm`, `editarCliente`, `guardarCliente`, `renderClientes`, `eliminarCliente`, `verCliente`, `editarClienteDesdeVer`, `eliminarClienteDesdeVer`, `irCrearCliente`, `calcularResumenFacturasCliente`.
- [x] Eliminar de `dashboard.js`: `proveedoresData`, `nextProveedorSeq`, `editingProveedorId`, `proveedorVerId`, `proveedoresDisponibles`, `actualizarProveedoresDisponibles`, `showProveedorTab`, `cancelarProveedorForm`, `editarProveedor`, `guardarProveedor`, `renderProveedores`, `eliminarProveedor`, `verProveedor`, `editarProveedorDesdeVer`, `eliminarProveedorDesdeVer`, `irCrearProveedor`, `calcularResumenFacturasProveedor`, `poblarSelectHistorialProveedor`, `obtenerMovimientosProveedor`, `renderHistorialProveedor`, `verHistorialProveedor`.
- [x] Eliminar de `showPage()` las llamadas a `renderClientes()` y `renderProveedores()`. Añadir el despacho del evento `page:change` con `{ detail: { pageId } }`.
- [x] Eliminar `poblarDatalists()` y `actualizarProveedoresDisponibles()` de `dashboard.js`.
- [x] Eliminar los manejadores inline de la sección Clientes y Proveedores del HTML.
- [x] Verificación: `grep` de los identificadores anteriores en `dashboard.js` no devuelve nada; el dashboard carga sin errores en consola; las secciones Clientes y Proveedores muestran estado vacío con la BD vacía.
- [x] Verificación: `npm test` pasa; el test de limpieza de `environment.test.js` se actualiza para cubrir los identificadores nuevos que se eliminan.

---

## 3. Fase 2 — Lotes, compras e historial

*(Lotes, facturas de entrada, cambios a `spec-02`, historial. Depende de la Fase 1: los proveedores son prerequisito.)*

### 3.1 `commands/invoices.rs` — listado y detalle genéricos

#### 3.1.1 `listInvoices`

- [x] Test (Rust, `list_invoices_inner`): sin filtros devuelve todas las facturas de todos los tipos; con `type` filtra por tipo; con `status` agrupa (pendientes, completadas, canceladas); con `search` busca por número y por la copia del contacto; con `fromDate`/`toDate` filtra por rango de día local. Los campos omitidos toman su valor por defecto (`serde(default)`).
- [x] Test (Rust): rango invertido → `validationFailed` con "La fecha inicial no puede ser posterior a la final".
- [x] Test (Rust): las pérdidas se excluyen con cualquier filtro de estado distinto de "Todos". `type = 'P'` las incluye explícitamente.
- [x] Test (Rust): devuelve `InvoiceListItem[]` con `associatedInvoice` solo en GG y GI.
- [x] Test (Rust): purga primero (un producto descontinuado vencido no aparece).
- [x] Test (Rust): fila de la matriz de autorización de `listInvoices`.
- [x] Implementación: `listInvoices(filters)`. Purga best-effort. `InvoiceListItem` se serializa con `camelCase`.

#### 3.1.2 `getInvoice`

- [x] Test (Rust, `get_invoice_inner`): devuelve todos los campos de `InvoiceDetail` con los campos opcionales según el tipo. Para entrada incluye `attachmentFileName`, `attachmentOriginalName`, `attachmentSizeBytes`, `attachmentUploadedAt`. Para venta/servicio incluye `categoryName` y `costAtSaleCents` en las líneas. Para garantía/insumo incluye `associatedInvoice`. Para pérdida, `status = null`, `contactFullName = null`, sin método de pago.
- [x] Test (Rust): factura inexistente o purgada → `notFound`.
- [x] Test (Rust): purga primero.
- [x] Test (Rust): fila de la matriz de autorización de `getInvoice`.
- [x] Implementación: `getInvoice(invoiceId)`.

#### 3.1.3 `changeInvoiceStatus`

- [x] Test (Rust, `change_invoice_status_inner`): todas las transiciones de `spec-03` §7.5. Mismo estado → `invalidState`. Cancelada → cualquier → `invalidState`. `paymentMethod` solo con `completed`/`cancelled` y tipos E/V/S.
- [x] Test (Rust): cambiar a `completed` en E/V/S registra `paidAt` con `now`; con `paymentMethod` opcional. Volver a `pending` borra `paidAt` y `paymentMethod`. No toca el inventario.
- [x] Test (Rust): cambiar a `cancelled` en una salida no cancelada devuelve los consumos a sus lotes (LT-04) y registra una Reversión por línea.
- [x] Test (Rust): cambiar a `cancelled` en una entrada no cancelada quita lo que queda de sus lotes y registra una Reversión por línea.
- [x] Test (Rust): cambiar a `cancelled` desde `completed` en una salida conserva `paidAt` y `paymentMethod` (D-07).
- [x] Test (Rust, **atomicidad**): con un fallo forzado en la inserción de un movimiento, no queda ningún cambio parcial.
- [x] Test (Rust, **N-1 / `updatedAt`**): `changeInvoiceStatus` actualiza `invoices.updatedAt` con `format_utc_timestamp(now)` en cada transición válida; `createdAt` no cambia.
- [x] Test (Rust): fila de la matriz de autorización de `changeInvoiceStatus`.
- [x] Implementación: `changeInvoiceStatus(invoiceId, status, paymentMethod?)`. Aplica la purga best-effort antes. Registra los movimientos correspondientes. **Actualiza `updatedAt` con `format_utc_timestamp(now)` en cada transición.**

#### 3.1.4 `deleteInvoice`

- [x] Test (Rust, `delete_invoice_inner`): eliminar una entrada no cancelada quita lo que queda de sus lotes y registra una Reversión. Eliminar una salida no cancelada devuelve los consumos a sus lotes y registra una Reversión. Eliminar una Cancelada sin restos no toca el inventario. Eliminar una pérdida devuelve los consumos.
- [x] Test (Rust): al eliminar una entrada con adjunto, se borra el archivo de disco. Un fallo de borrado se registra pero la operación continúa.
- [x] Test (Rust, **H-A1 / eliminar una compra Cancelada con lotes restaurados**): escenario completo: producto con 10 u, compra E-1 de 10 u, venta V-1 que consume 4 u, cancelar E-1 (lote a 0), cancelar V-1 (restaura 4 u), eliminar E-1. Después: el lote con 4 u se quita y se registra una Reversión por las 4 u retiradas (`quantityMilli = -4000`) al precio unitario del consumo original; `products.quantityMilli = 0` y la suma de `remainingMilli` de los lotes del producto también. El invariante de caché se mantiene.
- [x] Test (Rust, **H-A1 / eliminar una compra Cancelada sin restos**): con todos los lotes en `remainingMilli = 0`, se elimina sin tocar el inventario (comportamiento anterior).
- [x] Test (Rust, **H-A1 / producto eliminado con lotes restaurados**): si un producto de la compra Cancelada ya no existe, se salta su ajuste; los demás lotes se procesan con normalidad; el aviso lo menciona.
- [x] Test (Rust): fila de la matriz de autorización de `deleteInvoice`.
- [x] Implementación: `deleteInvoice(invoiceId)`. Usa `reverse_lots_of_invoice` para entradas no canceladas y Canceladas con restos (H-A1); la eliminación de pérdidas revierte sus consumos registrados.

### 3.2 `commands/invoices.rs` — creación y edición de factura de entrada

#### 3.2.1 `createEntryInvoice`

- [x] Test (Rust, `create_entry_invoice_inner`), sub-casillas:
  - [x] Creación válida con proveedor existente. Número `E-00001`. Estado inicial `pending` o `completed`.
  - [x] Con estado `completed` se registra `paidAt` con `now`.
  - [x] Se crea un lote por línea con `arrivalOrder` incremental. Se recalcula el costo del producto (LT-03). Se registra un movimiento `entry` por línea (HP-01).
  - [x] **Producto nuevo sobre la marcha:** con `newProduct` en una línea, el producto se crea en la misma transacción. Si el nombre coincide con un activo existente → `duplicateField` con "Ya existe un producto con ese nombre" y la factura no se guarda. La cantidad y el costo de la línea son su existencia inicial, **sin crear un lote inicial aparte**.
  - [x] Límite: 1 a 100 líneas. Más de 100 → `validationFailed`. Producto repetido → `validationFailed` con "El producto ya está en la factura".
  - [x] Total mayor que $999.999.999,99 → `validationFailed`.
  - [x] `supplierId` inexistente → `notFound` con "El proveedor ya no existe".
  - [x] `productId` inexistente → `notFound`. Producto descontinuado → `validationFailed` con "El producto «X» está descontinuado. Actívalo para usarlo en facturas".
  - [x] Fecha futura → `validationFailed`. Fecha < 2000-01-01 → `validationFailed`. `2026-02-30` → `validationFailed`.
  - [x] Con adjunto (`attachmentFileName`, etc.) los cuatro campos se guardan juntos.
  - [x] Copia del proveedor guardada en la factura; copia del producto en cada línea.
  - [x] **H-A3 / tope de stock:** con un producto cuyo stock es 999.999.000 y una línea de 2.000 unidades, `createEntryInvoice` devuelve `stockLimitExceeded`; factura, líneas, lote y movimientos no se crean. Con una línea de 999 unidades, pasa.
- [x] Test (Rust, **rollback completo**): con un fallo forzado en la inserción de un lote, no queda ni la factura, ni las líneas, ni el producto nuevo, ni los movimientos.
- [x] Test (Rust, **H-A3 / rollback por tope**): si una línea supera el tope de stock, ni siquiera se inserta la factura; la base queda como estaba.
- [x] Test (Rust): fila de la matriz de autorización de `createEntryInvoice`.
- [x] Implementación: `createEntryInvoice(input)` con `validate_stock_limit` por línea antes de crear el lote.

#### 3.2.2 `updateEntryInvoice`

- [x] Test (Rust, `update_entry_invoice_inner`), sub-casillas:
  - [x] Editar una entrada `pending` con un cambio de fecha funciona.
  - [x] Editar una entrada `completed` → `invalidState` con "Solo se pueden editar las facturas que están Por Pagar".
  - [x] Editar una entrada `cancelled` → `invalidState` con "Una factura cancelada no se puede reactivar ni editar".
  - [x] Agregar una línea: se crea el lote y el movimiento `entry`. Se recalcula el costo del producto.
  - [x] Quitar una línea **sin consumo**: se elimina el lote, se recalcula el costo y se registra un movimiento `reversal`.
  - [x] Quitar una línea **con consumo** → `invalidState` con el mensaje "No se puede quitar «X»: ya se usaron unidades de esta compra".
  - [x] Reducir cantidad **por debajo de lo consumido** → `invalidState` con "No se puede reducir «X» por debajo de las N unidades ya usadas".
  - [x] Aumentar cantidad **con consumo** → `Ok`; el lote mantiene su posición de llegada.
  - [x] Cambiar costo de una línea **con consumo** → `invalidState` con "No se puede cambiar el costo de «X»: ya se usaron unidades de esta compra en la factura V-00012".
  - [x] Cambiar el producto de una línea **con consumo** → `invalidState`.
  - [x] Producto descontinuado en una línea existente: solo se puede reducir la cantidad; aumentar → `invalidState`.
  - [x] Producto eliminado en una línea existente: se puede quitar (siempre) o reducir a 0.
  - [x] `updateEntryInvoice` con `quantity` en el `input` → falla al deserializarse (campo prohibido).
  - [x] **H-A4 / cambio de costo por re-FIFO sin cambio de cantidad:** editar una línea sin cambiar su cantidad pero con los lotes en un estado distinto (por ejemplo, se eliminó el lote que se consumía originalmente) actualiza el `unitPriceCents` de la línea **sin registrar un Ajuste**.
  - [x] **H-A3 / tope de stock al editar compra:** con un producto cuyo stock es 999.999.500, agregar una línea o aumentar la cantidad de una existente por encima del tope falla con `stockLimitExceeded`; la factura no cambia.
- [x] Test (Rust, **atomicidad**): con un fallo forzado en la inserción del movimiento de Reversión, no queda ningún cambio.
- [x] Test (Rust, **N-1 / `updatedAt`**): `updateEntryInvoice` actualiza `invoices.updatedAt` con `format_utc_timestamp(now)`; `createdAt` no cambia.
- [x] Test (Rust): fila de la matriz de autorización de `updateEntryInvoice`.
- [x] Implementación: `updateEntryInvoice(input)`. **Actualiza `updatedAt` con `format_utc_timestamp(now)`.** Llama a `validate_stock_limit` antes de insertar/aumentar lotes.

#### 3.2.3 `saveInvoiceAttachment` y `downloadInvoiceAttachment`

- [x] Test (Rust, `save_invoice_attachment_inner`): con archivo PDF válido ≤ 5 MB copia a `app_data_dir/attachments/{uuid}.pdf`; actualiza los cuatro campos de `invoices`. Archivo no PDF → `validationFailed` con "El archivo debe ser un PDF de hasta 5 MB". Archivo > 5 MB → `validationFailed`. Factura `completed`/`cancelled` → `invalidState` con "Solo se pueden editar las facturas que están Por Pagar".
- [x] Test (Rust, `download_invoice_attachment_inner`): copia el archivo al destino. Archivo no encontrado → `notFound` con "El archivo adjunto no se encuentra en el equipo...".
- [x] Test (Rust, **reemplazo**): se escribe el nuevo archivo, se hace `UPDATE` en transacción, `commit`, luego se borra el viejo. Si el `UPDATE` falla, el nuevo queda huérfano y el barrido lo limpia.
- [x] Test (Rust, **barrido al arrancar**): con 2 archivos referenciados y 1 huérfano, borra el huérfano. Best-effort: un fallo se registra en `errors.log` y no aborta el arranque.
- [x] Test (Rust, **H-10 / archivo corrupto al descargar**): `downloadInvoiceAttachment` con un archivo cuyo nombre está en la BD pero cuyo contenido no empieza por `%PDF-` **copia igual** (la validación es al subir, no al descargar) y registra un aviso en `errors.log` si el tamaño en disco difiere del registrado.
- [x] Test (Rust, **N-1 / O-B / `updatedAt`**): `saveInvoiceAttachment` actualiza `invoices.updatedAt` con `format_utc_timestamp(now)` (confirmado en `plan-03 §3.8`); `createdAt` no cambia.
- [x] Test (Rust): filas de la matriz de autorización de `saveInvoiceAttachment` y `downloadInvoiceAttachment`.
- [x] Implementación: `saveInvoiceAttachment`, `downloadInvoiceAttachment` usando `attachments.rs`. Al arrancar, ejecutar `sweep_orphans` junto a `init_schema`, `clear_non_persistent_sessions` y la purga de `errorLogs`. **`saveInvoiceAttachment` actualiza `updatedAt`.**

### 3.3 `commands/history.rs` — historial (backend)

#### 3.3.1 `getProductHistory`

- [x] Test (Rust, `get_product_history_inner`), sub-casillas:
  - [x] Sin filtros devuelve todos los movimientos del producto ordenados por `createdAt DESC, id DESC`.
  - [x] Con `fromDate`/`toDate` (día local, extremos incluidos).
  - [x] Con `types` (uno o varios, semántica OR). Sin `types` seleccionados → todos.
  - [x] El filtro con `fromDate > toDate` → `validationFailed`.
  - [x] Incluye movimientos de productos eliminados si el `productIdOriginal` coincide.
  - [x] `expiredCleanupWarning` se incluye (purga best-effort).
  - [x] `productId` inexistente que nunca tuvo movimientos → `notFound`.
- [x] Test (Rust, **H-9 / purga fallida**): con un trigger `RAISE(ABORT)` que haga fallar el `DELETE` de `purge_expired_products`, `getProductHistory` **no aborta**: devuelve los movimientos (incluidos los de productos vencidos, porque el historial no depende de la purga) con `expiredCleanupWarning` no nulo y registra el error.
- [x] Test (Rust): fila de la matriz de autorización de `getProductHistory`.
- [x] Implementación: `getProductHistory(filters)`. Purga best-effort.

#### 3.3.2 `listHistoryProducts`

- [x] Test (Rust, `list_history_products_inner`): devuelve un arreglo plano con todos los productos existentes (`isDeleted: false`) más los productos eliminados **que tienen movimientos** (`isDeleted: true`). Los eliminados sin movimientos no aparecen. Orden alfabético español dentro de cada grupo.
- [x] Test (Rust, **C-7 / no purga**): `listHistoryProducts` **no ejecuta la purga**. Con un descontinuado vencido insertado por fixture, se devuelve en el grupo de activos hasta que otro comando de la lista de `spec-03 §7.6` lo purgue; tras la purga (por ejemplo, llamando a `listInvoices`), el mismo producto aparece en el grupo de eliminados y sus movimientos se conservan.
- [x] Test (Rust): fila de la matriz de autorización de `listHistoryProducts`.
- [x] Implementación: `listHistoryProducts()`. **No purga** (decisión de diseño de `plan-03 §3.9`).

### 3.4 `commands/products.rs` — ampliación para facturas

#### 3.4.1 `searchProductsForInvoice`

- [x] Test (Rust, `search_products_for_invoice_inner`): busca por nombre, categoría y descripción con relevancia. Solo activos. Excluye agotados y descontinuados. Límite 20.
- [x] Test (Rust): purga best-effort antes de buscar.
- [x] Test (Rust): fila de la matriz de autorización de `searchProductsForInvoice`.
- [x] Implementación: `searchProductsForInvoice(query, limit?)`.

#### 3.4.2 `getProductAssociatedInvoices`

- [x] Test (Rust, `get_product_associated_invoices_inner`): busca por `productIdOriginal`. Devuelve hasta 50 facturas de cualquier tipo, de la más reciente a la más antigua. Incluye `quantity` de la línea.
- [x] Test (Rust): funciona con un producto eliminado que tiene movimientos.
- [x] Test (Rust): fila de la matriz de autorización de `getProductAssociatedInvoices`.
- [x] Implementación: `getProductAssociatedInvoices(productId, limit?)`.

### 3.5 Cambios a `spec-02` (backend)

- [x] **SP-01**: `commands/products.rs::create_product_inner` amplía para crear el lote inicial y registrar el movimiento `initial`. El producto creado desde `createEntryInvoice` no tiene lote inicial (su existencia es el lote de la línea).
- [x] **SP-02**: eliminar `pricing.rs::weighted_average_cost_cents` y su test. `selling_price_cents` se conserva.
- [x] **SP-02**: `createProduct` sigue pidiendo cantidad inicial y costo inicial. **No** cambia la firma.
- [x] **SP-03**: `commands/products.rs::list_products_inner` y `get_product_inner` calculan `valorTotalCents` desde los lotes con existencia.
- [x] **SP-03**: `commands/categories.rs::list_categories_inner` y `get_category_inner` calculan `ingresosCents` desde `invoiceLines.categoryId` + `subtotalCents` de facturas V y S no canceladas. **En Fase 2 devuelven 0** porque no hay ventas; se activa en Fase 3.
- [x] Añadir `valorTotalCents` a `ProductListItem` y `ProductDetail`; `ingresosCents` a `CategoryListItem` y `CategoryDetail`.
- [x] Anotar en los tests de `spec-02` §6.1, §6.6, §8 y §14 el comentario "válido solo dentro de `spec-02`".
- [x] Test (Rust): `create_product_inner` crea el lote inicial y el movimiento `initial`. `products.quantityMilli` = cantidad del lote inicial.
- [x] Test (Rust): eliminar `weighted_average_cost_cents` no rompe otros tests.
- [x] Test (Rust): `list_products_inner` devuelve `valorTotalCents` correcto tras varias compras.
- [x] Test (Rust): `list_categories_inner` devuelve `ingresosCents = 0` sin ventas.
- [x] Implementación de los cambios.
- [x] **H-2 / modificación de tests de `spec-02`:** en los tests de `src-tauri/src/commands/products.rs` (o donde vivan), añadir el comentario `// Nota: esta aserción es válida solo dentro de spec-02. spec-03 cambia el comportamiento (los comandos de facturas sí modifican quantityMilli).` en:
  - Test de `spec-02` §6.1 sobre "ningún comando de esta spec la modifica después".
  - Test de `spec-02` §6.6 sobre "los comandos… no alteran la cantidad".
  - Test de `spec-02` §8 sobre "P-10 sin consumidor".
  - Test de `spec-02` §14 sobre "con datos reales ningún activo está agotado".
- [x] **O-1 / regresión de `selling_price_cents`:** tras eliminar `weighted_average_cost_cents`, añadir un test que confirme que `selling_price_cents` sigue produciendo los mismos valores para una tabla de casos (12,50 → 1500 con 20 %; 0 % → cost; 1000 % → cost × 11). Protege contra una regresión accidental del cálculo de precios históricos.

### 3.6 Interfaz de Facturas de Entrada

#### 3.6.1 Servicio

- [x] Test (Vitest + jsdom, `invoicesService.js`): cada wrapper `camelCase` llama a su comando `snake_case` con los argumentos correctos. `saveInvoiceAttachment(invoiceId, sourcePath)` y `downloadInvoiceAttachment(invoiceId, destinationPath)` reciben las rutas.
- [x] Implementación: `invoicesService.js` con los wrappers de los 17 comandos de facturas.

#### 3.6.2 Vista de facturas de entrada

- [x] Test (Vitest + jsdom, `facturasView.js` — submódulo `entry`): pinta solo lo que devuelve `listInvoices({ type: 'E' })`. Estados de carga, vacío, error.
- [x] Test (Vitest + jsdom): el buscador `#factura-entrada-search` con `debounce(300)`.
- [x] Test (Vitest + jsdom): el `<select>` `#factura-estado` tiene las opciones correctas (`pending`/`completed`/`cancelled`). El `<select>` de método de pago solo se muestra con `completed`.
- [x] Test (Vitest + jsdom): `#factura-proveedor` es un combobox; el botón `+` abre el formulario de proveedor.
- [x] Test (Vitest + jsdom): `#factura-buscar-producto` es un combobox con botón `+`.
- [x] Test (Vitest + jsdom): `#factura-descuento` y `#factura-impuesto` **no existen**.
- [x] Test (Vitest + jsdom): el dropzone `#factura-dropzone` sigue existiendo. Al guardar, llama a `saveInvoiceAttachment` si hay archivo.
- [x] Test (Vitest + jsdom): el botón "Editar" del detalle se muestra solo si `status === 'pending'`. Si no, oculto.
- [x] Test (Vitest + jsdom): el botón "Guardar en el equipo" del detalle llama a `downloadInvoiceAttachment`.
- [x] Test (Vitest + jsdom): el botón "Exportar Factura" llama a `exportarFacturaPDF` adaptado al DTO de `getInvoice`.
- [x] Test (Vitest + jsdom, **salida segura**): un nombre de producto con HTML se muestra como texto.
- [x] Implementación: submódulo `entry` de `facturasView.js`. Registra listeners. Elimina `#factura-descuento` y `#factura-impuesto` del HTML. Sustituye los `<datalist>` por combobox.

#### 3.6.3 Eliminar el mock de facturas de entrada

- [x] Eliminar de `dashboard.js`: `facturasEntradaData`, `nextFacturaEntradaSeq`, `editingFacturaId`, `facturaLineItems`, `nextLineItemId`, `facturaArchivoActual`, `renderFacturasEntrada`, `eliminarFactura`, `showFacturaEntradaTab`, `verFactura`, `eliminarFacturaDesdeVer`, `prepararNuevaFactura`, `cancelarFacturaEntradaForm`, `agregarLineaFactura`, `agregarLineaDesdeBusqueda`, `crearProductoRapido`, `quitarLineaFactura`, `actualizarLineaFactura`, `renderLineasFactura`, `recalcularTotalesFactura`, `manejarArchivoFactura`, `manejarDropArchivo`, `quitarArchivoFactura`, `guardarFactura`.
- [x] Adaptar `exportarFacturaPDF` para leer del DTO de `getInvoice`.
- [x] Eliminar los manejadores inline de la sección Facturas de Entrada del HTML.
- [x] Verificación: `grep` de los identificadores; `dashboard.js` carga sin errores; el listado muestra estado vacío con la BD vacía.
- [x] `npm test` pasa.

### 3.7 Interfaz de Historial de Producto

#### 3.7.1 Servicio

- [x] Test (Vitest + jsdom, `historyService.js`): cada wrapper `camelCase` llama a su comando `snake_case`.
- [x] Implementación: `historyService.js`.

#### 3.7.2 Vista de historial

- [x] Test (Vitest + jsdom, `historialView.js`): `#producto-historial` y `#tab-producto-historial` dejan de estar ocultos. `#producto-ver-historial` deja de estar oculto.
- [x] Test (Vitest + jsdom): el selector `#historial-producto-select` se puebla con `listHistoryProducts()`; tiene dos `<optgroup>` (activos, eliminados). Los eliminados se identifican por `Nombre (eliminado, #ID)`.
- [x] Test (Vitest + jsdom): el filtro `#historial-producto-tipo` es un `<select multiple>` con tres `<optgroup>` (Entradas, Salidas, Ajustes) y los valores de código. Sin selección → todos.
- [x] Test (Vitest + jsdom): el listado `#historial-producto-body` pinta los movimientos del backend. `textContent` para los datos.
- [x] Test (Vitest + jsdom): el botón "Ver Historial" del detalle de producto dispara el evento `product:viewHistory` (ya implementado en `productosView.js`); `historialView.js` lo escucha y abre la pestaña con ese producto preseleccionado.
- [x] Test (Vitest + jsdom): al entrar a la pestaña, se recarga el historial (consistente con "tras cada mutación se recarga").
- [x] Test (Vitest + jsdom): las fechas en la columna "Fecha" van en hora local con `formatTimestamp`.
- [x] Test (Vitest + jsdom, **salida segura, N-2**): un nombre de producto con HTML (`<img src=x onerror=…>`) se muestra como texto y no crea ningún elemento en las celdas del historial.
- [x] Implementación: `historialView.js`. Registra listeners.

### 3.8 Interfaz de Productos y Categorías (cambios de `spec-02`)

- [x] Test (Vitest + jsdom): `productosView.js` muestra `valorTotalCents` en la columna "Valor Total" y en el detalle.
- [x] Test (Vitest + jsdom): `categoriasView.js` muestra `ingresosCents` en la columna "Ingresos" y en el detalle. En Fase 2 se muestra 0 porque no hay ventas.
- [x] Test (Vitest + jsdom, **salida segura, N-2**): en `productosView.js` y `categoriasView.js`, un nombre con HTML se muestra como texto tanto en el listado como en el detalle.
- [x] Implementación: aplicar los cambios de `plan-03.md` §5.15/§5.16.

### 3.9 Verificación de la Fase 2

- [x] `cargo test` pasa con todos los tests de la Fase 2. `cargo clippy --all-targets -- -D warnings` sin warnings.
  - Verificado en máquina del dueño (2026-10-11): 322/322 en verde; `clippy --all-targets -- -D warnings` limpio (tras corregir `E0382` mover-y-prestar y `type_complexity` en los helpers nuevos).
- [x] `npm test` pasa.
  - Verificado 2026-10-10: 24 archivos, 131/131 tests en verde (una corrida intermedia mostró 1 flaky ajeno —`loginView`/`dashboardSummary`, interferencia entre archivos con `isolate: false`— que pasa aislado y en la corrida final).
- [x] **Test de contrato `invoke ↔ generate_handler!`**: el test de `environment.test.js` pasa a esperar los comandos de esta fase. El test **calcula el recuento a partir de los archivos** (no literal).
  - Sin lista fija de módulos ni recuento literal: deriva `handlerCommands` de `generate_handler!`, `serviceCommands` de `*Service.js` y `viewCommands` de `*View.js`; solo dos listas documentadas y auto-mantenidas (`pendingBackend`: 5 wrappers adelantados de Fase 3/4; `pendingWrapper`: `get_product_associated_invoices`). 6/6 en verde.
- [x] **Invariante de caché**: el test de §1.9 se refuerza con los comandos de esta fase (crear/editar/cancelar/eliminar entrada, crear/editar producto).
  - Cubierto por `lot_order_and_product_cache_recalculation_follow_remaining_stock`, `deleting_cancelled_entry_with_restored_stock_preserves_inventory_cache` y `create_product_creates_initial_lot_and_history_entry`, todos en verde en la corrida 322/322.
- [ ] Verificación manual: crear un proveedor, una factura de entrada con adjunto, verificar el lote inicial del producto creado con `createProduct`, y consultar el historial de un producto.
  - **Nota de entorno:** pendiente; este entorno no permite operar la ventana nativa de Tauri (precedente de `tasks-01.md`/`tasks-02.md`).
- [x] `cargo fmt --check` pasa.
  - Verificado en máquina del dueño (2026-10-11): `cargo fmt` aplicado (incluida deriva preexistente de versión) y `--check` limpio.

---

## 4. Fase 3 — Salidas

*(Venta, servicio, garantía, insumos, pérdidas, lista general, indicadores, "Valor Total" e "Ingresos". Depende de la Fase 2: las salidas consumen los lotes.)*

### 4.1 `commands/invoices.rs` — creación de facturas de salida

#### 4.1.1 `createSaleInvoice`

- [x] Test (Rust, `create_sale_invoice_inner`), sub-casillas:
  - [x] Creación válida con cliente existente. Número `V-00001`.
  - [x] Las líneas descuentan por FIFO (LT-02). Se registra un movimiento `saleOut` por línea con el precio de venta vigente.
  - [x] `unitPriceCents` = `selling_price_cents(costCents, profitPercentHundredths)` vigente al crear.
  - [x] La línea guarda `categoryId`, `categoryName` y `costAtSaleCents` del producto en ese momento.
  - [x] `categoryId`/`categoryName` son **snapshots sin FK**: sobreviven a la eliminación de la categoría.
  - [x] Producto sin stock → `stockInsuficiente` con "No hay stock suficiente de «X» (disponible: N)".
  - [x] Producto descontinuado → `validationFailed`. Producto agotado → `validationFailed`.
  - [x] Límites de 100 líneas; producto repetido → `validationFailed`.
  - [x] Total mayor que $999.999.999,99 → `validationFailed`.
  - [x] `clientId` inexistente → `notFound`. Fecha futura → `validationFailed`.
- [x] Test (Rust, **atomicidad**): con un fallo forzado en la inserción de un consumo, no queda ningún cambio.
- [x] Test (Rust): fila de la matriz de autorización de `createSaleInvoice`.
- [x] Implementación: `createSaleInvoice(input)`.
- **Nota:** verificado en máquina del dueño (2026-10-11): `cargo test` 322/322, `clippy` limpio, `npm test` 131/131.

#### 4.1.2 `createServiceInvoice`

- [x] Test (Rust, `create_service_invoice_inner`): idéntico a venta pero tipo `S`. El total es la suma de cantidad × precio de venta de los insumos.
- [x] Test (Rust): fila de la matriz de autorización.
- [x] Implementación: `createServiceInvoice(input)`.
- **Nota:** venta y servicio comparten el helper `create_client_outgoing_invoice_inner` (FS-01). Verificado en máquina del dueño (2026-10-11): `cargo test` 322/322, `clippy` limpio, `npm test` 131/131.

#### 4.1.3 `createGuaranteeInvoice`

- [x] Test (Rust, `create_guarantee_invoice_inner`), sub-casillas:
  - [x] Creación válida con `associatedInvoiceId` de una venta o servicio del mismo cliente, no cancelada. Tipo `GG`.
  - [x] Líneas con costo real de los lotes consumidos (`line_unit_cost_cents`). Total = costo total de la garantía.
  - [x] Sin método de pago. Estados `pending`/`completed`/`cancelled`.
  - [x] `associatedInvoiceId` de otro cliente → `validationFailed`.
  - [x] `associatedInvoiceId` de una factura cancelada → `validationFailed`.
  - [x] `associatedInvoiceId` inexistente → `notFound`.
  - [x] `associatedInvoiceId` de una entrada o pérdida → `validationFailed`.
  - [x] Se guarda copia del número y tipo (`associatedNumber`, `associatedType`).
- [x] Test (Rust, **atomicidad**): con un fallo forzado, no queda ningún cambio.
- [x] Test (Rust): fila de la matriz de autorización.
- [x] Implementación: `createGuaranteeInvoice(input)`.
- **Nota:** verificado en máquina del dueño (2026-10-11): `cargo test` 322/322, `clippy` limpio, `npm test` 131/131.

#### 4.1.4 `createInstallationInvoice`

- [x] Test (Rust, `create_installation_invoice_inner`): tipo `GI`. `associatedInvoiceId` debe ser un servicio del mismo cliente, no cancelado. Todo lo demás como garantía.
- [x] Test (Rust): fila de la matriz de autorización.
- [x] Implementación: `createInstallationInvoice(input)`.
- **Nota:** garantía e insumos comparten el helper `create_associated_outgoing_invoice_inner`; el wrapper `createInstallationInvoice` en `invoicesService.js` completa los 17 de la 3.6.1. Verificado en máquina del dueño (2026-10-11): `cargo test` 322/322, `clippy` limpio, `npm test` 131/131.

#### 4.1.5 `createLossInvoice`

- [x] Test (Rust, `create_loss_invoice_inner`), sub-casillas:
  - [x] Creación válida con un producto activo con stock. Número `P-00001`.
  - [x] Consumo FIFO. Costo real calculado. Movimiento `loss`.
  - [x] `status = NULL`. `contactFullName = NULL`. `paymentMethod = NULL`. `paidAt = NULL`.
  - [x] Producto descontinuado **con stock** → permitido (excepción de S-22).
  - [x] Producto descontinuado sin stock → `validationFailed`.
  - [x] Cantidad mayor que el stock → `stockInsuficiente`.
  - [x] Un solo producto por pérdida (no es un arreglo de líneas).
- [x] Test (Rust): fila de la matriz de autorización.
- [x] Implementación: `createLossInvoice(input)`.
- **Nota:** sin estados/cliente/pago (todo `NULL`); descontinuado con stock permitido, sin stock rechazado por agotado. `npm test` 131/131 en verde (2 flakies rotativos preexistentes en corridas intermedias, verde final). Ejecución Rust pendiente en tu máquina: `cargo test --manifest-path src-tauri/Cargo.toml loss`.

### 4.2 `commands/invoices.rs` — edición de facturas de salida

- [x] Test (Rust, `update_sale_invoice_inner`), sub-casillas:
  - [x] Editar una venta `pending`. Devuelve los consumos anteriores a sus lotes (LT-04), vuelve a descontar, registra movimientos (`saleOut`, `reversal`, `adjustment`).
  - [x] Las líneas existentes **conservan** su `unitPriceCents`; las nuevas toman el vigente.
  - [x] Líneas con producto descontinuado solo se conservan sin cambios; las eliminadas se conservan sin efecto.
  - [x] Editar una venta `completed` → `invalidState`.
  - [x] Sin stock suficiente tras la edición → `stockInsuficiente`, no se guarda.
  - [x] **H-A4 / re-FIFO sin cambio de cantidad no genera Ajuste:** venta V-1 con una línea de producto X, 3 u a $5,00. Editar V-1 sin cambiar la cantidad de esa línea, pero con los lotes en un estado distinto (se eliminó el lote que se consumía originalmente y se consumió otro más caro) → el `unitPriceCents` de la línea se actualiza al nuevo costo real, **pero no se registra un Ajuste**; solo hay movimientos de Salida para líneas nuevas y Reversión para líneas quitadas.
- [x] Test (Rust, `update_service_invoice_inner`, `update_guarantee_invoice_inner`, `update_installation_invoice_inner`): análogos.
- [x] Test (Rust, **atomicidad**): con un fallo forzado en la inserción de un movimiento o un consumo, no queda ningún cambio.
- [x] Test (Rust, **N-1 / `updatedAt`**): `updateSaleInvoice`, `updateServiceInvoice`, `updateGuaranteeInvoice` y `updateInstallationInvoice` actualizan `invoices.updatedAt` con `format_utc_timestamp(now)`; `createdAt` no cambia.
- [x] Test (Rust): filas de la matriz de autorización de los 4 comandos.
- [x] Implementación: los 4 comandos. **Actualizan `updatedAt` con `format_utc_timestamp(now)`.** El Ajuste se emite solo si la cantidad de la línea cambió (H-A4).
- **Nota (2026-10-11):** dos núcleos compartidos (`update_priced_outgoing_invoice_inner` para V/S con precio preservado; `update_costed_outgoing_invoice_inner` para GG/GI con costo recalculado) + 4 wrappers + 4 comandos registrados + 3 wrappers JS añadidos (`updateService/Guarantee/InstallationInvoice`). H-A4: en V/S se preserva `unitPriceCents` y se refresca `costAtSaleCents` en silencio; en GG/GI se actualiza el costo en silencio. `npm test` 131/131 en verde (flakies rotativos preexistentes en intermedias). Ejecución Rust pendiente en tu máquina: `cargo test --manifest-path src-tauri/Cargo.toml update_`.

### 4.3 `commands/invoices.rs` — lista general

- [x] Test (Rust, `list_invoices_inner` sin `type`): devuelve facturas de todos los tipos. Los filtros `status` agrupan correctamente. Con cualquier `status` != "" se excluyen las pérdidas.
- [x] Test (Rust): orden por fecha descendente (a igualdad, por ID descendente).
- [x] Test (Rust): paginador global funciona con las filas del backend.
- [x] Test (Rust): ya cubierto por §3.1.1; se añaden los casos con datos de varios tipos.
- [x] Implementación: `listInvoices` ya existe; se ajusta para el caso sin tipo.
- **Nota (2026-10-11):** el código ya cubría el caso sin tipo (sin cambios de implementación); se añadió `list_invoices_returns_all_types_in_date_id_order_for_general_list` (6 tipos, orden fecha+ID, agrupación, exclusión de pérdidas, contrato de campos; el paginador es de cliente y recibe filas completas y ordenadas). `npm test` 131/131 en verde. Ejecución Rust pendiente en tu máquina: `cargo test --manifest-path src-tauri/Cargo.toml list_invoices`.

### 4.4 `commands/clients.rs` y `suppliers.rs` — indicadores

- [x] Test (Rust, `get_client_detail_inner`): `totalInvoicedCents` = suma de totales de ventas y servicios no cancelados. `pendingBalanceCents` = suma de los que están `pending`. `invoiceCount` = cantidad. `lastPurchaseDate` = fecha de la más reciente. Garantías e insumos no cuentan.
- [x] Test (Rust, `get_supplier_detail_inner`): `totalPurchasedCents` = suma de entradas no canceladas. `pendingBalanceCents` = suma de las Por Pagar. `invoiceCount` = cantidad. `lastPurchaseDate`.
- [x] Test (Rust): ambos comandos purgan best-effort antes de leer.
- [x] Test (Rust): las facturas de un contacto eliminado no rompen el cálculo.
- [x] Implementación: los indicadores en `get_client_detail_inner` y `get_supplier_detail_inner`. Purga best-effort.
- **Nota (2026-10-11):** además de los indicadores se rellenan `invoiceHistory` (V/S/GG/GI y E, fecha+ID desc) y `expiredCleanupWarning` (patrón `categories.rs`) que pedían CL-03/PV-03 y §5bis. `npm test` 131/131 en verde. Ejecución Rust pendiente en tu máquina: `cargo test --manifest-path src-tauri/Cargo.toml detail`.

### 4.5 Interfaz de Facturas de Salida

#### 4.5.1 Vista de factura de venta

- [x] Test (Vitest + jsdom, `facturasView.js` — submódulo `sale`): pinta solo lo que devuelve el backend. Estados de carga/vacío/error.
- [x] Test (Vitest + jsdom): `#factura-venta-estado` con opciones correctas (`pending`/`completed`/`cancelled`).
- [x] Test (Vitest + jsdom): `#factura-venta-lineas-body` con botón "Agregar Fila" y botón `+` (nuevos).
- [x] Test (Vitest + jsdom): el botón "Editar" del detalle se muestra solo si `status === 'pending'`.
- [x] Test (Vitest + jsdom): `#factura-venta-descuento` y `#factura-venta-impuesto` no existen.
- [x] Test (Vitest + jsdom, **salida segura**): un nombre de cliente con HTML se muestra como texto.
- [x] Implementación: submódulo `sale` de `facturasView.js`. Registra listeners. Añade el botón "Agregar Fila" y el botón `+` al HTML.

#### 4.5.2 Vista de factura de servicio

- [ ] Test (Vitest + jsdom): idéntico a venta, pero la etiqueta del cliente y las columnas de la tabla.
- [ ] Implementación: submódulo `service` de `facturasView.js`.

#### 4.5.3 Vista de factura de garantía

- [ ] Test (Vitest + jsdom): `#factura-garantia-estado` con opciones correctas. **No hay `<select>` de método de pago.**
- [ ] Test (Vitest + jsdom): `#factura-garantia-dropzone` **no existe**. `#factura-garantia-original` es un `<select required>` que se llena con las facturas de venta y servicio del cliente.
- [ ] Test (Vitest + jsdom): al cambiar de cliente, el `<select>` se limpia.
- [ ] Test (Vitest + jsdom): `#factura-garantia-ver-original` muestra `{numero} · {tipo visible}`.
- [ ] Test (Vitest + jsdom): el botón "Crear Nuevo Producto" de garantía **no existe** (el producto se crea al guardar la factura).
- [ ] Test (Vitest + jsdom): los inputs de descuento e impuesto no existen.
- [ ] Implementación: submódulo `guarantee` de `facturasView.js`. Elimina el dropzone.

#### 4.5.4 Vista de factura de insumo

- [ ] Test (Vitest + jsdom): análogo a garantía con `#factura-insumo-servicio`.
- [ ] Implementación: submódulo `installation` de `facturasView.js`.

#### 4.5.5 Vista de factura de pérdida

- [ ] Test (Vitest + jsdom): `#factura-perdida-costo` es `<input type="text" readonly>`. No se envía.
- [ ] Test (Vitest + jsdom): `#factura-perdida-cantidad` es `<input type="text" inputmode="decimal">`.
- [ ] Test (Vitest + jsdom): `#factura-perdida-producto` es combobox sin "Agregar nueva".
- [ ] Test (Vitest + jsdom): el formulario solo pide producto, cantidad, fecha y descripción.
- [ ] Test (Vitest + jsdom): el detalle llena `#factura-perdida-ver-costo-total` y `#factura-perdida-ver-costo` con el costo real.
- [ ] Implementación: submódulo `loss` de `facturasView.js`.

#### 4.5.6 Vista de lista general

- [ ] Test (Vitest + jsdom): `#factura-general-tipo` con los `value` correctos (`""`, `E`, `V`, `S`, `GG`, `GI`, `P`).
- [ ] Test (Vitest + jsdom): `#factura-general-estado` con los `value` correctos (`""`, `pendientes`, `completadas`, `canceladas`).
- [ ] Test (Vitest + jsdom): `renderFacturasGeneral` reescrito con `textContent` y `createElement`. Sin `innerHTML` con datos.
- [ ] Test (Vitest + jsdom): el botón "Ver" abre el detalle del tipo correspondiente.
- [ ] Test (Vitest + jsdom): `facturaTipoInfo` y `obtenerFacturasCombinadas` no existen.
- [ ] Implementación: submódulo `general` de `facturasView.js`. Adapta `renderFacturasGeneral` e `irAFacturaDesdeGeneral`.

#### 4.5.7 Reescritura de vistas con `textContent`

- [ ] **Pre-tarea (O-2):** usar el recuento de `innerHTML` levantado en §0 para dimensionar el trabajo; anotar en cada test cuántos `innerHTML` con datos quedan por vista y verificar que se reduce a 0.
- [ ] Test (Vitest + jsdom, **XSS**): las 12 vistas de facturas (`verFactura`, `verFacturaVenta`, etc.) se reescriben con `textContent`/`createElement`. Un nombre con HTML se muestra como texto.
- [ ] Implementación: reescribir con `textContent` todas las vistas que usaban `innerHTML` con datos.

#### 4.5.8 Adaptación de `exportarFactura*PDF`

- [ ] Test (Vitest + jsdom): cada función `exportarFacturaPDF`, `exportarFacturaVentaPDF`, etc. existe y lee el DTO de `getInvoice` en vez de los arreglos simulados.
- [ ] Implementación: adaptar las 6 funciones para leer `getInvoice`.

#### 4.5.9 Eliminar el mock de facturas de salida

- [ ] Eliminar de `dashboard.js`: `facturasVentaData`, `nextFacturaVentaSeq`, `editingFacturaVentaId`, `facturaVentaLineItems`, `nextVentaLineItemId`, `renderFacturasVenta`, `eliminarFacturaVenta`, `facturaVentaVerNumero`, `verFacturaVenta`, `eliminarFacturaVentaDesdeVer`, `showFacturaVentaTab`, `prepararNuevaFacturaVenta`, `cancelarFacturaVentaForm`, `renderLineasVenta`, `recalcularTotalesVenta`, `guardarFacturaVenta`.
- [ ] Eliminar de `dashboard.js` los análogos para servicios, insumos, garantía y pérdida.
- [ ] Eliminar `facturaTipoInfo` y `obtenerFacturasCombinadas`.
- [ ] Adaptar `renderFacturasGeneral` e `irAFacturaDesdeGeneral`.
- [ ] Eliminar `insumosDisponibles` y `opcionesUnidadHTML`.
- [ ] Eliminar los manejadores inline de las secciones de facturas de salida en el HTML.
- [ ] Eliminar el bloque final de notificaciones iniciales completo, incluida la llamada `crearNotificacion('factura', 'Bienvenido al Sistema', ...)` y el `.forEach`.
- [ ] Eliminar las llamadas a `crearNotificacion` y `notificarFactura` desde facturas y pérdidas.
- [ ] Verificación: `grep` de los identificadores; `dashboard.js` carga sin errores; el listado de la lista general muestra estado vacío con la BD vacía.
- [ ] `npm test` pasa.

### 4.6 Interfaz de Contactos — indicadores

- [ ] Test (Vitest + jsdom, `clientesView.js`): el detalle muestra `totalInvoicedCents`, `pendingBalanceCents`, `invoiceCount`, `lastPurchaseDate` con datos reales.
- [ ] Test (Vitest + jsdom, `proveedoresView.js`): el detalle muestra `totalPurchasedCents`, etc.
- [ ] Test (Vitest + jsdom): el historial de facturas del cliente se llena con `getClientDetail().invoiceHistory`.
- [ ] Implementación: pintar los indicadores y el historial en las vistas.

### 4.7 Interfaz de Productos y Categorías — "Valor Total" e "Ingresos"

- [ ] Test (Vitest + jsdom, `productosView.js`): con datos reales, la columna "Valor Total" y el campo del detalle muestran `valorTotalCents` correcto.
- [ ] Test (Vitest + jsdom, `categoriasView.js`): con datos reales, "Ingresos" muestra `ingresosCents` correcto. Un cambio de categoría del producto no altera las ventas anteriores.
- [ ] Implementación: ya se aplicó en §3.8; se verifica con datos reales aquí.

### 4.8 Verificación de la Fase 3

- [ ] `cargo test` pasa con todos los tests de la Fase 3.
- [ ] `cargo clippy --all-targets -- -D warnings` sin warnings.
- [ ] `npm test` pasa.
- [ ] **Invariante de caché** reforzado con los comandos de esta fase.
- [ ] **Test de contrato `invoke ↔ generate_handler!`** actualizado con los comandos de esta fase.
- [ ] Verificación manual: crear una venta, un servicio, una garantía, un insumo y una pérdida. Comprobar el historial y los indicadores de contactos.
- [ ] `cargo fmt --check` pasa.

---

## 5. Fase 4 — Exportación del historial

*(Independiente una vez que existe el historial. Depende de la Fase 2.)*

### 5.1 `commands/history.rs` — exportación

#### 5.1.1 `exportHistoryExcel`

- [ ] Test (Rust, `export_history_excel_inner`): genera un `.xlsx` con el encabezado y las filas correctas. El archivo existe y no está vacío.
- [ ] Test (Rust): sin movimientos que exportar → `Err` con código `invalidState` y mensaje "No hay movimientos para exportar".
- [ ] Test (Rust): fallo de escritura (destino no escribible) → `Err` con código `internal` y mensaje "No se pudo guardar el archivo".
- [ ] Test (Rust): fila de la matriz de autorización de `exportHistoryExcel`.
- [ ] Implementación: `exportHistoryExcel(input)` con `export.rs::export_history_xlsx`. Purga best-effort antes.

#### 5.1.2 `exportHistoryPdf`

- [ ] Test (Rust, `export_history_pdf_inner`): genera un `.pdf` con el encabezado y las filas. El archivo existe y no está vacío.
- [ ] Test (Rust): pinta `á é í ó ú ü ñ ¿ ¡ €` correctamente (verificación del spike).
- [ ] Test (Rust): sin movimientos → `Err` con "No hay movimientos para exportar".
- [ ] Test (Rust): fallo de escritura → `Err` con "No se pudo guardar el archivo".
- [ ] Test (Rust): fila de la matriz de autorización.
- [ ] Implementación: `exportHistoryPdf(input)` con `export.rs::export_history_pdf`. Purga best-effort antes.

### 5.2 Interfaz de exportación

- [ ] Test (Vitest + jsdom, `historialView.js`): los botones `#historial-exportar-excel` y `#historial-exportar-pdf` existen en el `.historial-toolbar`. Están deshabilitados cuando no hay movimientos, con `title="No hay movimientos para exportar"` y `aria-live="polite"`.
- [ ] Test (Vitest + jsdom): al pulsar "Exportar a Excel" se abre el diálogo nativo (stub) y se llama a `exportHistoryExcel({ destinationPath })` con la ruta.
- [ ] Test (Vitest + jsdom): al pulsar "Exportar a PDF", igual.
- [ ] Test (Vitest + jsdom): si el usuario cancela el diálogo, no se llama al comando.
- [ ] Implementación: en `historialView.js`, registrar listeners en los botones; usar `window.__TAURI__.dialog.save()` para obtener la ruta.

### 5.3 Verificación de la Fase 4

- [ ] `cargo test` pasa con los tests de la Fase 4.
- [ ] `cargo clippy` sin warnings.
- [ ] `npm test` pasa.
- [ ] Verificación manual: exportar el historial de un producto a `.xlsx` y `.pdf`, abrirlos en sus programas y comprobar el contenido.
- [ ] `cargo fmt --check` pasa.

---

## 5 bis. Tests transversales de `expiredCleanupWarning` (H-7)

*(Cada vista que recibe el campo lo muestra de forma discreta (`textContent`, `role="status"`) y lo limpia cuando llega `null`. Se replica el patrón de `tasks-02.md` §8.)*

- [ ] Test (Vitest + jsdom, `clientesView.js`): con `getClientDetail` devolviendo `expiredCleanupWarning` no nulo, se muestra el aviso; con `null`, no.
- [ ] Test (Vitest + jsdom, `proveedoresView.js`): ídem con `getSupplierDetail`.
- [ ] Test (Vitest + jsdom, submódulo `entry` de `facturasView.js`): con `InvoiceActionResult.expiredCleanupWarning` no nulo, se muestra el aviso tras crear/editar/eliminar.
- [ ] Test (Vitest + jsdom, submódulos `sale`, `service`, `guarantee`, `installation`, `loss`): ídem.
- [ ] Test (Vitest + jsdom, `historialView.js`): con `ProductHistoryList.expiredCleanupWarning` no nulo, se muestra el aviso; con `null`, no.
- [ ] Test (Vitest + jsdom, `productosView.js` y `categoriasView.js`): verificar que no se rompen cuando el campo llega `null` (los listados de `spec-02` que no purgan no lo traen).

---

## 6. Cierre del módulo

- [ ] **Revisión final contra `spec-03.md`:** recorrer CL-01 a CL-06, PV-01 a PV-07, LT-01 a LT-04, FE-01 a FE-08, FS-01 a FS-05, FP-01, FG-01, HP-01 a HP-04, SP-01 a SP-04 y confirmar que cada requisito tiene su comando implementado y su test pasando. Recorrer también la §14 de la spec.
- [ ] **Verificación de la matriz de autorización (`Agents.md` §3):** la tabla `(comando, sin sesión → rechazado, ambos roles → permitido)` tiene una fila por cada uno de los **35 comandos** nuevos y cada fila tiene su test pasando. Ningún comando de esta feature queda fuera. Ninguno actualiza `lastActivityAt`.
- [ ] **Verificación de registros:** `validationFailed` e `invalidCredentials` no generan fila en `errorLogs`; `duplicateField`, `notFound`, `invalidState`, `stockInsuficiente`, `stockLimitExceeded`, `exportFailed` e `internal` sí (los que tienen throttle, bajo la política H-A2); ningún registro contiene valores ingresados por el usuario.
- [ ] **Verificación de la purga best-effort:** con un trigger de prueba que haga fallar el `DELETE` sobre `products`, cada comando de la lista de `spec-03` §7.6 devuelve su resultado sin vencidos, incluye `expiredCleanupWarning` y registra el error. La app sigue usable. **`listHistoryProducts` no purga** (decisión de diseño de `plan-03 §3.9`).
- [ ] **Verificación de columnas de auditoría:** `createdAt` y `updatedAt` están en las tablas nuevas con el formato correcto. `consumptions` y `productMovements` solo tienen `createdAt`. **`updatedAt` se refresca en `update*Invoice`, `changeInvoiceStatus` y `saveInvoiceAttachment`** (N-1, O-B).
- [ ] **Verificación de la inmutabilidad:** el trigger `productMovementsImmutable` bloquea un `UPDATE` directo (incluido el `UPDATE` mixto de N-4); el trigger `productMovementsNoDelete` bloquea un `DELETE` directo. Los triggers `invoiceCountersNoDecrease` e `invoiceCountersNoDelete` funcionan.
- [ ] **Verificación del invariante de caché:** tras cada operación sobre lotes, `products.quantityMilli` = suma de `remainingMilli` y `products.costCents` = promedio de los lotes con existencia. **Incluye el escenario H-A1** (eliminar una compra Cancelada con lotes restaurados).
- [ ] **Verificación del esquema (H-11):** `init_schema` crea las 8 tablas nuevas (`clients`, `suppliers`, `invoiceCounters`, `invoices`, `invoiceLines`, `lots`, `consumptions`, `productMovements`) con sus índices y triggers; los tests de §1.5 pasan; ningún `CHECK`/`UNIQUE`/FK/trigger queda sin test.
- [ ] **Verificación H-A1:** el test del escenario completo (compra cancelada + venta cancelada restaura + eliminar compra) pasa y el invariante de caché se mantiene.
- [ ] **Verificación H-A2:** los tests del throttle y de `should_log` con los códigos nuevos pasan; el `HashMap` del throttle se limpia al superar la ventana y el tamaño máximo.
- [ ] **Verificación H-A3:** `validate_stock_limit` se llama en `createProduct`, `createEntryInvoice`, `updateEntryInvoice` y `restore_consumption`; el catálogo incluye `stockLimitExceeded`; los tests de tope pasan.
- [ ] **Verificación H-A4:** los tests de edición de salidas comprueban que el cambio de costo por re-FIFO no genera un Ajuste.
- [ ] **Verificación de la sección de facturas:** el enlace del sidebar del submenú de facturas está visible. Las 7 páginas están visibles. Los botones "Ver Historial" y "Ver Facturas" están visibles. **El `id`/`data-page` real se usa** (verificado en §0).
- [ ] **Verificación de garantías:** ninguna tarea [x] de `tasks-01.md` ni `tasks-02.md` fue reabierta; `should_log()` de `spec-01` sigue sin cambios; no se editaron `auth.css` ni `dashboard.css`; el test existente de `invoke.js` y los tests de `spec-01`/`spec-02` siguen en verde.
- [ ] **Complexity Tracking:** confirmar que `plan-03.md` §9 refleja las desviaciones reales (incluidas las de H-A2, H-A3, H-A4). Si apareció alguna otra durante la implementación, documentarla.
- [ ] **Documentación:** actualizar `Agents.md` §1 ("Estado actual": Autenticación y Usuarios, Inventario-Productos y Contactos-Facturación-Historial implementados) y §7 con los comandos reales; actualizar `pendientes.md` (verificar que PEN-01, PEN-02, PEN-06, PEN-07, PEN-12, PEN-13, PEN-14, PEN-15, PEN-16 y PEN-17 siguen vigentes y que ninguno se resolvió sin querer; el estado de PEN-04, PEN-05 y PEN-09 sigue como resuelto; PEN-10 sigue como parcial). `Constitution.md` **no** cambia salvo que surja una nueva desviación decidida por el dueño.
- [ ] **Formato y lint:** `npx prettier --write` (frontend y tests, sin los bundles vendorizados), `cargo fmt -- --check` y `cargo clippy --all-targets -- -D warnings` sin warnings; no hay `#[allow(non_snake_case)]`.
- [ ] **Normalizar `task-03.md` a LF** (CRLF→LF) para que el diff no muestre todo el archivo modificado.
- [ ] `cargo test` y `npm test` pasan; anotar los recuentos finales.
- [ ] **Test de contrato `invoke ↔ generate_handler!`:** verifica los **76 comandos** (15 de `spec-01` + 26 de `spec-02` + 35 de `spec-03`). El test calcula el recuento a partir de los archivos, no de un literal fijo.
- [ ] **Test de limpieza de `environment.test.js`:** `dashboard.js` no contiene los identificadores eliminados de facturas ni de contactos; no quedan manejadores inline en las secciones migradas; la sección de facturas del dashboard no se ha desmigrado por error.
- [ ] **Verificación manual de extremo a extremo** en la app real, como Administrador y como Contador: crear un cliente, un proveedor, una factura de entrada con adjunto, una venta, un servicio, una garantía, un insumo, una pérdida; consultar el historial; exportar a Excel y PDF; verificar los indicadores y el "Valor Total"/"Ingresos". Comprobar que Historial, Facturas Asociadas, la sección de facturas, Valor Total e Ingresos están visibles. **Pendiente si el entorno no permite operar la ventana nativa de Tauri**; en ese caso se anota explícitamente, como en `tasks-01.md` y `tasks-02.md`.
- [ ] **Pasada en la app compilada (Q-12):** repetir la verificación anterior con `npm run tauri:build` (no solo `tauri:dev`): carga de los `<script type="module">`, listeners que reemplazaron a los `onclick`, `[hidden]`, diálogos con foco, buscadores con teclado y estados vacíos. Si el entorno no permite operar la ventana nativa, se anota como pendiente igual que arriba.
- [ ] **Verificación de la BD de desarrollo:** si durante la implementación se ajustó un `CHECK`, una columna o un índice de una tabla que ya existía en la BD local, borrarla y reabrir la aplicación para regenerarla con el esquema final. Comprobar que `init_schema` crea las 8 tablas nuevas + `invoiceCounters` + `consumptions` + `productMovements` y que la app arranca sin errores en consola.
- [ ] **Commit(s)** siguiendo Conventional Commits, idealmente uno por fase completa de este documento (`Agents.md` §8).