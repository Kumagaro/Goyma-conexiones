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
- [ ] Línea base: `cargo test` (478), `npm test` (419), clippy, fmt y prettier en verde.
- [ ] Preparar una base de prueba 0.2.0 con datos en todas las tablas (fixture de Rust) para los tests de migración.

## 1. Fase 1 — Costo, precio, umbral y cuentas por cobrar

### 1.1 Migración 9 (`plan-06` §2.2)

- [ ] Test: la migración 9 guarda el costo actual como costo base y el último `arrivalOrder`, añade `lots.stockBeforeMilli` en `NULL`, deja el umbral en `NULL`, quita `lowStockThreshold` de `settings` y no cambia ningún costo.
- [ ] Implementación.

### 1.2 Costo solo con entradas (CO-01)

- [ ] Test: una venta, un servicio, insumos, garantía y pérdida, y la cancelación de cada una, no cambian el costo.
- [ ] Test: los dos ejemplos de `spec-06` CO-01 (stock vendido → 20 USD; 50 + 50 → 15 USD).
- [ ] Test: anular o eliminar una entrada ajusta el `stockBeforeMilli` de las posteriores y el costo vuelve al que tendría sin ella; editarla lo recalcula.
- [ ] Test: producto nuevo con stock inicial 0 toma el costo de su primera entrada.
- [ ] Implementación: `stockBeforeMilli` al registrar entradas, repaso de la fórmula y revisión de las **14** llamadas a `recalculate_product_cost` (11 en `invoices.rs`, 3 en `inventory.rs`): solo quedan las de entradas.

### 1.3 Precio redondeado (PV-01, PV-02)

- [ ] Test: 18,37 → 19,00; 19,00 → 19,00; 0,01 → 1,00; DTO con precio de venta y calculado.
- [ ] Implementación en `pricing.rs` y DTOs; actualizar los tests de precio existentes.
- [ ] Interfaz: precio calculado en tono claro en listado, detalle y formulario (solo si difiere).

### 1.4 Valor del inventario al costo (VI-01)

- [ ] Test: columna Valor Total, Resumen de Inventario y exportaciones usan stock × costo.
- [ ] Implementación.

### 1.5 Umbral por producto (UM-01)

- [ ] Test: crear y editar exigen umbral; sin umbral no avisa; el aviso compara con el disponible (con apartado 0 en esta fase).
- [ ] Test: importación CSV exige la columna; exportación la incluye.
- [ ] Implementación backend; quitar el ajuste general de Configuración.
- [ ] Interfaz: campo en el formulario, "Sin umbral" en el listado.

### 1.6 Cuentas por cobrar (KP-01)

- [ ] Test: total y cantidad de V y S Por Cobrar, incluidas reservas (en esta fase, solo líneas; los servicios se suman en la fase 4).
- [ ] Implementación y tarjeta del dashboard.

- [ ] Cierre de fase 1.

## 2. Fase 2 — Facturas y contactos

### 2.1 Reconstrucción segura (`plan-06` §2.1)

- [ ] Test: el ejecutor desactiva `foreign_keys` para las migraciones que reconstruyen, comprueba `foreign_key_check`, lo reactiva aunque falle y hace la copia previa.
- [ ] Implementación en `db/migrations.rs`.

### 2.2 Migración 10 (`plan-06` §2.3)

- [ ] Test: la base 0.2.0 migra con los mismos recuentos y sumas por tabla, lotes y consumos intactos, `grossCents` = `subtotalCents`, `listPriceCents` y `deliveredMilli` rellenados, tablas de servicios vacías y `foreign_key_check` vacío.
- [ ] Test: tras migrar, un producto puede aparecer dos veces en una factura.
- [ ] Test: la copia `goyma-antes-de-migrar-10.db` existe y se abre.
- [ ] Test: restaurar un respaldo 0.2.0 con datos lo migra sin perder nada (`stage_restore`).
- [ ] Implementación.

### 2.3 Contactos (CT-01)

- [ ] Test: cliente y proveedor con solo el nombre; formato y unicidad cuando hay carné o teléfono; factura con contacto sin carné ni teléfono; impresión sin esos datos.
- [ ] Implementación backend.
- [ ] Interfaz: formularios y alta rápida sin `required` salvo el nombre.

### 2.4 Precio editable y descuentos (PF-01, PF-02)

- [ ] Test: precio menor que el de lista → `priceBelowList`; igual o mayor se guarda; al editar el mínimo es el de lista guardado.
- [ ] Test: descuento en % y en importe; mayor que el bruto → `discountTooLarge`; `subtotalCents` = neto; totales de línea y factura.
- [ ] Test: con un descuento, los ingresos del dashboard, las estadísticas de cliente y categoría y la exportación usan el neto.
- [ ] Test: solo Venta y Servicio; Insumos, Garantía y Pérdida sin cambios.
- [ ] Test (regla interina, `plan-06` §2.3): crear o editar Venta y Servicio en esta fase guarda `deliveredMilli = quantityMilli`, y el disponible no baja dos veces.
- [ ] Implementación backend, impresión y PDF.
- [ ] Interfaz: precio editable con aviso al salir del campo, selector de descuento, precio calculado en la línea.

