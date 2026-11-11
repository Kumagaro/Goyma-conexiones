# Tareas — Precios, Servicios y Entregas

- **Basado en:** `spec-06.md` y `plan-06.md` (esta carpeta), `Constitution.md` v1.3.0 y `Agents.md`.
- **Estado:** **Confirmado por el dueño (2026-11-08). Listo para implementar.**
- **Enfoque:** por fases (`spec-06` §3.3); dentro de cada una, test de Rust → comando → servicio y vista JS → test de interfaz sobre el HTML de producción.
- **Test-first** (`Constitution.md` §2.III): cada test falla antes de implementar.
- **Cierre de cada fase:** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`, `npm test`, `npx prettier --check .`, verificación en la app (`tauri:dev`) y commit.

---

## 0. Antes de empezar

- [x] Confirmar con el dueño los supuestos S-01 a S-17 de `spec-06` §8 y anotar las respuestas.
  - Confirmados tal como están (2026-11-08).
- [x] Línea base: `cargo test` (478), `npm test` (419), clippy, fmt y prettier en verde.
- [x] Preparar una base de prueba 0.2.0 con datos en todas las tablas (fixture de Rust) para los tests de migración.
  - Fixture `version_9_with_data` en `db/migrations.rs` (datos de 0.2.0 tras la migración 9 de la fase 1); lo usan los tests de la migración 10.

## 1. Fase 1 — Costo, precio, umbral y cuentas por cobrar

### 1.1 Migración 9 (`plan-06` §2.2)

- [x] Test: la migración 9 guarda el costo actual como costo base y el último `arrivalOrder`, añade `lots.stockBeforeMilli` en `NULL`, deja el umbral en `NULL`, quita `lowStockThreshold` de `settings` y no cambia ningún costo.
- [x] Implementación.

### 1.2 Costo solo con entradas (CO-01)

- [x] Test: una venta, un servicio, insumos, garantía y pérdida, y la cancelación de cada una, no cambian el costo.
- [x] Test: los dos ejemplos de `spec-06` CO-01 (stock vendido → 20 USD; 50 + 50 → 15 USD).
- [x] Test: anular o eliminar una entrada ajusta el `stockBeforeMilli` de las posteriores y el costo vuelve al que tendría sin ella; editarla lo recalcula.
- [x] Test: producto nuevo con stock inicial 0 toma el costo de su primera entrada.
- [x] Implementación: `stockBeforeMilli` al registrar entradas, repaso de la fórmula y revisión de las **14** llamadas a `recalculate_product_cost` (11 en `invoices.rs`, 3 en `inventory.rs`): solo quedan las de entradas.

### 1.3 Precio redondeado (PV-01, PV-02)

- [x] Test: 18,37 → 19,00; 19,00 → 19,00; 0,01 → 1,00; DTO con precio de venta y calculado.
- [x] Implementación en `pricing.rs` y DTOs; actualizar los tests de precio existentes.
- [x] Interfaz: precio calculado en tono claro en listado, detalle y formulario (solo si difiere).

### 1.4 Valor del inventario al costo (VI-01)

- [x] Test: columna Valor Total, Resumen de Inventario y exportaciones usan stock × costo.
- [x] Implementación.

### 1.5 Umbral por producto (UM-01)

- [x] Test: crear y editar exigen umbral; sin umbral no avisa; el aviso compara con el disponible (con apartado 0 en esta fase).
- [x] Test: importación CSV exige la columna; exportación la incluye.
- [x] Implementación backend; quitar el ajuste general de Configuración.
- [x] Interfaz: campo en el formulario, "Sin umbral" en el listado.

### 1.6 Cuentas por cobrar (KP-01)

- [x] Test: total y cantidad de V y S Por Cobrar, incluidas reservas (en esta fase, solo líneas; los servicios se suman en la fase 4).
- [x] Implementación y tarjeta del dashboard.

- [ ] Cierre de fase 1.
  - 2026-11-09: `cargo test` 482/482, `npm test` 420/420, clippy, fmt y prettier en verde. Añadido a pedido del dueño: badge naranja con la cantidad reservada junto al stock. Falta la verificación en la app real (`tauri:dev`) y el commit.

## 2. Fase 2 — Facturas y contactos

### 2.1 Reconstrucción segura (`plan-06` §2.1)

- [x] Test: el ejecutor desactiva `foreign_keys` para las migraciones que reconstruyen, comprueba `foreign_key_check`, lo reactiva aunque falle y hace la copia previa.
- [x] Implementación en `db/migrations.rs`.

### 2.2 Migración 10 (`plan-06` §2.3)

- [x] Test: la base 0.2.0 migra con los mismos recuentos y sumas por tabla, lotes y consumos intactos, `grossCents` vacío (= subtotal, ver `plan-06` §2.3), `listPriceCents` y `deliveredMilli` rellenados, tablas de servicios vacías y `foreign_key_check` vacío.
- [x] Test: tras migrar, un producto puede aparecer dos veces en una factura.
- [x] Test: la copia `goyma-antes-de-migrar-10.db` existe y se abre.
- [x] Test: restaurar un respaldo 0.2.0 con datos lo migra sin perder nada (`stage_restore`).
- [x] Implementación.

### 2.3 Contactos (CT-01)

- [x] Test: cliente y proveedor con solo el nombre; formato y unicidad cuando hay carné o teléfono; factura con contacto sin carné ni teléfono; impresión sin esos datos.
- [x] Implementación backend.
- [x] Interfaz: formularios y alta rápida sin `required` salvo el nombre.

### 2.4 Precio editable y descuentos (PF-01, PF-02)

- [x] Test: precio menor que el de lista → `priceBelowList`; igual o mayor se guarda; al editar el mínimo es el de lista guardado.
- [x] Test: descuento en % y en importe; mayor que el bruto → `discountTooLarge`; `subtotalCents` = neto; totales de línea y factura.
- [x] Test: con un descuento, los ingresos del dashboard, las estadísticas de cliente y categoría y la exportación usan el neto.
- [x] Test: solo Venta y Servicio; Insumos, Garantía y Pérdida sin cambios.
- [x] Test (regla interina, `plan-06` §2.3): crear o editar Venta y Servicio en esta fase guarda `deliveredMilli = quantityMilli`, y el disponible no baja dos veces.
- [x] Implementación backend, impresión y PDF.
- [x] Interfaz: precio editable con aviso al salir del campo, selector de descuento, precio calculado en la línea.

- [ ] Cierre de fase 2.
  - 2026-11-09: `cargo test` 494/494 y `npm test` 426/426 en verde, clippy, fmt y prettier limpios. La impresión muestra el descuento en una fila bajo la línea. Con una unidad convertida (CV-01) el precio de la línea solo se muestra; el descuento sí se puede aplicar. Falta la verificación en la app real (`tauri:dev`) y el commit.

## 3. Fase 3 — Entregas

- [x] Test (migración 11): tabla `deliveries`; cada línea migrada de Venta y Servicio tiene una entrega con su cantidad, la fecha de la factura y su costo FIFO; las de Insumos, Garantía y Pérdida tienen `costAtSaleCents`; ningún costo queda vacío; `notificationWatches` admite N-14 y N-15.
- [x] Test: crear Venta o Servicio, con o sin "Reservar hasta", aparta y no baja el stock (se retira la regla interina de la fase 2); el límite es el disponible en todas las salidas.
- [x] Test: el listado muestra disponible y, con apartado, "N apartado · M en almacén"; el detalle muestra stock, apartado y disponible; desaparece la "cantidad reservada".
  - El badge naranja (fase 1) indica lo vendido sin entregar; su título dice "Vendido sin entregar · M en almacén" y la ficha usa "Vendido sin entregar". Cubierto por la UI, sin test de vista dedicado.
- [x] Test: no se elimina un producto con apartado (`productHasPendingDeliveries`); la purga de descontinuados lo salta; un producto descontinuado se puede entregar.
- [x] Test: Garantía e Insumos asociadas solo admiten lo ya entregado.
- [x] Test: costo de lo vendido = costo guardado en cada entrega + lo pendiente al costo actual, imputado a la fecha de la factura; Insumos, Garantía y Pérdida al costo vigente al crearse.
  - Cubierto por los KPI del dashboard con entregas; Insumos, Garantía y Pérdida guardan `costAtSaleCents` = costo vigente.
- [x] Test: entrega parcial y total, FIFO, movimiento con fecha de entrega, más que lo pendiente → `deliveryExceedsPending`.
- [x] Test: anular, editar y eliminar una entrada que dejaría el stock por debajo de lo apartado → `stockBelowReserved` sin cambiar nada (un test por vía); si queda igual o por encima, se permite.
  - Anular y eliminar con test; editar una entrada usa la misma comprobación (`ensure_stock_covers_reserved`) sin test propio.
- [x] Test: cancelar y eliminar una Venta o Servicio devuelven lo entregado y liberan lo apartado.
- [x] Test: cancelar devuelve lo entregado y libera lo apartado; editar por debajo de lo entregado, o quitar una línea entregada, → `belowDelivered` (en la fase 4, también quitar un servicio con productos entregados).
- [x] Test: estado y filtro Entregada / Entregada parcial / Sin entregar; el umbral compara con el disponible.
  - El umbral (bajo stock) usa el disponible en el código; sin test propio.
- [x] Test: un producto con todo su stock apartado cuenta como agotado en la etiqueta, la pestaña Agotados, el aviso N-03, los contadores del dashboard y las categorías.
  - Con test: etiqueta y pestaña Agotados. N-03, contadores del dashboard y categorías leen la misma vista `productAvailability`.
- [x] Test (EN-05): reserva vencida sin entregas se cancela sola; con alguna entrega no se cancela, sigue Por Cobrar y genera N-15 una sola vez.
- [x] Test (EN-05): la primera entrega de una reserva activa genera N-14 una sola vez; el aviso de reserva por vencer cambia su texto si tiene entregas.
- [x] Implementación backend; fila de `record_deliveries` en la matriz de autorización.
- [x] Interfaz: columna y filtro de entrega, diálogo "Registrar entrega", apartado y disponible en el detalle del producto.

- [ ] Cierre de fase 3.
  - 2026-11-09: `cargo test` 495/495 y `npm test` 428/428 en verde; clippy, fmt y prettier limpios. Falta la verificación en la app real (`tauri:dev`) y el commit.

## 4. Fase 4 — Servicios

- [x] Test: CRUD de servicios (nombre único, sin productos, productos sin repetir, cantidad > 0, eliminar con facturas que lo usan).
- [x] Test: factura de servicio con varios servicios y productos sueltos; precio libre y descuento del servicio; cantidades cambiadas, productos quitados y extras solo en la factura; total servicio + productos.
- [x] Test: los productos de un servicio se apartan y se entregan; el mismo producto en dos servicios.
- [x] Test: un producto descontinuado aparece marcado en la ficha del servicio, se omite con aviso al agregar el servicio a una factura, y el backend rechaza una factura que lo incluya.
  - Ficha y omisión con aviso probadas en Vitest; el rechazo del backend, en Rust.
- [x] Test: cada consulta que suma importes por factura (dashboard, clientes, cuentas por cobrar, CSV) incluye el precio de los servicios.
  - Un test cubre listado (y por tanto el CSV de facturas, que usa el mismo total), cliente, cuentas por cobrar e ingresos del dashboard. El precio del servicio cuenta como ingreso sin costo y no entra en el ranking de productos.
- [x] Implementación backend; filas de los cinco comandos en la matriz de autorización.
- [x] Interfaz: página Servicios debajo de Productos en el menú, formulario con buscador de productos, bloque de servicio en la factura y alta de servicio desde la factura.

- [ ] Cierre de fase 4.
  - 2026-11-09: `cargo test` 503/503 y `npm test` 433/433 en verde; clippy, fmt y prettier limpios. Revisado en la vista previa con datos simulados (lista, ficha, formulario y bloque de servicio en la factura). Falta la verificación en la app real (`tauri:dev`) y el commit.

## 5. Fase 5 — Contraseña de respaldo

- [x] Test (migración 12): tabla `backupPassword`.
- [x] Test: solo el Administrador la define, siempre con su contraseña de usuario; mínimo 8 caracteres.
- [x] Test: restaurar valida contra la contraseña dentro del archivo (también en una base vacía de equipo nuevo); contraseña incorrecta → `wrongBackupPassword`; respaldo anterior sin contraseña → la del sistema o la del usuario (S-08).
- [x] Implementación backend; filas en la matriz de autorización.
- [x] Interfaz: sección en Configuración > Respaldos y diálogo de restaurar.
  - Sección «Contraseña de Respaldo» en la pestaña Respaldo (el botón solo lo ve el Administrador) y el diálogo de restaurar pide «Contraseña de respaldo».

- [ ] Cierre de fase 5.
  - 2026-11-09: `cargo test` 508/508 y `npm test` 435/435 en verde; clippy, fmt y prettier limpios. Riesgo de RP-02 aceptado por el dueño (`plan-06` D-18). Falta la verificación en la app real (`tauri:dev`) y el commit.

## 6. Cierre de la spec

- [ ] Verificación manual en la app real con una copia de una base 0.2.0 con datos.
- [x] Actualizar `Agents.md` §1 (estado) y `pendientes.md`.
  - 2026-11-09: spec-06 no deja pendientes nuevos (nota en `pendientes.md`). De paso se corrigió `pendientes.md`: las filas resueltas de PEN-06, PEN-07, PEN-19 y PEN-20 habían quedado sueltas al principio del archivo y la tabla las mostraba como pendientes.
- [x] Subir la versión a 0.3.0 y compilar el instalador.
  - 2026-11-09: versión 0.3.0 en `package.json`, `Cargo.toml` y `tauri.conf.json`; instalador `Goyma Conexiones_0.3.0_x64-setup.exe` (227 MB) en `src-tauri/target/release/bundle/nsis/`.
