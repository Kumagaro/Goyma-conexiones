# Plan: Precios, Servicios y Entregas

- **Basado en:** `spec-06.md` (esta carpeta), `Constitution.md` v1.3.0, `Agents.md`, `plan-03` (facturas, lotes) y `plan-04` (migraciones, respaldo).
- **Estado:** **Confirmado por el dueño (2026-11-08).**
- **Sin dependencias nuevas.** Todo con `rusqlite`, `bcrypt` y `chrono`, ya presentes.

---

## 1. Estado actual que cambia

| Hoy | Dónde | Cambio |
|---|---|---|
| Costo = promedio de `lots.remainingMilli` (cambia al vender y al cancelar ventas) | `inventory.rs::recalculate_product_cost` y 14 llamadas | Promedio móvil solo con entradas (§3.1) |
| Costo de lo vendido = consumos FIFO de lotes | `commands/dashboard.rs` (consulta de `consumptions`) | Costo guardado en cada entrega (§3.4) |
| Subtotal de línea = cantidad × precio | `subtotalCents`, leído en 13 archivos | `subtotalCents` = neto tras descuento; bruto aparte (§2.3) |
| Reserva descuenta stock al guardarse; "cantidad reservada" en el detalle | `commands/invoices.rs`, `commands/products.rs` | Aparta como toda venta (EN-03) |
| Precio = costo × (1 + %), redondeo a céntimos | `pricing.rs::selling_price_cents` | Techo al entero (§3.2) |
| `invoiceLines` con `UNIQUE (invoiceId, productIdOriginal)` | `db/schema.rs` | Un producto puede repetirse (servicios) |
| Instantánea de contacto obligatoria (`contactFullName` 5–101, carné y teléfono `NOT NULL`) | `invoices` | Solo el nombre |
| `clients`/`suppliers` con apellidos, carné y teléfono `NOT NULL` | `db/schema.rs` | Opcionales |
| Venta y Servicio consumen lotes al guardar | `commands/invoices.rs` | Consumen al entregar (§3.4) |
| Ajuste `lowStockThreshold` general | `settings`, `automation.rs`, `products.rs` | Columna por producto |
| Restaurar pide la contraseña del usuario | `backup.rs::verify_password` | Contraseña de respaldo (§3.6) |
| Las migraciones corren en una transacción con `foreign_keys = ON` | `db/migrations.rs::run_migrations_with` | Soporte de reconstrucción (§2.1) |

## 2. Esquema (migraciones 9 a 12)

### 2.1 Reconstruir tablas sin perder datos (prerrequisito de la migración 10)

SQLite no puede quitar un `UNIQUE` ni un `CHECK` con `ALTER TABLE`: hay que crear la tabla nueva, copiar, borrar la vieja y renombrar. Con `foreign_keys = ON`, **borrar `invoices` o `invoiceLines` dispara los `ON DELETE CASCADE` y borra lotes, consumos y líneas**. Y `PRAGMA foreign_keys` no tiene efecto dentro de una transacción.

- `Migration` gana una marca `rebuilds_tables`. Para esas, el ejecutor hace `PRAGMA foreign_keys = OFF` **antes** de abrir la transacción; dentro, al terminar, `PRAGMA foreign_key_check` debe devolver cero filas (si no, rollback y `migrationFailed`); después del commit vuelve a `ON` siempre, también si falló.
- Antes de cualquier migración que reconstruya, se hace una copia `goyma-antes-de-migrar-<versión>.db` (prefijo distinto de `goyma-previa-`, la copia anterior a restaurar, y de `goyma-auto-`) con `VACUUM INTO` en la carpeta de respaldos (reutiliza `backup.rs`). La hace `db/connection.rs::initialize_database`, que conoce `app_data_dir`, antes de llamar a `run_migrations` cuando alguna migración pendiente reconstruye; `run_migrations` no recibe rutas.
- Los índices y disparadores de cada tabla reconstruida se vuelven a crear con el mismo nombre.
- **Test obligatorio:** una base 0.2.0 con datos en todas las tablas (lotes, consumos, movimientos, garantías asociadas, adjuntos) migra con los mismos recuentos y sumas por tabla y `foreign_key_check` vacío.
- **Restaurar un respaldo 0.2.0** también ejecuta estas migraciones: `backup.rs::stage_restore` migra la copia con `foreign_keys = ON` (líneas 261-263). Como el ejecutor desactiva las claves por sí mismo, sirve igual; ese camino no hace copia previa porque ya trabaja sobre una copia. Lleva su propio test.