- [ ] Cierre de fase 2.

## 3. Fase 3 — Entregas

- [ ] Test (migración 11): tabla `deliveries`; cada línea migrada de Venta y Servicio tiene una entrega con su cantidad, la fecha de la factura y su costo FIFO; las de Insumos, Garantía y Pérdida tienen `costAtSaleCents`; ningún costo queda vacío; `notificationWatches` admite N-14 y N-15.
- [ ] Test: crear Venta o Servicio, con o sin "Reservar hasta", aparta y no baja el stock (se retira la regla interina de la fase 2); el límite es el disponible en todas las salidas.
- [ ] Test: el listado muestra disponible y, con apartado, "N apartado · M en almacén"; el detalle muestra stock, apartado y disponible; desaparece la "cantidad reservada".
- [ ] Test: no se elimina un producto con apartado (`productHasPendingDeliveries`); la purga de descontinuados lo salta; un producto descontinuado se puede entregar.
- [ ] Test: Garantía e Insumos asociadas solo admiten lo ya entregado.
- [ ] Test: costo de lo vendido = costo guardado en cada entrega + lo pendiente al costo actual, imputado a la fecha de la factura; Insumos, Garantía y Pérdida al costo vigente al crearse.
- [ ] Test: entrega parcial y total, FIFO, movimiento con fecha de entrega, más que lo pendiente → `deliveryExceedsPending`.
- [ ] Test: anular, editar y eliminar una entrada que dejaría el stock por debajo de lo apartado → `stockBelowReserved` sin cambiar nada (un test por vía); si queda igual o por encima, se permite.
- [ ] Test: cancelar y eliminar una Venta o Servicio devuelven lo entregado y liberan lo apartado.
- [ ] Test: cancelar devuelve lo entregado y libera lo apartado; editar por debajo de lo entregado, o quitar una línea entregada, → `belowDelivered` (en la fase 4, también quitar un servicio con productos entregados).
- [ ] Test: estado y filtro Entregada / Entregada parcial / Sin entregar; el umbral compara con el disponible.
- [ ] Test: un producto con todo su stock apartado cuenta como agotado en la etiqueta, la pestaña Agotados, el aviso N-03, los contadores del dashboard y las categorías.
- [ ] Test (EN-05): reserva vencida sin entregas se cancela sola; con alguna entrega no se cancela, sigue Por Cobrar y genera N-15 una sola vez.
- [ ] Test (EN-05): la primera entrega de una reserva activa genera N-14 una sola vez; el aviso de reserva por vencer cambia su texto si tiene entregas.
- [ ] Implementación backend; fila de `record_deliveries` en la matriz de autorización.
- [ ] Interfaz: columna y filtro de entrega, diálogo "Registrar entrega", apartado y disponible en el detalle del producto.

- [ ] Cierre de fase 3.

## 4. Fase 4 — Servicios

- [ ] Test: CRUD de servicios (nombre único, sin productos, productos sin repetir, cantidad > 0, eliminar con facturas que lo usan).
- [ ] Test: factura de servicio con varios servicios y productos sueltos; precio libre y descuento del servicio; cantidades cambiadas, productos quitados y extras solo en la factura; total servicio + productos.
- [ ] Test: los productos de un servicio se apartan y se entregan; el mismo producto en dos servicios.
- [ ] Test: un producto descontinuado aparece marcado en la ficha del servicio, se omite con aviso al agregar el servicio a una factura, y el backend rechaza una factura que lo incluya.
- [ ] Test: cada consulta que suma importes por factura (dashboard, clientes, cuentas por cobrar, CSV) incluye el precio de los servicios.
- [ ] Implementación backend; filas de los cinco comandos en la matriz de autorización.
- [ ] Interfaz: página Servicios debajo de Productos en el menú, formulario con buscador de productos, bloque de servicio en la factura y alta de servicio desde la factura.

- [ ] Cierre de fase 4.

## 5. Fase 5 — Contraseña de respaldo

- [ ] Test (migración 12): tabla `backupPassword`.
- [ ] Test: solo el Administrador la define, siempre con su contraseña de usuario; mínimo 8 caracteres.
- [ ] Test: restaurar valida contra la contraseña dentro del archivo (también en una base vacía de equipo nuevo); contraseña incorrecta → `wrongBackupPassword`; respaldo anterior sin contraseña → la del sistema o la del usuario (S-08).
- [ ] Implementación backend; filas en la matriz de autorización.
- [ ] Interfaz: sección en Configuración > Respaldos y diálogo de restaurar.

- [ ] Cierre de fase 5.

## 6. Cierre de la spec

- [ ] Verificación manual en la app real con una copia de una base 0.2.0 con datos.
- [ ] Actualizar `Agents.md` §1 (estado) y `pendientes.md`.
- [ ] Subir la versión a 0.3.0 y compilar el instalador.