### 2.2 Migración 9 — Costo base y umbral (fase 1)

- `products`: `costBaseCents INTEGER NOT NULL DEFAULT 1 CHECK (costBaseCents BETWEEN 1 AND 99999999)`, `costBaseArrivalOrder INTEGER NOT NULL DEFAULT 0`, `lowStockThresholdMilli INTEGER CHECK (lowStockThresholdMilli IS NULL OR lowStockThresholdMilli BETWEEN 1 AND 999999999)`.
- `lots`: `stockBeforeMilli INTEGER CHECK (stockBeforeMilli IS NULL OR stockBeforeMilli BETWEEN 0 AND 999999999)`: stock físico del producto justo antes de esa entrada (§3.1). `NULL` en los lotes anteriores al costo base, que no se usan para recalcular.
- Datos: `costBaseCents = costCents`, `costBaseArrivalOrder = COALESCE(MAX(lots.arrivalOrder), 0)` del producto; `lowStockThresholdMilli` queda `NULL` (UM-01).
- Se borra la fila `lowStockThreshold` de `settings` (el `CHECK` de claves la sigue admitiendo; no se reconstruye `settings` por esto).

### 2.3 Migración 10 — Facturas y contactos (fase 2, `rebuilds_tables`)

- `clients`, `suppliers`: `lastName`, `nationalId`, `phone` pasan a admitir `NULL`, con el mismo formato si no son `NULL`; `nationalId` sigue `UNIQUE` (SQLite admite varios `NULL`).
- `invoices`: `contactFullName` 2–101; `contactNationalId` y `contactPhone` admiten `NULL`; el resto de `CHECK` igual.
- `invoiceLines`:
  - se quita `UNIQUE (invoiceId, productIdOriginal)`;
  - `listPriceCents INTEGER` (precio de lista, PF-01; `NULL` en Entrada, Pérdida, Garantía e Insumos);
  - `grossCents INTEGER` (bruto = cantidad × precio): **vacío cuando no hay descuento**, así las demás salidas (entradas, pérdidas, garantías, insumos) no cambian sus `INSERT` (implementación, 2026-11-09);
  - `discountKind TEXT CHECK (discountKind IN ('percent','amount'))`, `discountValue INTEGER` (centésimas de % o céntimos), `discountCents INTEGER NOT NULL DEFAULT 0 CHECK (discountCents BETWEEN 0 AND grossCents)`;
  - **`subtotalCents` pasa a ser el neto:** `CHECK` de `discountCents`: sin bruto, descuento 0; con bruto, `discountCents BETWEEN 0 AND grossCents AND subtotalCents = grossCents - discountCents`. Así los 13 archivos que lo leen (dashboard, clientes, proveedores, categorías, productos, búsqueda, CSV, inventario, impresión) cuentan ya el importe con descuento sin cambiar sus consultas; solo la impresión y el formulario muestran además bruto y descuento;
  - `serviceLineId INTEGER REFERENCES invoiceServiceLines(id) ON DELETE CASCADE` (fase 4; se crea ya para reconstruir una sola vez);
  - `deliveredMilli INTEGER NOT NULL DEFAULT 0 CHECK (deliveredMilli BETWEEN 0 AND quantityMilli)` (fase 3, mismo motivo).
- `services`, `serviceProducts` (§2.5) e `invoiceServiceLines` se crean **en esta migración**, vacías, porque `invoiceLines` referencia a `invoiceServiceLines` y esta a `services`.
- `invoiceServiceLines`: `id`, `invoiceId` (`ON DELETE CASCADE`), `position`, `serviceId` (`ON DELETE SET NULL`), `serviceName`, `priceCents`, `discountKind`, `discountValue`, `discountCents`, `totalCents` (`= priceCents − discountCents`, sin los productos), fechas.
- Datos existentes: `grossCents = subtotalCents`; `listPriceCents = unitPriceCents` en Venta y Servicio; `deliveredMilli = quantityMilli` en Venta y Servicio no canceladas (EN-03); descuentos en 0.
- **Regla interina hasta la fase 3:** mientras crear o editar Venta y Servicio siga consumiendo lotes al guardar (comportamiento actual), esas operaciones guardan `deliveredMilli = quantityMilli`. Así las facturas creadas al probar la fase 2 no quedan a la vez descontadas y apartadas. La fase 3 retira esta regla en el mismo cambio que deja de consumir al guardar, y la migración 11 trata esas líneas como las demás entregadas.

### 2.4 Migración 11 — Entregas (fase 3)

- `deliveries`: `id`, `invoiceLineId` (`ON DELETE CASCADE`), `quantityMilli` (> 0), `unitCostCents` (costo vigente al entregar, EN-04), `deliveredAt`, `userId`, `userName`. Una fila por entrega; `invoiceLines.deliveredMilli` es su suma (se mantiene en la misma transacción).
- Las líneas migradas como entregadas reciben **una** fila en `deliveries` con su cantidad, fecha = fecha de la factura y `unitCostCents` = costo FIFO que ya tenían (Σ consumos / cantidad).
- Las líneas existentes de Insumos, Garantía y Pérdida reciben `costAtSaleCents` = su costo FIFO (Σ consumos / cantidad).
- Las dos son necesarias para que el dashboard tenga una sola fuente de costo sin valores vacíos; que además conserven el costo histórico es una consecuencia, no un objetivo (`spec-06`, datos anteriores).
- `notificationWatches`: su `CHECK` de tipos admite además `N-14` y `N-15` (EN-05). Es una tabla sin claves foráneas: se reconstruye dentro de esta migración sin la marca `rebuilds_tables`.

### 2.5 Servicios (tablas de la migración 10, se usan en la fase 4)

- `services`: `id`, `name` (2–100), `nameNormalized`, `description`, `suggestedPriceCents` (`NULL` o 0–9999999999), fechas; índice único de `nameNormalized`.
- `serviceProducts`: `serviceId` (`ON DELETE CASCADE`), `productId` (`ON DELETE CASCADE`), `suggestedQuantityMilli` (> 0), `position`; `UNIQUE (serviceId, productId)`.
- Contador de facturas sin cambios (los servicios no son un tipo de factura). La fase 4 no necesita migración.

### 2.6 Migración 12 — Contraseña de respaldo (fase 5)

- `backupPassword`: `id INTEGER PRIMARY KEY CHECK (id = 1)`, `passwordHash TEXT NOT NULL`, `updatedAt`, `updatedByUserId`. Al vivir en la base, cada respaldo (copia de la base) la lleva dentro (RP-02).

## 3. Reglas en código

### 3.1 Costo (CO-01, `inventory.rs`)

```
costo = costBaseCents
para cada lote con origin = 'entry' y arrivalOrder > costBaseArrivalOrder, en orden de llegada:
    sb = stockBeforeMilli; q = initialMilli; c = unitCostCents
    costo = (sb × costo + q × c) / (sb + q)        (redondeo a la mitad hacia arriba)
```

- **Al registrar una entrada:** `stockBeforeMilli` = `products.quantityMilli` antes de sumarla; el costo se actualiza con un paso de la fórmula (no hace falta repasar).
- **Al anular o eliminar una entrada** de cantidad `q`: en la misma transacción, los lotes de entrada posteriores del mismo producto bajan su `stockBeforeMilli` en `q` (sin bajar de 0); después se repasa la fórmula completa. **Al editarla**, lo mismo con la diferencia de cantidad, y si la edición recrea el lote, conserva su `arrivalOrder` y su `stockBeforeMilli`.
- `recalculate_product_cost` pasa a ser el repaso completo y **solo** se llama desde crear, editar, anular y eliminar **entradas**. Hoy tiene **14 llamadas**: 11 en `commands/invoices.rs` y 3 en `inventory.rs` (`restore_consumption` y `reverse_lots_of_invoice`, que también corren al cancelar ventas). Se revisan una a una; las de salidas y cancelaciones de salidas se quitan, y cada caso lleva su test.
- Crear producto: `costBaseCents` = costo inicial; `costBaseArrivalOrder` = el del lote inicial (o 0 si no tiene).
- Los lotes de restitución (cancelaciones) no cuentan en la fórmula.

### 3.2 Precio (PV-01, `pricing.rs`)

- `calculated_price_cents(cost, profit)` = la función actual (céntimos, mitad hacia arriba).
- `selling_price_cents(cost, profit)` = `calculated` redondeado hacia arriba a múltiplo de 100.
- Los DTO de producto devuelven `sellingPriceCents` y `calculatedPriceCents`; el frontend pinta el calculado solo si difiere.

### 3.3 Precio editable y descuentos (PF, `commands/invoices.rs`)

- Venta y Servicio: cada línea de producto recibe `unitPriceCents`, `discountKind` y `discountValue`. El backend fija `listPriceCents` (precio de venta actual al crear; el guardado al editar) y rechaza `unitPriceCents < listPriceCents` con `priceBelowList`.
- `discountCents`: `percent` → bruto × valor / 10 000 (mitad hacia arriba); `amount` → valor. Mayor que el bruto → `discountTooLarge`. `subtotalCents` = bruto − descuento.
- Total de la factura = Σ `subtotalCents` de todas sus líneas (sueltas y de servicios) + Σ `totalCents` de sus servicios. Las consultas que hoy suman `subtotalCents` por factura deben sumar también `invoiceServiceLines.totalCents` (dashboard, clientes, cuentas por cobrar, CSV): se listan en la fase 4 con un test cada una.
- Impresión y PDF (`invoice_print.rs`, `printPreview.js`): la línea muestra el bruto y, debajo, una fila «Descuento» en negativo; el total es el neto (implementación, 2026-11-09).
- Los movimientos de salida (`productMovements`) guardan el precio neto por unidad y el importe proporcional a la cantidad entregada.

### 3.4 Apartado y entregas (EN, `inventory.rs` + `commands/invoices.rs`)

- `reserved_milli(product)` = Σ (`quantityMilli` − `deliveredMilli`) de líneas de Venta y Servicio no canceladas.
- `validate_stock_limit` usa disponible = `quantityMilli` − apartado (en todas las salidas).
- Crear o editar Venta y Servicio, **también con "Reservar hasta"**: **no** consume lotes ni registra movimientos; solo valida el disponible. Cambia `spec-04` RS-01.
- `commands/products.rs` deja de calcular la "cantidad reservada" (reservas activas) y devuelve `reservedMilli` (apartado) y `availableMilli`; el listado muestra el disponible y, con apartado, también el stock en almacén.
- **Entradas y apartado:** las tres vías que bajan el stock de una entrada (editar con ajuste `adjustment` en `commands/invoices.rs`, anular con `change_invoice_status_inner` y eliminar con `delete_invoice_inner`) comprueban, por producto, que el stock resultante sea ≥ apartado; si no, `stockBelowReserved` sin cambiar nada. Una función común en `inventory.rs`, llamada desde las tres.
- Eliminar una Venta o Servicio (`delete_invoice_inner`) devuelve sus consumos (lo entregado, como hoy); sus `deliveries` y lo apartado desaparecen por `ON DELETE CASCADE`.
- `delete_product_inner` rechaza un producto con apartado > 0 (`productHasPendingDeliveries`), y la purga de descontinuados (`purge_best_effort`, que también corre desde `automation.rs`) lo excluye. `record_deliveries` acepta productos descontinuados.
- Garantía e Insumos asociadas validan, por producto, la cantidad **entregada** de la factura asociada (`create_associated_outgoing_invoice_inner`).
- `record_deliveries(invoiceId, [{lineId, quantityMilli}])`: valida pendiente y stock físico, consume lotes por FIFO (`consume_fifo`), registra `deliveries` con el costo vigente, actualiza `deliveredMilli`, registra `productMovements` (`saleOut`/`serviceOut`) y la actividad. Una transacción.
- **Costo de lo vendido (EN-04, `commands/dashboard.rs`):** la consulta deja de leer `consumptions` para Venta y Servicio y usa Σ `deliveries.quantityMilli × unitCostCents` + (`quantityMilli − deliveredMilli`) × `products.costCents`; Insumos, Garantía y Pérdida usan `costAtSaleCents` (costo vigente al crearlas). Todo imputado a `invoiceDate`. `consumptions` sigue guardando de qué lotes salió la cantidad, solo para devolverla al cancelar.
- Cancelar (`reverse_lots_of_invoice`): devuelve los consumos existentes (lo entregado); lo pendiente se libera solo, porque una factura cancelada no aparta.
- **Reservas (EN-05, `commands/invoices.rs::expire_reservations` y `automation.rs`):** la consulta de vencidas excluye las facturas con `deliveredMilli > 0` en alguna línea. Para esas, el mismo pase genera N-15 una vez (vigilancia `N-15` + id de factura). `record_deliveries` genera N-14 cuando es la primera entrega de una reserva activa. El aviso previo de reserva por vencer cambia su texto si la reserva tiene entregas.
- Editar: rechaza `quantityMilli < deliveredMilli`, y quitar una línea o un servicio con productos entregados, con `belowDelivered`.
- Estado de entrega derivado (no se guarda): todas entregadas → `delivered`; ninguna → `pending`; resto → `partial`. Filtro en `list_invoices`.

### 3.5 Servicios (SV, `commands/services.rs`, nuevo)

- CRUD con validación de nombre único, precio sugerido y productos (sin repetidos, cantidad > 0, producto activo al añadirlo).
- `get_service` devuelve el estado de cada producto; el frontend marca los descontinuados en la ficha y los omite al agregar el servicio a una factura, con aviso. El backend rechaza igualmente una factura con productos descontinuados (regla actual de las salidas).
- Factura de servicio: `services: [{serviceId | null, name, priceCents, discount…, products: [{productId, quantityMilli, unitPriceCents, discount…}]}]` + `lines` sueltas. Un servicio creado desde la factura usa el mismo comando de alta (como el alta rápida de productos).
- Instantánea: el nombre y el precio del servicio se copian en `invoiceServiceLines`; sus productos son `invoiceLines` con `serviceLineId`.

### 3.6 Contraseña de respaldo (RP, `backup.rs`)

- `set_backup_password(userPassword, newPassword)`: `require_admin`, verifica la contraseña del usuario, guarda bcrypt.
- `restore_backup_inner(…, backupPassword)`: abre el archivo origen en solo lectura y lee su `backupPassword`; si existe, verifica contra él; si el archivo no tiene esa tabla o fila, usa la del sistema y, si tampoco hay, la del usuario (S-08). Error `wrongBackupPassword`.

### 3.7 Umbral (UM, `automation.rs`, `products.rs`, `csv_io.rs`)

- Crear y editar producto exigen `lowStockThresholdMilli` (en la unidad del producto, admite decimales).
- El aviso N-02 y la etiqueta "Stock Bajo" comparan el disponible con el umbral del producto; los productos sin umbral no avisan.
- **Agotado = disponible 0** (fase 3, cuando existe el apartado). Hoy se calcula con `quantityMilli = 0` en cuatro sitios que pasan a usar el disponible: `commands/dashboard.rs` (contadores y listado de agotados), `automation.rs` (aviso N-03), `commands/categories.rs` (`is_out_of_stock`) y la pestaña Agotados y Descontinuados de `commands/products.rs`. Conviene una sola expresión SQL de disponible en `inventory.rs`, reutilizada por las cuatro.
- `get_settings`/`update_settings` dejan de exponer `lowStockThreshold`. Los tests que construyen ajustes con `low_stock_threshold` (`backup.rs`, `activity.rs`) se actualizan.

### 3.8 Cuentas por cobrar (KP, `commands/dashboard.rs`)

- `receivables`: `COUNT` y total de facturas `V`/`S` con `status = 'pending'`. En la fase 1 suma solo `subtotalCents` de las líneas (la tabla de servicios aún no existe); la fase 4 le añade `invoiceServiceLines.totalCents` (§3.3). Sustituye a `reservations_summary` en la tarjeta; antes de quitar `reservations_summary` se comprueba con `grep` si la usa otra parte.

## 4. Comandos Tauri

### 4.1 Nuevos

| Comando | Rol | Fase |
|---|---|---|
| `record_deliveries` | Sesión | 3 |
| `list_services`, `get_service`, `create_service`, `update_service`, `delete_service` | Sesión | 4 |
| `set_backup_password`, `get_backup_password_status` | Administrador / Sesión | 5 |

Cada uno con su fila en la matriz de autorización (`Agents.md` §3).

### 4.2 Que cambian

- `create_product`, `update_product`, `get_product`, `list_products`: umbral, precio calculado, apartado y disponible.
- `create_invoice`, `update_invoice`, `get_invoice`, `list_invoices`: precios, descuentos, servicios, entregas y filtro de entrega.
- `update_invoice`, `change_invoice_status`, `delete_invoice` (entradas): comprobación de stock ≥ apartado (`stockBelowReserved`).
- `delete_invoice` (Venta y Servicio): libera lo apartado y devuelve lo entregado.
- `delete_product`: rechaza productos con apartado (`productHasPendingDeliveries`).
- `get_dashboard_summary`: cuentas por cobrar; valor de inventario al costo.
- `restore_backup`: el parámetro `password` pasa a ser la contraseña de respaldo.
- `create_client`, `update_client`, `create_supplier`, `update_supplier`: campos opcionales.
- `get_settings`, `update_settings`: sin `lowStockThreshold`.
- Exportaciones (`export.rs`, `csv_io.rs`): valor al costo, precio calculado, umbral.

### 4.3 Errores nuevos

`priceBelowList`, `discountTooLarge`, `quantityExceedsAvailable`, `deliveryExceedsPending`, `belowDelivered`, `productHasPendingDeliveries`, `stockBelowReserved`, `serviceNameTaken`, `wrongBackupPassword`, `backupPasswordRequired` (solo validación). Mensajes en español, sin detalles técnicos.

## 5. Frontend

- **Menú:** `Servicios` debajo de `Productos` (en `knownPages` de `dashboard.js`, grupo Operación).
- **Nuevos:** `services/servicesService.js`, `views/serviciosView.js` (listado, formulario con buscador de productos y cantidades, detalle), página en `dashboard-unificado.html`.
- **Productos:** campo umbral, precio calculado en tono claro (`.price-calculated` en `components.css`), "Sin umbral", apartado y disponible en el detalle.
- **Facturas (`facturasView.js`):** precio editable con aviso al salir del campo, descuento (selector % / USD + valor), bloque de servicio con sus productos, columna y filtro de entrega, diálogo "Registrar entrega" (cantidad pendiente por línea).
- **Dashboard:** tarjeta Cuentas por cobrar.
- **Contactos:** quitar `required` salvo el nombre, en formularios y alta rápida.
- **Configuración > Respaldos:** sección Contraseña de respaldo (solo Administrador); el diálogo de restaurar pide la contraseña de respaldo.
- Todo con los componentes y tokens de `ui-spec.md`.

## 6. Testing

- **Rust:** fórmula de costo (los dos ejemplos de `spec-06` CO-01, anular, editar, ventas y sus cancelaciones no lo mueven), redondeo, precio mínimo y descuentos, neto en `subtotalCents`, apartado y entregas parciales, costo de lo vendido por entrega, cancelación con entregas, reservas con y sin entregas al vencer y sus avisos N-14 y N-15, garantía sobre lo entregado, restaurar un respaldo 0.2.0, servicios, umbral, restauración con contraseña dentro del archivo, respaldo sin contraseña, y las migraciones 9–12 sobre una base 0.2.0 con datos (§2.1).
- **Frontend (Vitest):** servicios, formulario de factura con precio, descuento y servicio, registrar entrega, umbral, tarjeta del dashboard, contactos opcionales, sección de contraseña de respaldo, sobre el HTML de producción.

## 7. Decisiones técnicas

- **D-01 — Promedio móvil desde un costo base:** respeta "se queda el costo actual" sin depender de facturas antiguas, sigue las compras recientes y permite recalcular sin una entrada anulada. Guardar `stockBeforeMilli` en cada lote evita reconstruir el stock histórico a partir de los movimientos.
- **D-02 — Un producto puede repetirse en una factura:** necesario para servicios con productos comunes; se quita el `UNIQUE` y la búsqueda de líneas pasa a usar el `id` de línea, nunca el `productId`.
- **D-03 — Una sola reconstrucción de `invoiceLines`:** la migración 10 crea también las columnas de entregas y servicios, aunque se usen en las fases 3 y 4, para no reconstruir la tabla tres veces.
- **D-04 — Copia previa a la migración:** las reconstrucciones son la única operación del proyecto que puede perder datos si fallan a medias; la copia permite volver atrás.
- **D-05 — La contraseña de respaldo no cifra:** decisión del dueño (`spec-06` §3.2).
- **D-06 — `subtotalCents` es el neto:** cambiar su significado una vez en el esquema es más seguro que adaptar las 13 consultas que lo leen.
- **D-07 — Entregas migradas con su costo FIFO:** el costo de lo vendido tiene una sola fuente (`deliveries`) sin valores vacíos. Que el histórico no cambie es una consecuencia, no un objetivo (`spec-06`, datos anteriores).
- **D-08 — El stock nunca baja de lo apartado:** lo apartado no consume lotes, así que las vías que reducen stock sin pasar por el disponible (entradas) lo comprueban explícitamente.
- **D-09 — Vista `productAvailability`:** apartado y disponible salen de una sola vista (migración 11) que leen productos, categorías, dashboard, avisos y purga; así "agotado" no puede divergir entre pantallas.
- **D-10 — Entrega por posición de línea:** `record_deliveries` recibe `{ position, quantity }`, porque desde D-02 un producto puede repetirse en la factura.
- **D-11 — Insumos, Garantía y Pérdida al costo vigente:** salen de los lotes al crearse (FIFO) pero su costo guardado es el `costCents` actual del producto (S-02), igual que el de las entregas.
- **D-12 — Entregas sin fila en el registro de actividad:** el `CHECK` de `activityLog` no admite una acción de entrega; queda el movimiento del producto (con usuario y fecha) y la tabla `deliveries`. Si hace falta, se añade en una migración posterior.
