# Spec: Contactos, Facturación e Historial de Productos

- **Feature:** `contactos-facturacion-historial`
- **Estado:** **Confirmada por el dueño. Lista para implementar.** Incorpora todas las decisiones confirmadas en las rondas de preguntas, las auditorías QA previas y las respuestas a las 25 preguntas de clarificación del frontend real (A1–A2, B1–B12, C1–C12). Todos los supuestos de la sección 11 están confirmados. **Revisiones posteriores aplicadas:** consistencia (C-1 a C-7), auditoría QA del módulo (H-1 a H-11, O-1 a O-4), segunda ronda QA (N-1 a N-4, O-B) y auditoría de inventario (H-A1 a H-A4). Incluye la regla de eliminación de compras Canceladas con lotes restaurados (H-A1), el tope de stock por producto (H-A3), la política de registro con throttle (H-A2) y la regla de que un cambio de costo por re-FIFO sin cambio de cantidad no genera Ajuste (H-A4).
- **Basada en:** `Requisitos_del_sistema.docx` (RF-032 a RF-097), `Constitution.md` v1.2.0, `Agents.md`, `spec-01` (autenticación y usuarios), `spec-02` (inventario y productos), `pendientes.md`, y el frontend real `dashboard-unificado.html` + `dashboard.js`.
- **Convención de referencias:** `spec-01`, `spec-02`, `spec-03` se refieren a las specs de `autenticacion-y-usuarios`, `inventario-productos` y `contactos-facturacion-historial` respectivamente. `plan-01`, `plan-02`, `plan-03` y `tasks-01`, `tasks-02`, `tasks-03` siguen el mismo criterio. Las referencias internas a esta spec usan `§N` o `§N.M`. Las referencias a archivos se hacen por su nombre: `Constitution.md`, `Agents.md`, `pendientes.md`, `dashboard-unificado.html`, `dashboard.js`.
- **Siguiente paso:** `plan-03.md` confirmado; `tasks-03.md` en 4 fases cerrables por separado (sección 3.3).

---

## 1. Resumen

Esta spec fusiona seis módulos de los requisitos porque comparten un mismo núcleo: el **inventario por lotes**, que decide cuánto cuesta cada unidad que entra y qué unidades salen en cada operación.

- **Clientes y Proveedores** (RF-032 a RF-043): CRUD, búsqueda, detalle con indicadores e historial de facturas, y alta rápida desde una factura.
- **Facturas de entrada / compras** (RF-044 a RF-051): aumentan el inventario creando lotes.
- **Facturas de salida** (RF-052 a RF-085): venta, servicio, gasto de garantía y gasto de insumos de instalación; descuentan inventario por orden de llegada.
- **Facturas de pérdida** (RF-086 a RF-090).
- **Historial de productos** (RF-091 a RF-096): registro inmutable de cada movimiento, consulta, filtros y exportación a Excel y PDF.
- **Cambios a `spec-02`**: la cantidad y el costo de un producto pasan a derivarse de sus lotes (sección 6, módulo "Cambios a `spec-02`").

Incluye además la lista general de facturas del mockup (todos los tipos), la sección "Facturas Asociadas" del producto y las columnas "Valor Total" (productos) e "Ingresos" (categorías), que `spec-02` dejó ocultas.

## 2. Objetivo / Por qué

El negocio compra productos a proveedores, los vende o los usa en servicios a clientes y a veces los pierde o los repone por garantía. Necesita saber en todo momento cuánto tiene, cuánto le costó realmente cada unidad que sale y qué pasó con cada producto. Debe garantizarse que:

- Todo lo que se ve proviene de la base de datos y de comandos del backend (`Constitution.md` §2.I): desaparecen los datos simulados de Clientes, Proveedores y todas las secciones de facturas del mockup.
- El inventario nunca queda negativo ni inconsistente: cada unidad que sale se descuenta de un lote concreto, y toda anulación devuelve exactamente lo que se descontó.
- El costo de lo que sale es el **costo real** de los lotes consumidos, no un promedio que oculte diferencias entre compras.
- Una factura es un documento histórico: conserva una copia de los datos del contacto y del producto del momento, de modo que eliminar un cliente, un proveedor o un producto no la rompe.
- El historial de un producto explica cada cambio de su cantidad, quién lo hizo y con qué factura, y no se puede alterar.

**Cachés derivadas de los lotes (`quantityMilli` y `costCents`):**

- **`products.quantityMilli`** y **`products.costCents`** se conservan como **cachés** sincronizadas con los lotes. No son la fuente de verdad (los lotes lo son).
- Se recalculan dentro de la **misma transacción** que cualquier operación sobre lotes (crear producto, crear/editar/cancelar/eliminar factura de entrada, crear/editar/cancelar/eliminar factura de salida, pérdida).
- Se mantienen como cachés por dos motivos: (1) simplifican las consultas de listado y de DTOs existentes de `spec-02`; (2) evitan recalcular el promedio de lotes en cada lectura.
- **Desviación de `Constitution.md` §4** (Base de datos): la igualdad `quantityMilli = suma(lotes.remainingMilli)` y `costCents = promedio_ponderado(lotes con existencia)` **no se puede expresar** con un `CHECK` de SQLite. Se garantiza con sincronización en código + tests. Se registrará en el Complexity Tracking de `plan-03.md`.

## 3. Alcance

### 3.1 Incluido en esta spec

- CRUD de clientes y de proveedores, búsqueda por relevancia, detalle con indicadores, historial de facturas y alta rápida desde la factura (CL-01 a CL-06, PV-01 a PV-07).
- Modelo de **lotes** y reglas de consumo FIFO, devolución y costo del producto (LT-01 a LT-04).
- Facturas de entrada: crear (con alta de proveedor y de producto sobre la marcha), listar, detalle, editar, eliminar, cambiar estado y adjunto PDF (FE-01 a FE-07).
- Facturas de salida: venta, servicio, gasto de garantía y gasto de insumos, con sus reglas comunes (FS-01 a FS-05).
- Facturas de pérdida (FP-01) y lista general de facturas (FG-01).
- Historial de productos: registro automático, pestaña de consulta con filtros, exportación a Excel y PDF, y "Facturas Asociadas" (HP-01 a HP-04).
- Cambios a `spec-02`: creación de producto con lote inicial, costo derivado de lotes, "Valor Total", "Ingresos" y agotados reales (SP-01 a SP-04).
- Reemplazo de los datos simulados de estas secciones por datos reales vía comandos Tauri.

### 3.2 Fuera de alcance de esta spec (registrado en la sección 10 y en `pendientes.md`)

- **Respaldo y migración** (PEN-15): exportar e importar toda la información del programa para actualizar o mover la aplicación. **Reemplaza a RF-097** (importar historial desde Excel), que no se implementa.
- Notificaciones (pestaña, contador, N-02, N-03, N-04, N-07 y el aviso de "producto agotado al intentar vender"): siguen en el módulo de Notificaciones (PEN-01, PEN-02).
- **Notificaciones del mockup:** las llamadas a `crearNotificacion` y `notificarFactura` de los formularios de facturas y pérdidas **se eliminan** en esta spec. **La pestaña de Notificaciones (`#page-notificaciones`) y su código se mantienen intactos.** La única diferencia es que, tras esta spec, la lista arrancará vacía al no haber notificaciones generadas por facturas/pérdidas. La generación de notificaciones desde facturas se especifica en el módulo de Notificaciones (PEN-01, PEN-02). El bloque de inicialización de notificaciones al final de `dashboard.js` —que genera notificaciones a partir de facturas "Vencida" del mockup y crea la notificación "Bienvenido al Sistema"— **se elimina completo**.
- KPIs del dashboard, "Mejor Vendidos" y "Lento Movimiento" (PEN-16).
- Uso de las conversiones entre unidades en compras y ventas (PEN-06): las cantidades de una línea se expresan siempre en la unidad del producto.
- Otras monedas y tasas de cambio (PEN-07): todo está en USD. El método de pago no implica moneda. **La sección Configuración del mockup se mantiene tal como está hoy** hasta que se implemente PEN-07.
- Descuentos e impuestos en las facturas: **se eliminan** del HTML. La spec no los respalda y el mockup los muestra sin RF de origen. Se eliminan los 10 inputs de descuento e impuesto de los formularios de facturas, y las funciones `recalcularTotales*` de `dashboard.js` dejan de aplicarlos. El total de una factura es la suma de los subtotales de sus líneas (§7.2).
- Registro de quién eliminó o canceló qué (PEN-13). Sí se registra quién creó cada factura y quién ejecutó cada movimiento del historial.
- Estado "Vencida" del mockup: no existe; las facturas no cambian de estado por el paso del tiempo.
- Exportación a PDF del historial con alfabetos no latinos (PEN-17): cubierta Latin-1/WinAnsi; si el proyecto cambia de contexto, se incrusta una fuente TrueType en `export.rs`.

### 3.3 Fases de implementación

Esta spec es grande. Se redacta como una sola porque los lotes atraviesan todos los módulos, pero `tasks-03.md` se organiza en cuatro fases que se pueden cerrar, probar y confirmar por separado:

1. **Contactos:** clientes y proveedores (sin indicadores que dependan de facturas).
2. **Lotes, compras e historial:** modelo de lotes, facturas de entrada, cambios a `spec-02`, historial (registro y consulta).
3. **Salidas:** venta, servicio, garantía, insumos, pérdidas, lista general, indicadores de contactos, "Valor Total" e "Ingresos".
4. **Exportación del historial** a Excel y PDF.

Los comandos de cada fase se enumeran en §5.5. La fase 2 no puede empezar sin la fase 1 (las facturas de entrada referencian proveedores); la fase 3 necesita la 2 (las salidas consumen los lotes que crean las entradas); la fase 4 es independiente una vez que existe el historial (fase 2).

## 4. Roles y actores

**Ambos roles, Administrador y Contador, pueden realizar todas las acciones de esta spec**, igual que en `spec-02` §4: los requisitos dicen "la contadora" porque describen al usuario principal. Toda acción exige una sesión activa y válida (`require_session`); no hay acciones exclusivas del Administrador. Cada comando nuevo tiene su fila en la matriz de tests de autorización (`Agents.md` §3): sin sesión → rechazado; con cualquiera de los dos roles → permitido.

## 5. Modelo conceptual

(Conceptual — el esquema concreto corresponde a `plan-03.md`.) Todas las tablas nuevas usan ID entero autoincremental que nunca se reutiliza y columnas de auditoría `createdAt`/`updatedAt`, como en `spec-02`.

### 5.1 Cliente y Proveedor

Dos entidades independientes con los mismos campos (una persona puede ser cliente y proveedor):

| Campo | Regla |
|---|---|
| ID | Autoincremental; se muestra con relleno de ceros (`#00001`). Es el "ID en Sistema" del mockup, de solo lectura. |
| Nombre(s) | Obligatorio, 2–50 caracteres. |
| Apellidos | Obligatorio, 2–50 caracteres. |
| CI | Obligatorio, 11 dígitos numéricos. **Único** dentro de su tabla. |
| Teléfono | Obligatorio, `+53` seguido de 8 dígitos. **No es único.** |
| Empresa | Opcional, hasta 100 caracteres. |
| Cargo | Opcional, hasta 100 caracteres. |
| Correo | Opcional, formato simple `texto@dominio.ext` sin espacios, hasta 100 caracteres. El patrón exacto se fija en §7.1. |
| Dirección | Opcional, hasta 200 caracteres. |
| Información de factura | Solo clientes. Opcional, hasta 200 caracteres. |
| Fecha de alta | Automática, **no editable en el formulario de alta/edición**. El `<input>` "Fecha de Entrada" del mockup se elimina; la fecha se calcula en el backend al crear y se muestra solo en el detalle (como `createdAt`). |

### 5.2 Lote e inventario

- Un **lote** es una partida de un producto que entró en una sola operación: la **existencia inicial** al crear el producto, o **una línea de una factura de entrada**. Guarda el producto, su origen, la cantidad inicial, la **cantidad restante**, el **costo unitario** (centavos de USD) y su **orden de llegada** (el orden en que se registró).
- La **cantidad de un producto** es siempre la suma de las cantidades restantes de sus lotes. Un producto con cantidad 0 está **Agotado**.
- El **costo del producto** (el "Precio de entrada" que ya muestra `spec-02`) es el promedio ponderado de los lotes con cantidad restante, redondeado al centavo (mitad hacia arriba). Si no queda existencia, conserva el último costo calculado. El precio de venta sigue derivándose del costo y del % de ganancia (`spec-02` P-11).
- Cada **salida de inventario** (venta, servicio, garantía, insumos, pérdida) guarda, por línea, de qué lotes consumió, cuánta cantidad de cada uno y a qué costo unitario. Con eso se puede devolver exactamente lo consumido. **El costo unitario se guarda en el consumo, no solo en el lote:** es el dato que sobrevive a la eliminación del lote (vínculo `NULL` por `SET NULL`) y el que se usa para registrar una Reversión cuando el lote ya no existe (§7.7, HP-01).
- **Tope de stock por producto (H-A3):** la suma de `remainingMilli` de todos los lotes de un producto no puede superar **999.999.999 milésimas** (999.999,999 unidades). Es el mismo máximo que admiten `stockAfterMilli` y cada lote, así que no requiere `CHECK` nuevos: se valida en el código antes de cada operación que pueda aumentar el stock.

### 5.3 Factura

| Tipo | Prefijo | Contacto | Estados | Total |
|---|---|---|---|---|
| Entrada (compra) | `E-` | Proveedor | Por Pagar · Pagada · Cancelada | Suma de cantidad × costo |
| Venta | `V-` | Cliente | Por Cobrar · Cobrada · Cancelada | Suma de cantidad × precio de venta |
| Servicio | `S-` | Cliente | Por Cobrar · Cobrada · Cancelada | Suma de cantidad × precio de venta de los insumos |
| Gasto de garantía | `GG-` | Cliente + factura de venta o servicio original | Pendiente · Completada · Cancelada | Costo real de los lotes consumidos |
| Gasto de insumos | `GI-` | Cliente + factura de servicio asociada | Pendiente · Completada · Cancelada | Costo real de los lotes consumidos |
| Pérdida | `P-` | — | Sin estados | Costo real de los lotes consumidos |

**Campos por tipo de factura:**

| Campo | Entrada | Venta | Servicio | Garantía | Insumo | Pérdida |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Proveedor | ✅ | — | — | — | — | — |
| Cliente | — | ✅ | ✅ | ✅ | ✅ | — |
| Fecha | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Descripción | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Observaciones | ✅ | ✅ | ✅ | ✅ | ✅ | — |
| Método de pago | ✅ | ✅ | ✅ | — | — | — |
| Fecha de pago/cobro | ✅ | ✅ | ✅ | — | — | — |
| PDF adjunto | ✅ | — | — | — | — | — |
| Factura asociada | — | — | — | ✅ | ✅ | — |
| Estados | 3 | 3 | 3 | 3 | 3 | 0 |

Todos los campos opcionales se guardan como `NULL` si están vacíos.

Cada factura tiene: número, fecha (la elige el usuario, por defecto hoy), fecha y hora de creación, **responsable** (usuario que la creó, con copia de su nombre), líneas, total, observaciones o descripción y estado. Las facturas de entrada, venta y servicio pueden tener **método de pago** y **fecha de pago o cobro**. Las de entrada pueden tener un **PDF adjunto**.

**Una única tabla `invoices` con discriminador `type`:** en lugar de seis tablas paralelas, existe una tabla `invoices` con una columna `type` (`'E'`, `'V'`, `'S'`, `'GG'`, `'GI'`, `'P'`) y una tabla `invoiceLines` con `invoiceId`. La lista general (FG-01) es una consulta única; los vínculos polimórficos (líneas, movimientos del historial, factura asociada) son uniformes. El contrato de entrada se separa por tipo (§5.5), no la tabla.

**Numeración — tabla `invoiceCounters`:** un contador independiente por tipo, persistido en una tabla `invoiceCounters(type, nextNumber)`. En la misma transacción que crea la factura se incrementa el contador del tipo y se forma `{type}-{nextNumber:05}`. El contador **nunca se decrementa ni se reinicia**, ni al eliminar ni al cancelar una factura; al superar 99999 muestra los dígitos necesarios (por ejemplo `E-100000`). No se usa `MAX(number) + 1` (reutilizaría el número de una factura eliminada, violando §7.4) ni un `AUTOINCREMENT` (es global, no por tipo).

**El `unitPriceCents` de la línea se guarda** en la línea al crear/editar la factura. El subtotal de la línea es `quantityMilli × unitPriceCents / 1000` (redondeado a centavos una sola vez), y el total de la factura es la suma de subtotales. **El precio de la línea no se recalcula al leer**: lo que se emitió es lo que se muestra, aunque el costo del producto cambie después. El `unitPriceCents` de la línea es:

- En **entradas**: el costo unitario del lote (que se consume íntegro en esta línea).
- En **ventas y servicios**: el precio de venta vigente al crear la línea (derivado del costo y del `%` de ganancia en ese momento).
- En **garantía, insumos y pérdidas**: el costo unitario **de la línea**, calculado con la **misma función** que el precio unitario del movimiento del historial (HP-01): `line_unit_cost_cents(line) = round_half_up(line_total_cost_cents / line_quantity_milli)`, todo en aritmética entera. Este valor es idéntico al precio unitario que registra el movimiento.

Las líneas de **venta y servicio** guardan además:
- El **`categoryId` (o el nombre de la categoría) que el producto tenía en el momento de la venta**. **El `categoryId` y el `categoryName` de la línea son snapshots sin clave foránea**, para que el histórico de "Ingresos" sobreviva a la eliminación de la categoría.
- El **costo unitario** del producto en el momento de la venta, para poder calcular el margen de la línea sin consultar el costo actual.
- La **unidad del producto** (`unitId`, `unitName` y `unitAbbreviation`): el `unitId` es un vínculo (`ON DELETE SET NULL`) y las dos copias textuales sobreviven a la eliminación de la unidad.

Las líneas de entrada, garantía, insumos y pérdida no necesitan la copia de categoría ni de costo unitario de venta, pero sí la de unidad. Una factura guarda también una copia de los datos del contacto (**nombre completo, empresa, CI, teléfono y correo**). El correo se conserva aunque el contacto se elimine. Esas copias son las que se muestran siempre; el vínculo con el contacto o el producto se mantiene mientras existan (§7.7).

### 5.4 Movimiento de historial

Registro **inmutable** (solo se agregan filas, nunca se editan ni se borran) con: fecha y hora, producto (con copia del nombre y el ID original), tipo, cantidad con signo, stock resultante, precio unitario, precio total, factura asociada (número y tipo, con copia) y usuario que ejecutó la operación (con copia del nombre).

### 5.5 Comandos Tauri

Los comandos nuevos de esta feature (**35 en total**) se enumeran aquí para que `plan-03.md`, `tasks-03.md` y el test de contrato `invoke ↔ generate_handler!` tengan el recuento cerrado. La convención de nombres es la de `plan-01` §4: nombre lógico `camelCase` (wrapper JS) y función Rust `snake_case`.

| Módulo | Comandos |
|---|---|
| `commands/clients.rs` | `listClients`, `searchClients`, `getClientDetail`, `createClient`, `updateClient`, `deleteClient` |
| `commands/suppliers.rs` | `listSuppliers`, `searchSuppliers`, `getSupplierDetail`, `createSupplier`, `updateSupplier`, `deleteSupplier` |
| `commands/invoices.rs` | `listInvoices`, `getInvoice`, `createEntryInvoice`, `createSaleInvoice`, `createServiceInvoice`, `createGuaranteeInvoice`, `createInstallationInvoice`, `createLossInvoice`, `updateEntryInvoice`, `updateSaleInvoice`, `updateServiceInvoice`, `updateGuaranteeInvoice`, `updateInstallationInvoice`, `deleteInvoice`, `changeInvoiceStatus`, `saveInvoiceAttachment`, `downloadInvoiceAttachment` |
| `commands/history.rs` | `getProductHistory`, `listHistoryProducts`, `exportHistoryExcel`, `exportHistoryPdf` |
| `commands/products.rs` (ampliación) | `searchProductsForInvoice`, `getProductAssociatedInvoices` |

**Decisiones estructurales:**

- **`create*Invoice` y `update*Invoice` por tipo (11 comandos: 6 `create*` + 5 `update*`; la pérdida no se edita), no un comando genérico con discriminador:** los campos difieren entre tipos y `#[serde(deny_unknown_fields)]` convive mal con enums internamente etiquetados en serde. Un struct de entrada por tipo permite que el compilador imponga el contrato.
- **`listInvoices`, `getInvoice`, `deleteInvoice`, `changeInvoiceStatus`, `saveInvoiceAttachment` y `downloadInvoiceAttachment` genéricos:** el tipo no cambia la semántica de listar, obtener, borrar, cambiar estado, adjuntar o descargar. `listInvoices` con `type` opcional cubre FE-02, FS-01 (listar), FP-01 (listar) y FG-01 (lista general); la lista general es `listInvoices` sin filtro de tipo. `changeInvoiceStatus` recibe `status` y, opcionalmente, `paymentMethod`; el backend valida que la transición sea legal para el tipo.
- **`searchProductsForInvoice`:** `listProducts` de `spec-02` usa `instr` (subcadena simple); las facturas piden **relevancia** (exacto > prefijo > subcadena). No se complica `listProducts`; se añade un comando específico que devuelve hasta 20 resultados con `isOutOfStock` e `isDiscontinued`.
- **`saveInvoiceAttachment`:** recibe `invoiceId` y `sourcePath` (que el frontend obtiene del diálogo nativo de Tauri vía `tauri-plugin-dialog`). El backend copia el archivo a `app_data_dir/attachments/` con nombre UUID, valida cabecera PDF y tamaño, y responde `Ok` o `notFound` "El archivo adjunto no se encuentra en el equipo" si ya no está.
- **`downloadInvoiceAttachment`:** recibe `invoiceId` y `destinationPath`. Copia el archivo desde `app_data_dir/attachments/` al destino y responde `Ok`; si el archivo ya no está en disco, responde `notFound` con el texto ya fijado en §8.2. No escribe en la base de datos: solo lee `attachmentFileName` de `invoices` y copia el archivo.
- **`getProductAssociatedInvoices`:** la sección "Facturas Asociadas" (HP-04). Devuelve hasta 50 facturas de cualquier tipo que incluyan el producto, de la más reciente a la más antigua. **Busca por `productIdOriginal`** (el producto puede estar eliminado y la factura aún tiene la copia del ID en la línea); no busca por `productId` (que queda en `NULL` si el producto se elimina).
- **`createProductFromInvoice` no es un comando aparte:** es un campo opcional (`newProduct`) dentro del `input` de `createEntryInvoice`.
- **DTO `InvoiceListItem` (compartido por todas las listas):** incluye un campo opcional `associatedInvoice: {number: string, type: string} | null`, que solo se rellena en las facturas de **garantía** y **de insumos**. En `InvoiceListItem`, `type` viaja como el código del prefijo (`'E'`, `'V'`, `'S'`, `'GG'`, `'GI'`, `'P'`); la traducción a texto visible es del frontend.
- **El listado de proveedores para el filtro de PV-07 usa `listSuppliers`;** no se añade un comando aparte.
- **`previewInvoiceTotals` no existe.** La vista previa del total mientras se arma una factura la compone **el frontend** sumando las entradas del usuario (cantidad × precio/costo de cada línea). El backend recalcula el total autoritativo al guardar. Ver §7.2.

Los DTOs y los structs de entrada se fijarán en la sección "DTOs" de `plan-03.md`, con la forma de `plan-02` §4: structs de entrada por tipo con `#[serde(deny_unknown_fields, rename_all = "camelCase")]`, DTOs de respuesta con `#[serde(rename_all = "camelCase")]`, y un `InvoiceDetail` con campos opcionales según el tipo en lugar de un enum.

## 6. Requisitos funcionales

Cada requisito indica el RF de origen y sus escenarios de aceptación en formato Dado/Cuando/Entonces. Las reglas de texto, números, fechas, numeración y estados están en la sección 7.

### Módulo: Clientes

#### CL-01 — Crear cliente (RF-032)

- **Dado** un usuario autenticado, **cuando** completa Nombre(s), Apellidos, CI y Teléfono (y, opcionalmente, Empresa, Cargo, Correo, Dirección e Información de factura) con datos válidos, **entonces** el cliente se crea con ID autoincremental y fecha de alta automática.
- **Dado** un CI que ya pertenece a otro cliente, **entonces** se deniega indicando que ese carnet ya está registrado. El teléfono repetido se acepta.
- **Dado** un campo obligatorio vacío, con formato inválido o fuera de rango (7.1), **entonces** el backend rechaza el formulario con un mensaje claro que dice qué corregir.

#### CL-02 — Ver y buscar clientes (RF-033)

- **Dado** un usuario autenticado, **cuando** abre la pestaña Clientes, **entonces** ve los clientes con ID, nombre completo, empresa y teléfono, ordenados alfabéticamente por apellidos y luego por nombre (§7.1 de `spec-02`), con el paginador global.
- **Búsqueda** (7.1): coincidencia sobre nombre, apellidos, nombre completo, CI, teléfono y empresa, con el orden de relevancia de 7.1.

#### CL-03 — Detalle del cliente (RF-034)

- **Dado** un cliente, **cuando** el usuario lo selecciona, **entonces** ve su perfil (nombre completo, CI, teléfono, correo, dirección, empresa, cargo, información de factura, fecha de alta), sus indicadores y su **historial de facturas**.
- **Indicadores:** **Total facturado** = suma de los totales de sus facturas de venta y servicio no canceladas; **Saldo pendiente** = suma de las que están Por Cobrar; **Número de facturas** = cantidad de facturas de venta y servicio no canceladas; **Última compra** = fecha de la más reciente de ellas. Las garantías e insumos no se cobran y no cuentan en los indicadores.
- **Historial de facturas:** todas las facturas de venta, servicio, garantía e insumos del cliente, de la más reciente a la más antigua, con número, tipo, fecha, total y estado; cada una abre su detalle. Un cliente sin facturas muestra un estado vacío claro.

#### CL-04 — Editar cliente (RF-035)

- **Dado** un cliente, **cuando** abre la edición, **entonces** el formulario aparece precargado y puede modificar cualquier campo salvo ID y fecha de alta.
- Las validaciones de unicidad del CI excluyen la propia fila.
- **Dado** que se edita un cliente con facturas, **entonces** las facturas existentes **no cambian**: conservan la copia de los datos del momento (7.7).

#### CL-05 — Eliminar cliente (RF-036)

- **Dado** un cliente, **cuando** elige eliminar, **entonces** el sistema pide confirmación y, si tiene facturas, indica cuántas tiene y que conservarán su información.
- **Dado** que confirma, **entonces** el cliente se elimina físicamente y sus facturas conservan la copia de sus datos; el cliente ya no aparece en el buscador.
- **Dado** que el cliente ya no existe, **entonces** se responde "El cliente ya no existe" (8.2).

#### CL-06 — Crear cliente rápido desde una factura (RF-037)

- **Dado** un formulario de factura de venta, servicio, garantía o insumos, **cuando** el buscador de clientes no encuentra coincidencias, **entonces** muestra "Cliente no encontrado" con la opción "Crear nuevo cliente", que abre el formulario rápido con los mismos campos y reglas de CL-01.
- **Dado** que el cliente se guarda, **entonces** queda creado de inmediato (aunque luego se cancele la factura, como S-7 de `spec-02`) y se selecciona en la factura.
- **Regla general de "creación sobre la marcha":** las **entidades simples** (contactos, categorías, unidades) se crean al elegirlas, aunque la operación que las originó se cancele después. Las **entidades con dependencia operativa** (productos) se crean al guardar la factura, porque necesitan cantidad y costo de la línea para existir.

### Módulo: Proveedores

#### PV-01 a PV-06 — CRUD de proveedores (RF-038 a RF-043)

Los proveedores se comportan exactamente como los clientes en cuanto a campos y reglas (5.1, 7.1), con estas diferencias:

- **PV-01 Crear** (RF-038), **PV-02 Ver y buscar** (RF-039), **PV-04 Editar** (RF-041), **PV-05 Eliminar** (RF-042) y **PV-06 Crear proveedor rápido desde la factura de entrada** (RF-043, mensaje "Proveedor no encontrado" y opción "Crear nuevo proveedor") siguen CL-01, CL-02, CL-04, CL-05 y CL-06.
- Los proveedores **no tienen** "Información de factura".
- **PV-03 Detalle del proveedor** (RF-040): perfil e **historial de facturas de entrada**, con los indicadores **Total comprado** (suma de entradas no canceladas), **Saldo pendiente** (suma de las Por Pagar), **Número de facturas** (no canceladas) y **Última compra** (fecha de la más reciente no cancelada).

#### PV-07 — Historial de compras del proveedor (pestaña del mockup)

- **Dado** un usuario autenticado, **cuando** abre la pestaña "Historial de Compras", **entonces** elige un proveedor y un rango de fechas opcional y ve una fila por factura de entrada con fecha, estado, cantidad de productos, total, número de factura y responsable. Se ordena por fecha (el encabezado alterna ascendente y descendente) y usa el paginador global.
- Solo se listan los proveedores que existen. Las facturas de un proveedor eliminado se consultan en la lista de facturas de entrada (FE-02).

### Módulo: Inventario por lotes

#### LT-01 — Creación de lotes

- **Dado** que se crea un producto (SP-01), **entonces** se crea su **lote inicial** con la cantidad y el costo ingresados, el más antiguo del producto.
- **Dado** que se guarda una factura de entrada, **entonces** cada línea crea **un lote** nuevo con orden de llegada posterior a todos los existentes del producto.
- Los lotes con cantidad restante 0 **se conservan** mientras exista su origen (factura de entrada o producto), porque permiten revertir consumos.
- **Tope (H-A3):** antes de crear el lote, se comprueba que la suma de `remainingMilli` de todos los lotes del producto más la cantidad del lote nuevo no supere 999.999.999 milésimas; en caso contrario, el comando responde `stockLimitExceeded` (§8.2).

#### LT-02 — Consumo FIFO

- **Dado** una salida de inventario (venta, servicio, garantía, insumos o pérdida), **entonces** se consumen los lotes del producto con cantidad restante, de **más antiguo a más reciente por orden de llegada**, consumiendo cada uno por completo antes de pasar al siguiente. El orden de llegada es el de **registro**, no la fecha escrita en la factura.
- Cada línea guarda los consumos (lote, cantidad, costo unitario del lote). El **costo de la línea** es la suma de cantidad consumida × costo del lote, con aritmética entera exacta y un único redondeo al centavo (mitad hacia arriba) por línea. El costo unitario que se muestra es ese costo dividido entre la cantidad de la línea, con la **misma función** que el precio unitario del movimiento (`line_unit_cost_cents`, §5.3 y HP-01).
- **Ejemplo:** lotes de 10 u a $1,00, 5 u a $3,00 y 8 u a $2,00. Vender 12 u consume 10 del primero y 2 del segundo (costo $16,00). Quedan 3 u a $3,00 y 8 u a $2,00, y el costo del producto pasa a (3 × 3 + 8 × 2) / 11 = $2,27.
- **Dado** una cantidad solicitada mayor que la existencia, **entonces** la operación se rechaza completa con un mensaje que indica el stock disponible; nunca se consume parcialmente ni queda negativo.

#### LT-03 — Costo del producto

- **Dado** cualquier operación que cambie lotes, **entonces**, dentro de la misma transacción, el costo del producto se recalcula como el promedio ponderado de los lotes con existencia, redondeado al centavo (mitad hacia arriba).
- **Dado** que no queda existencia, **entonces** el costo conserva su último valor. **Dado** que luego llega una compra, **entonces** el costo pasa a ser el de esa compra.
- **Sin ninguna venta**, el resultado coincide con el promedio ponderado de `spec-02` P-10 (10 u a $1,00 y 5 u a $2,00 → $1,33).
- El precio de venta se recalcula con el % de ganancia guardado (P-11). **Consecuencia aceptada por el dueño:** cuando un lote barato se agota, el costo y por tanto el precio de venta del producto suben. Las facturas ya emitidas conservan el precio con que se hicieron.
- **Compatibilidad con `spec-02`:** la columna `quantityMilli` de `products` **se conserva** y se mantiene **sincronizada** con la suma de las cantidades restantes de los lotes del producto. Se recalcula dentro de la misma transacción que cada operación de lotes. No es una fuente de verdad (los lotes lo son), pero se mantiene como caché para simplificar consultas.

#### LT-04 — Devolución a lotes

- **Dado** que una factura de salida se cancela, se elimina o se edita, **entonces** cada consumo se devuelve al **mismo lote** del que salió (la cantidad restante de ese lote aumenta).
- **Dado** que el lote ya no existe (**porque se eliminó la compra que lo originó**), **entonces** se crea un **lote de restitución** con el mismo costo unitario y se ubica como el más antiguo del producto. **El costo unitario sale del consumo original** (`consumption.unitCostCents`), no del lote (que ya no existe); ver §7.7.
- **Aclaración:** un lote con cantidad 0 cuya compra fue **cancelada** sigue existiendo y **se puede restaurar directamente**. "Ya no existe" solo aplica cuando el lote fue eliminado físicamente. Es el caso que cubre H-A1: si esa compra cancelada se elimina después, la restauración ya no es posible y hay que ajustar el inventario antes de borrar el lote (ver FE-05).
- **Dado** que el producto ya no existe (eliminado a mano o por la purga de P-08), **entonces** no hay nada que devolver: la operación continúa, registra una Reversión con cantidad 0 en el historial y el mensaje de resultado lo avisa.
- **Reutilización del lote de restitución:** si ya existe un lote de restitución con el **mismo costo unitario** y cantidad restante > 0, la devolución se hace sobre ese lote en lugar de crear uno nuevo.
- **Tope (H-A3):** una restauración que llevaría la suma de `remainingMilli` por encima de 999.999.999 se rechaza con `stockLimitExceeded`. En la práctica no debería ocurrir (la restauración devuelve unidades al mismo producto), pero se valida como defensa en profundidad.

### Módulo: Facturas de entrada

#### FE-01 — Crear factura de entrada (RF-044)

- **Dado** un usuario autenticado en "Nueva Factura de Entrada", **entonces** elige un **proveedor** con el buscador (CL-02, relevancia de 7.1; si no existe, PV-06), la **fecha** (por defecto hoy, no futura), el **estado inicial** (Por Pagar por defecto, o Pagada) y opcionalmente observaciones, método de pago (solo si el estado es Pagada) y un PDF adjunto (FE-07).
- **Productos:** se agregan con un buscador que busca por nombre, descripción y categoría, ordenado por relevancia (7.1). Solo aparecen productos **activos**; un descontinuado debe activarse primero. Cada línea pide **cantidad** (> 0, hasta 3 decimales) y **costo unitario** (> 0, hasta dos decimales); el sistema calcula el subtotal de la línea y el **total**. Un mismo producto no puede repetirse en la factura.
- **Producto nuevo sobre la marcha:** si el producto no existe, el formulario pide nombre, descripción, categoría, unidad y **% de ganancia**. La cantidad y el costo de la línea son su existencia inicial, **sin contarse dos veces**: el lote de la línea es su único lote y no se crea lote inicial aparte. El producto se crea **al guardar la factura**, en la misma transacción: si la factura falla, el producto no se crea. Si el nombre coincide con el de otro producto activo, la creación del producto (y por ende la factura) se rechaza con `duplicateField` "Ya existe un producto con ese nombre".
- **Dado** que se guarda, **entonces**, en una sola transacción: se crea la factura con su número `E-XXXXX` (contador de `invoiceCounters`, §7.4), se crea un lote por línea (LT-01), se recalcula el costo de cada producto (LT-03), se aumenta su cantidad y se registra un movimiento **Entrada** por línea (HP-01). Se guarda una copia de los datos del proveedor y de cada producto.
- **Dado** que el estado inicial es Pagada, **entonces** se registra la fecha de pago automáticamente.
- Se aplican los **límites de líneas y total de §7.2**: de 1 a 100 líneas por factura, y el total no puede superar $999.999.999,99.
- **Tope de stock (H-A3):** antes de insertar el lote de cada línea, se comprueba que la suma de `remainingMilli` del producto más la cantidad de la línea no supere 999.999.999 milésimas. Si se supera, la operación completa se rechaza con `stockLimitExceeded` (§8.2).

#### FE-02 — Ver todas las facturas de entrada (RF-045)

- **Dado** un usuario autenticado, **cuando** abre la sección, **entonces** ve las facturas con número (`E-XXXXX`), proveedor, fecha, total, estado, contador de productos y acciones, de la más reciente a la más antigua (a igualdad, por ID descendente; el encabezado de fecha alterna el orden), con el paginador global.
- **Búsqueda** por número o por proveedor. **Filtros** combinables: estado (Todos, Por Pagar, Pagada, Cancelada) y rango de fechas.

#### FE-03 — Detalle de factura de entrada (RF-046)

- Muestra: datos del proveedor (la copia guardada), productos (nombre, cantidad, costo unitario, subtotal), total, fecha, fecha de creación, responsable, estado, fecha y método de pago si aplican, observaciones y el archivo adjunto si existe.
- Las acciones disponibles dependen del estado (7.5).

#### FE-04 — Editar factura de entrada (RF-047)

- **Solo se pueden editar las facturas Por Pagar.** Una Pagada o Cancelada es de solo lectura; intentar editarla se rechaza con un mensaje que lo explica.
- Se puede modificar proveedor, fecha, observaciones, adjunto, líneas (agregar, quitar, cambiar cantidad o costo) con las mismas reglas de FE-01, más estas restricciones por **lotes con consumo** (cantidad inicial del lote − cantidad restante > 0):
  - No se puede **cambiar el costo** de una línea cuyo lote ya tiene consumo.
  - No se puede **reducir la cantidad** por debajo de lo ya consumido. Aumentarla sí se permite (el lote mantiene su posición de llegada).
  - No se puede **quitar** una línea cuyo lote ya tiene consumo, ni **cambiar su producto**.
  - Cada mensaje indica qué factura de salida usó esas unidades.
- Las líneas de productos **descontinuados** se conservan **sin ningún cambio**: no se pueden modificar la cantidad, el costo, ni el producto, ni quitarlas si el lote tiene consumo. **Excepción:** se permite **reducir** la cantidad hasta lo ya consumido. Aumentarla **no** se permite.
- Las líneas de productos **eliminados** se conservan sin efecto en inventario; se permite quitarlas (siempre) o reducirlas a 0, pero no modificarlas de otra forma.
- **Dado** que se guarda, **entonces**, en una transacción, se ajustan los lotes afectados, se recalcula el costo de cada producto y se registran los movimientos: **Entrada** por línea nueva, **Reversión** por línea quitada y **Ajuste** por línea con cantidad cambiada (HP-01). El **cambio de costo sin cambio de cantidad** no genera movimiento (H-A4): se registra solo el nuevo `unitPriceCents` si la línea se recalculó.
- **Tope de stock (H-A3):** agregar una línea o aumentar la cantidad de una existente se valida contra el tope de 999.999.999 milésimas del producto; si lo supera, la operación completa se rechaza con `stockLimitExceeded`.

#### FE-05 — Eliminar factura de entrada (RF-048)

- **Dado** una factura de entrada en cualquier estado, **cuando** elige eliminar, **entonces** el sistema pide confirmación. Si la factura no está Cancelada y alguna de sus compras tiene unidades que aún quedan en inventario, el mensaje indica, por producto, **cuántas unidades se perderán** y cuántas ya se consumieron en salidas.
- **Dado** que confirma y la factura no está Cancelada, **entonces**, en una transacción: se eliminan **solo las unidades que quedan** de cada lote (lo ya consumido en salidas no se toca), se recalcula el costo de cada producto, se registra una **Reversión** por línea y se elimina la factura con su adjunto.
- **Dado** que la factura está Cancelada **y todos sus lotes tienen `remainingMilli = 0`**, **entonces** se elimina sin tocar el inventario.
- **Dado** que la factura está Cancelada **y algún lote suyo tiene `remainingMilli > 0`** (por ejemplo, porque una venta que consumió ese lote se canceló después y LT-04 restauró las unidades sobre el lote), **entonces** la eliminación se comporta como la de una no cancelada: quita las unidades restantes, recalcula el costo del producto y registra una **Reversión** por línea (H-A1). Motivo: sin esta regla, el `ON DELETE CASCADE` de `lots.invoiceLineId` borraría el lote con stock restaurado sin dejar rastro y rompería el invariante `products.quantityMilli = SUM(remainingMilli)`. El mismo helper (`reverse_lots_of_invoice`, `plan-03 §3.2`) se usa en los tres caminos.
- **Dado** que un producto de una línea ya no existe, **entonces** se omite su ajuste y el mensaje de resultado lo avisa.

#### FE-06 — Cambiar estado de factura de entrada (RF-049)

- Transiciones permitidas (7.5): Por Pagar ↔ Pagada y, desde cualquiera de las dos, → Cancelada. **Cancelada es final.**
- **Dado** que cambia a **Pagada**, **entonces** se registra la fecha de pago y puede indicar el **método de pago** (Efectivo, Transferencia, Tarjeta o Zelle; opcional). **Dado** que vuelve a Por Pagar, **entonces** se borran fecha y método de pago. Estos cambios no tocan el inventario.
- **Dado** que cambia a **Cancelada**, **entonces** se confirma con el mismo aviso de FE-05 y se aplica el mismo efecto sobre los lotes y el historial (Reversión por línea), pero la factura se conserva con estado Cancelada. **Los lotes NO se eliminan:** solo se pone su `remainingMilli = 0`, para que la cancelación posterior de una venta pueda restaurar unidades directamente (LT-04) y para que el lote siga existiendo si más tarde se elimina la compra (H-A1).

#### FE-07 — Adjunto PDF de factura de entrada (RF-050)

- Se puede adjuntar **un** archivo PDF opcional por factura, de hasta **5 MB**, guardado en el **directorio de datos de la aplicación** (`app_data_dir/attachments/`), no en la base. La base guarda solo la **referencia** (nombre de archivo en disco y metadatos).
  - El backend valida que el contenido sea realmente un PDF (cabecera `%PDF-`), no solo la extensión, y que el nombre original no contenga rutas.
  - El archivo en disco se renombra a un **UUID**.
  - **Al eliminar la factura**, se borra también su PDF del sistema de archivos. Si el borrado del archivo falla, se registra en el log pero la operación continúa.
  - **Adjunto huérfano:** si el PDF se guarda en disco pero falla el guardado de la factura, el archivo queda huérfano. Se gestiona con un **barrido al arrancar la aplicación** (best-effort): se listan los archivos de `app_data_dir/attachments/`, se comparan con los `attachmentFileName` referenciados en `invoices` y se borran los no referenciados; si el listado o un borrado falla, se registra en `errors.log` y se continúa.
- Se puede reemplazar o quitar mientras la factura esté Por Pagar (FE-04).
  - **Reemplazo:** al reemplazar un PDF, el archivo anterior se **borra del disco** dentro de la misma transacción. Si el borrado falla, se registra en el log y la operación continúa.
- En el detalle, en cualquier estado, se muestra el nombre del archivo con un botón para guardarlo en el equipo (diálogo de "Guardar como"; la copia la hace `downloadInvoiceAttachment`, §5.5).
- Un archivo inválido o mayor de 5 MB se rechaza con un mensaje claro y la factura no se guarda.

#### FE-08 — Costo de los productos de una factura de entrada (sustituye a RF-051)

El recálculo del promedio ponderado al crear, editar o eliminar una factura **no** se hace con la fórmula incremental de RF-051. El costo se obtiene siempre de los lotes con existencia (LT-03), por lo que cancelar o eliminar una compra no distorsiona el costo.

### Módulo: Facturas de salida (venta, servicio, garantía e insumos)

#### FS-01 — Reglas comunes de las facturas de salida (RF-052 a RF-085)

Aplican a las facturas de **venta, servicio, gasto de garantía y gasto de insumos**; los apartados FS-02 a FS-05 solo describen lo que cada tipo añade. La interfaz mantiene una página por tipo.

- **Crear:** el usuario elige el **cliente** con el buscador (CL-02; si no existe, CL-06), la fecha (por defecto hoy, no futura), el estado inicial (**Por Cobrar** por defecto, o Cobrada; en garantía e insumos, **Pendiente** por defecto, o Completada), una descripción opcional y las líneas. Se numera `V-`, `S-`, `GG-` o `GI-` (7.4).
- **Líneas:** de 1 a 100, producto sin repetir, cantidad > 0 con hasta 3 decimales. El buscador de productos usa nombre, descripción y categoría con relevancia (7.1), solo muestra productos **activos** y muestra su stock; un producto con stock 0 se ve como "Agotado" y no se puede agregar. Un descontinuado debe activarse primero.
- **Estructura de líneas:** el formulario de venta se reescribe manteniendo la estructura `div.li-row.li-body` para las líneas, con las columnas No, Producto, Cantidad, Unidad, Precio Unitario, Subtotal y Acciones.
- **Stock:** al guardar, cada línea se descuenta por FIFO (LT-02). Si la cantidad pedida de algún producto supera su stock, no se guarda nada y el mensaje indica el producto y el stock disponible.
- **Dado** que se guarda, **entonces**, en una transacción: se crea la factura con la copia de los datos del cliente y de cada producto, se consumen los lotes, se recalcula el costo de cada producto (LT-03) y se registra un movimiento de salida por línea (HP-01).
- **Listar** (RF-053, 062, 069, 078): número, cliente, fecha, total (en garantía e insumos, "Costo total"), estado y acciones; garantía e insumos añaden la **factura asociada**. Orden, paginador, búsqueda por número o cliente y filtros de estado y fechas, como FE-02.
- **Detalle** (RF-054, 063, 070, 079): datos del cliente (copia guardada), líneas con nombre, cantidad, precio unitario y subtotal, total, descripción, fecha, fecha de creación, responsable, estado, fecha y método de pago si aplican y la factura asociada en garantía e insumos.
- **Editar** (RF-055, 064, 071, 080): **solo facturas Por Cobrar** (en garantía e insumos, **Pendiente**). Se puede cambiar cliente, fecha, descripción, factura asociada y líneas. Al guardar, en una transacción, se **devuelven** a sus lotes todos los consumos anteriores (LT-04), se vuelven a descontar las líneas resultantes (LT-02) y se registran los movimientos: **Salida** por línea nueva, **Reversión** por línea quitada y **Ajuste solo si la cantidad de la línea cambió**. Si el nuevo descuento no alcanza el stock, no se guarda nada.
  - En **ventas y servicios**, las líneas que ya existían **conservan el precio** con que se emitieron; solo las líneas nuevas toman el precio de venta vigente.
  - En **garantía e insumos**, el costo se recalcula con los lotes que resulten consumidos.
  - Las líneas de productos descontinuados solo se pueden conservar sin cambios; las de productos eliminados se conservan sin efecto en inventario.
  - **Cambio de costo por re-FIFO sin cambio de cantidad (H-A4):** no genera ningún movimiento. El `unitPriceCents` de la línea se actualiza al nuevo costo real (en garantía, insumos y pérdidas), pero el historial no refleja el cambio. Es un **límite aceptado** (`plan-03 §9`).
- **Eliminar** (RF-056, 065, 072, 081): en cualquier estado, con confirmación. Si no está Cancelada, devuelve los consumos a sus lotes (LT-04) y registra una **Reversión** por línea; si está Cancelada, no toca el inventario. La factura se elimina físicamente.
- **Cambiar estado** (RF-057, 066, 073, 082): transiciones de 7.5. Cobrada registra la fecha de cobro y, en venta y servicio, el método de pago opcional; volver a Por Cobrar los borra. **Cancelada** (final) devuelve los consumos a sus lotes y registra la Reversión, conservando la factura.
- Las facturas de garantía e insumos no generan ingreso; su estado indica si el proceso está pendiente, completado o cancelado.

#### FS-02 — Factura de venta (RF-052 a RF-060)

- Cada línea guarda su **`unitPriceCents`** (precio de venta vigente en el momento de la creación o edición). El subtotal y el total se calculan con ese valor guardado; no se recalculan al leer.
- Líneas con el **precio de venta vigente** del producto (`spec-02` P-11) copiado a la línea; sin descuento ni precio manual. Subtotal = cantidad × precio (7.2); total = suma de subtotales.
- El buscador de clientes (RF-058) y de productos (RF-059) siguen 7.1.

#### FS-03 — Factura de servicio (RF-061 a RF-067)

- Las líneas son los **insumos** usados en el servicio, con la misma mecánica de la venta: precio de venta vigente, descuento FIFO. El total del servicio es la suma de cantidad × precio de venta de los insumos.

#### FS-04 — Factura de gasto de garantía (RF-068 a RF-076)

- Las facturas de garantía e insumos **no tienen método de pago** (no generan ingreso, no se cobran al cliente). Sus estados son **Pendiente / Completada / Cancelada**.
- Además del cliente, se elige la **factura original asociada**: un selector de las facturas de **venta y servicio del cliente seleccionado que no estén Canceladas**. Al cambiar de cliente, la selección se limpia. Es obligatoria.
- **Contrato de interfaz:** la factura original asociada se elige con un **`<select>`** (`#factura-garantia-original`) que se rellena al elegir cliente con las facturas de venta y servicio no canceladas de ese cliente. Los `dropzone` de subida de PDF del mockup (`#factura-garantia-dropzone`) **se eliminan**.
- En el detalle, `#factura-garantia-ver-original` muestra `{numero} · {tipo}` (por ejemplo `V-00005 · Venta`).
- **Estados:** Pendiente / Completada / Cancelada. Pendiente indica que la garantía aún no se ha procesado; Completada, que ya se repuso el producto o se registró el insumo; Cancelada, que la garantía se anuló. Desde Pendiente se puede ir a Completada o Cancelada; Completada puede ir a Cancelada; Cancelada es final (transiciones de §7.5 aplicables a los estados Pendiente/Completada/Cancelada).
- Las líneas muestran el **costo real** de los lotes consumidos, no un precio de venta; el total es el **costo total** de la garantía (sustituye al costo promedio de RF-075).
- **Dado** que la factura original se elimina o cancela después, **entonces** la garantía conserva una copia de su número y tipo (7.7) y no se modifica.

#### FS-05 — Factura de gasto de insumos de instalación (RF-077 a RF-085)

- Las facturas de garantía e insumos **no tienen método de pago**. Sus estados son **Pendiente / Completada / Cancelada**.
- Además del cliente, se elige la **factura de servicio asociada**: un selector de las facturas de servicio del cliente que no estén Canceladas. Es obligatoria y se trata igual que la factura original de FS-04.
- **Contrato de interfaz:** igual que FS-04, con `#factura-insumo-servicio` y `#factura-insumo-ver-asociada`. Los `dropzone` del mockup (`#factura-insumo-dropzone`) **se eliminan**.
- **Estados:** Pendiente / Completada / Cancelada. Pendiente indica que el gasto de insumos aún no se ha procesado; Completada, que ya se registró el insumo; Cancelada, que el gasto se anuló. Desde Pendiente se puede ir a Completada o Cancelada; Completada puede ir a Cancelada; Cancelada es final (transiciones de §7.5 aplicables a los estados Pendiente/Completada/Cancelada).
- Las líneas y el total siguen la mecánica de FS-04: costo real de los lotes consumidos (sustituye al costo promedio de RF-084).

### Módulo: Facturas de pérdida

#### FP-01 — Facturas de pérdida (RF-086 a RF-090)

- **Crear:** se elige **un producto activo, o descontinuado con stock** (buscador de 7.1; S-22), la **cantidad** perdida (> 0, hasta 3 decimales, no mayor que el stock), la **fecha** (por defecto hoy, no futura) y una descripción opcional del motivo. Se numera `P-`. El **costo** no se escribe: lo calcula el sistema con el consumo FIFO (LT-02). No tiene estados ni cliente.
- **Costo de pérdida en la interfaz:** el campo `#factura-perdida-costo` del mockup pasa a ser **de solo lectura**. Su valor lo calcula el backend con el consumo FIFO (LT-02) al crear la pérdida. El formulario de creación solo pide producto, cantidad, fecha y descripción. **Vista previa:** mientras el usuario arma la pérdida (antes de guardar), `#factura-perdida-costo` se muestra vacío o con "—". El valor se calcula y se muestra después de guardar.
- **Dado** que se guarda, **entonces**, en una transacción: se crea la factura con copia del producto, se consumen los lotes, se recalcula el costo del producto y se registra un movimiento **Pérdida** (HP-01).
- **Listar** (RF-087): número, producto, cantidad, costo total, fecha y descripción, con búsqueda por producto o número, filtro de fechas, orden y paginador como FE-02. **Detalle** (RF-088): producto (copia guardada), cantidad, costo unitario, costo total, fecha, fecha de creación, responsable y descripción.
- **No se edita.** **Eliminar** (RF-089): con confirmación, devuelve los consumos a sus lotes (LT-04), registra una Reversión y elimina la factura.

### Módulo: Lista general de facturas

#### FG-01 — Lista general (mockup, sin RF de origen)

- **Dado** un usuario autenticado, **cuando** abre "Facturas General", **entonces** ve, en una sola lista de **solo lectura**, las facturas de todos los tipos con número, tipo, cliente o proveedor (copia guardada), fecha, total, estado y acciones. Las pérdidas muestran "—" como cliente y estado, y su total es su costo.
- **Búsqueda** por número, cliente o proveedor. **Filtros** combinables: tipo (Entrada, Venta, Servicio, Garantía, Insumo, Pérdida), estado (Todos, Pendientes —Por Pagar, Por Cobrar o Pendiente—, Completadas —Pagada, Cobrada o Completada— y Canceladas) y rango de fechas. Un filtro de estado distinto de "Todos" excluye las pérdidas.
- **Contrato de interfaz:** el filtro de estado es un `<select>` (`#factura-general-estado`) con cuatro opciones: `""` (Todos), `"pendientes"`, `"completadas"`, `"canceladas"`. El backend traduce:
  - **Pendientes:** Por Pagar (entrada), Por Cobrar (venta/servicio), Pendiente (garantía/insumo).
  - **Completadas:** Pagada (entrada), Cobrada (venta/servicio), Completada (garantía/insumo).
  - **Canceladas:** Cancelada (todos los tipos).
- Con cualquier opción distinta de "Todos", las pérdidas se excluyen.
- **Contrato del comando:** la lista general se obtiene con `listInvoices(filters)` **sin filtro de `type`**. No existe un comando aparte: `listInvoices` acepta `type` opcional y, cuando se omite, devuelve facturas de todos los tipos. Devuelve `InvoiceListItem[]` con `{number, type, contactName, date, totalCents, status, associatedInvoice}`. `type` viaja como el código del prefijo (`'E'`, `'V'`, `'S'`, `'GG'`, `'GI'`, `'P'`); la traducción a texto visible es del frontend. Los filtros (`search`, `type`, `status`) son opcionales.
- Orden y paginador como FE-02. "Ver" abre el detalle del tipo correspondiente; **no se crea, edita ni elimina desde esta lista**.

### Módulo: Historial de productos

#### HP-01 — Registrar movimientos (RF-091)

- **Dado** cualquier operación que cambie la cantidad de un producto, **entonces** se registra automáticamente, en la misma transacción que la operación, un movimiento por producto afectado con: fecha y hora, tipo, cantidad **con signo** (+ entra, − sale), stock resultante, precio unitario, precio total, factura asociada (número y tipo) y usuario que ejecutó la operación.
- **Tipos de movimiento:** los de RF-091 —**Entrada, Salida Venta, Salida Servicio, Salida Garantía, Salida Insumos, Pérdida**— más tres que añade esta spec: **Inicial** (existencia inicial al crear el producto, PEN-09), **Ajuste** (edición de una factura que **cambia la cantidad** de una línea ya existente) y **Reversión** (anulación de una línea por cancelación, eliminación o edición). Los códigos internos son `initial`, `entry`, `saleOut`, `serviceOut`, `warrantyOut`, `supplyOut`, `loss`, `adjustment`, `reversal`.
- **Precio unitario y total:** entradas e Inicial, el costo del lote; ventas y servicios, el precio de venta; **garantía, insumos y pérdidas, el costo de la línea dividido entre la cantidad de la línea**, con la **misma función** `line_unit_cost_cents` que produce el `unitPriceCents` de la línea (§5.3).
- **Cuándo se registra:** crear producto (Inicial); crear factura (Entrada o Salida por línea); editar factura (Entrada/Salida por línea nueva, Reversión por línea quitada, **Ajuste solo si la cantidad de la línea cambió**); cancelar o eliminar una factura no Cancelada (Reversión por línea, incluso con cantidad 0); **eliminar una factura Cancelada cuyos lotes tengan `remainingMilli > 0` (Reversión por línea, H-A1)**. **No** se registra al cambiar entre Por Pagar/Pagada o Por Cobrar/Cobrada, ni al eliminar una factura Cancelada cuyos lotes estén todos a 0.
- **Aclaración sobre cantidad 0:** **Reversión con cantidad 0:** se registra siempre que una factura no cancelada se anula (cancelar, eliminar) o una línea se quita al editar. El movimiento lleva: cantidad = 0, stock resultante = stock actual del producto, **precio unitario = `consumption.unitCostCents` del consumo original**, precio total = 0.
- **Cambio de costo sin cambio de cantidad (H-A4):** si al editar una salida una línea no cambia de cantidad pero su costo por re-FIFO sí cambia (porque entre la emisión y la edición se modificó o eliminó alguna compra relacionada), **no se registra ningún movimiento de Ajuste**. El `unitPriceCents` de la línea sí se actualiza con el nuevo costo real (en garantía, insumos y pérdidas), pero el historial no refleja el cambio. Es un **límite aceptado** (`plan-03 §9`).
- El movimiento es **inmutable**: ningún comando lo edita ni lo borra. Se conserva aunque el producto, la factura o el usuario se eliminen (copias en 7.7). **No se purga por antigüedad.** Es un límite aceptado: la tabla de movimientos crece sin límite. Los **filtros por rango de fechas** de HP-02 permiten consultar cualquier período sin que el tamaño afecte al uso normal.

#### HP-02 — Ver y filtrar el historial de un producto (RF-092, RF-093, RF-094)

- La pestaña **Historial** de Productos y el botón **"Ver Historial"** del detalle del producto quedan visibles.
- **Dado** un usuario autenticado, **cuando** elige un producto, **entonces** ve sus movimientos, del más reciente al más antiguo (el encabezado de fecha alterna el orden), con Fecha, Tipo, Cantidad (con signo), Stock Resultante, Precio Unitario, Precio Total, Factura y Usuario, con el paginador global. El número de factura abre el detalle de la factura si todavía existe.
- **Selector de producto:** lista los productos existentes y, en un grupo aparte, los **productos eliminados** que tienen movimientos, identificados por su nombre guardado y su ID original (por ejemplo "Cable UTP (eliminado, #00012)").
- **Contrato del comando:** `listHistoryProducts()` devuelve un arreglo plano de `{id, name, isDeleted}` con lo necesario para armar los dos `<optgroup>`: todos los productos existentes (`isDeleted: false`) y, aparte, los productos eliminados **que tienen movimientos** (`isDeleted: true`). Los eliminados sin movimientos no se incluyen. Orden alfabético español dentro de cada grupo.
- **Contrato de interfaz:** el selector `#historial-producto-select` usa dos `<optgroup>`:
  - **"Productos activos":** los productos existentes.
  - **"Productos eliminados":** los productos eliminados con movimientos, identificados por `Nombre (eliminado, #ID)`.
- **Filtros** combinables: rango de fechas (día local, ambos extremos incluidos) y **uno o varios** tipos de movimiento. **Semántica del filtro multi-selección:** OR. Se devuelven los movimientos cuyo tipo esté en **cualquiera** de las opciones seleccionadas. Si no hay opciones seleccionadas, se devuelven todos.
- **Contrato de interfaz:** el filtro de tipos es un **`<select multiple>`** (`#historial-producto-tipo`) con tres `<optgroup>`:
  - **Entradas:** `initial` (Inicial), `entry` (Entrada).
  - **Salidas:** `saleOut` (Salida Venta), `serviceOut` (Salida Servicio), `warrantyOut` (Salida Garantía), `supplyOut` (Salida Insumos), `loss` (Pérdida).
  - **Ajustes:** `adjustment` (Ajuste), `reversal` (Reversión).
- Por defecto, ninguna opción seleccionada = todos los tipos.

#### HP-03 — Exportar el historial a Excel y a PDF (RF-095, RF-096)

- **Dado** el historial de un producto con o sin filtros, **cuando** el usuario toca "Exportar a Excel" o "Exportar a PDF", **entonces** el sistema abre un diálogo de "Guardar como" y genera un archivo `.xlsx` o `.pdf` con **todos** los movimientos que cumplen los filtros activos (no solo la página visible), en el orden de la pantalla.
- **Contrato de interfaz:** los botones de exportación son **`#historial-exportar-excel`** y **`#historial-exportar-pdf`**. Se colocan al final del `.historial-toolbar` de `#producto-historial`, alineados a la derecha. Están **deshabilitados** si no hay movimientos para exportar; el motivo se comunica con `title="No hay movimientos para exportar"` y un `aria-live="polite"` junto a la toolbar.
- El archivo incluye un encabezado (producto, rango de fechas y tipos filtrados, fecha de generación) y las mismas columnas de HP-02. Las fechas van en hora local con formato `dd/mm/aaaa` y los importes en USD con dos decimales.
- Si no hay movimientos que exportar, la acción está deshabilitada y un mensaje lo explica. Si el usuario cancela el diálogo, no ocurre nada. Si falla la escritura del archivo, se muestra un mensaje claro y el error se registra (8.1).
- **Generación en backend, no en frontend:** las dependencias (`rust_xlsxwriter` para `.xlsx`, `printpdf` para `.pdf`) se justifican en `plan-03.md`. El diálogo de "Guardar como" usa `tauri-plugin-dialog` (ya presente).
- **RF-097 (importar desde Excel) no se implementa**: lo sustituye el respaldo completo de PEN-15.

#### HP-04 — Facturas asociadas de un producto (PEN-09)

- La sección "Facturas Asociadas" del formulario de edición y del detalle del producto muestra las facturas, de cualquier tipo, que incluyen ese producto, de la más reciente a la más antigua y hasta 50: número, tipo, fecha, estado y cantidad de la línea. Cada fila abre el detalle de la factura.

### Módulo: Cambios a `spec-02`

#### SP-01 — Crear producto con lote inicial (modifica P-01)

- Crear un producto sigue pidiendo cantidad inicial (> 0) y costo inicial (> 0) con las reglas de `spec-02`. Ahora, en la misma transacción, crea el **lote inicial** (LT-01) y registra un movimiento **Inicial** (HP-01) con el usuario que lo creó. Los productos creados desde una factura de entrada (FE-01) no tienen lote inicial: su existencia es el lote de la línea.
- **Tope (H-A3):** la cantidad inicial no puede superar el tope de stock del producto; si lo supera, se rechaza con `stockLimitExceeded`.
- **Responsable:** la factura guarda el usuario que la **creó** (con copia de su nombre). Al editar una factura, el **responsable no cambia**. Si se quiere saber quién ejecutó una operación concreta, el historial de productos registra el usuario de cada movimiento (HP-01). La columna `updatedAt` registra cuándo se editó (sin quién).
- **Responsable no editable:** el responsable **no se puede cambiar** explícitamente desde ningún comando ni formulario. Se fija al crear la factura con el usuario de la sesión activa y se conserva inmutable.

#### SP-02 — Cantidad y costo derivados (modifica P-10, P-11 y varias frases de `spec-02`)

- La **cantidad** y el **costo** de un producto ya no se fijan al crearlo: cambian por facturas. El comando de edición de producto (P-03) sigue sin aceptar cantidad, costo ni precio de venta.
- El costo se deriva de los lotes (LT-03). La función de promedio ponderado de P-10 se **sustituye** por el cálculo sobre lotes con existencia. **La función `pricing.rs::weighted_average_cost_cents` se elimina**: ya no tiene consumidor y mantenerla sin uso violaría `Constitution.md` §2.II. `selling_price_cents` se conserva.
- Al eliminar un producto (P-14) o purgarlo (P-08), sus lotes se eliminan con él; las facturas y movimientos conservan sus copias (7.7).
- Desde esta spec, los productos **agotados** existen en el uso real: la pestaña "Agotados y Descontinuados" (P-13), el filtro "Agotados", "Stock bajo" y la tarjeta de DASH-01 muestran datos reales sin cambiar sus reglas.

#### SP-03 — "Valor Total" e "Ingresos" (PEN-10, parcial)

- **Valor Total** del producto (columna del listado de productos y campo del detalle): suma, sobre sus lotes con existencia, de cantidad restante × costo del lote, redondeada al centavo (mitad hacia arriba) una sola vez por producto.
- **Ingresos** de una categoría (columna del listado y del detalle de categoría): suma de los subtotales de las líneas de facturas de **venta y servicio no canceladas** cuya **categoría guardada en la línea** (la que el producto tenía en el momento de la venta) corresponde a esa categoría. Las líneas de productos sin categoría en el momento de la venta no cuentan.
- **Consecuencia:** cada línea de venta y servicio guarda una copia del `categoryId` (y opcionalmente del nombre de categoría) del producto en el momento de la venta. Si un producto cambia de categoría, las ventas anteriores siguen contando para la categoría que tenían al momento de la venta. El indicador es histórico y estable.
- Los KPIs del dashboard, "Mejor Vendidos" y "Lento Movimiento" **siguen en estado vacío** hasta la spec de Dashboard (PEN-16).

#### SP-04 — Efectos sobre otras reglas de `spec-02`

- Una factura de entrada no reactiva ni modifica un producto descontinuado. El resto de P-02 a P-09 y P-12 a P-14 no cambia.

## 7. Reglas de negocio transversales

### 7.1 Texto y búsqueda

- **Normalización, unicidad y orden alfabético** de `spec-02` §7.1 (minúsculas, plegado de las 6 vocales, `ñ` distinta, espacios colapsados, sin NFC/NFD) y **reglas de texto** de §7.3 (recorte, largos en caracteres, rechazo de caracteres de control y de formato, saltos de línea solo en descripciones y observaciones). Se reutilizan las mismas funciones.
- **Largos (S-19):** nombre(s) y apellidos 2–50; empresa y cargo hasta 100; correo hasta 100; dirección e información de factura hasta 200; observaciones y descripciones de factura hasta 500; nombre del PDF hasta 100.
- **Correo (patrón exacto):** `^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$`, en ASCII, hasta 100 caracteres. Es intencionalmente simple; no se valida contra RFC 5322. La validación es autoritativa en Rust (`validators.rs`); el frontend puede replicarla como ayuda de UX.
- **Búsqueda "a medida que escribe"** (contactos, productos en facturas y facturas): coincidencia por subcadena sobre el texto normalizado; `%` y `_` son texto literal; el teléfono se compara sin espacios ni `+`.
  - **Contactos:** se busca en nombre, apellidos, **nombre completo** (nombre + apellidos), CI, teléfono y empresa.
  - **Productos:** se busca en nombre, descripción y categoría.
  - **Facturas:** por número y por el nombre completo, empresa, CI o teléfono de la copia del contacto guardada en la factura.
  - **Relevancia:** 1.º el texto coincide **exactamente** con un campo; 2.º un campo **empieza** por el texto; 3.º un campo lo **contiene**. Dentro de un mismo nivel, en productos manda el campo (nombre, luego categoría, luego descripción); a igualdad, alfabético y por ID. Los buscadores desplegables muestran hasta 20 resultados (S-20).
  - **Aclaraciones (confirmadas):**
    - **Exacto:** el texto del usuario se compara con el **campo completo** (normalizado). En contactos, "nombre completo" = nombre + apellidos separados por espacio.
    - **Prefijo:** "empieza por" se evalúa desde el inicio de **cualquier palabra** del campo.
    - **Dentro del nivel 1 (exacto) y nivel 2 (prefijo):** si varios contactos coinciden exactamente por distintos campos, se ordenan igual que en el resto.
    - **Teléfono:** tanto el texto del usuario como el campo se normalizan quitando espacios y el símbolo `+` antes de comparar.
- Se ofrece "Crear nuevo…" (cliente o proveedor) solo si ningún contacto coincide con lo escrito.

### 7.2 Números y dinero

- Todo importe se guarda en **centavos de USD** (entero) y toda cantidad en **milésimas** (entero), con aritmética entera exacta y redondeo mitad hacia arriba, igual que `spec-02` §7.4. Formato de entrada, separador decimal (coma o punto), ambigüedad del punto, ceros a la izquierda y orden de mensajes: los de `spec-02` §7.4.
- **Rangos:** cantidad de línea > 0 y hasta 999.999,999 (3 decimales); costo unitario de compra > 0 y hasta 999.999,99 (2 decimales); precio de venta de línea: el vigente, que ya está acotado por `spec-02`.
- **Subtotal de línea** = cantidad × precio, redondeado al centavo (mitad hacia arriba) una sola vez por línea. **Total** = suma de subtotales. Un total mayor que **$999.999.999,99**, o una línea cuyo subtotal lo supere, se rechaza con un mensaje claro (S-17).
- Entre 1 y **100 líneas** por factura; un mismo producto no puede repetirse.
- **Vista previa del total al armar la factura:** el frontend compone el total preliminar sumando las entradas del usuario (cantidad × precio/costo de cada línea). El backend recalcula el total autoritativo al guardar con aritmética entera exacta. **No existe un comando `previewInvoiceTotals`:** la vista previa es una comodidad del frontend, no una consulta al backend.
- **Tope de stock (H-A3):** la suma de cantidades restantes de los lotes de un producto no puede superar 999.999,999 unidades. Ver §7.6.

### 7.3 Fechas

- **Fecha de la factura:** se guarda como `YYYY-MM-DD`, la elige el usuario (por defecto, hoy en hora local), debe ser una fecha de calendario válida, **no futura** y no anterior al 2000-01-01 (S-18). Es editable mientras la factura se pueda editar.
- **Fecha de creación, de pago o cobro y de los movimientos:** fecha y hora UTC en ISO 8601, con el formato fijo y el `CHECK` de `spec-02` §5.4/§2, y reloj del sistema operativo.
- **Visualización:** toda fecha en hora local con formato `dd/mm/aaaa`. Los filtros por rango de fechas se aplican sobre el **día local** y son inclusivos en ambos extremos. **Rango de fechas invertido:** si el usuario introduce un rango con `desde > hasta`, el backend rechaza la consulta con `validationFailed` y la interfaz muestra "La fecha inicial no puede ser posterior a la final". **Rango con una sola fecha:** si solo hay "desde", se filtran los movimientos desde esa fecha en adelante. Si solo hay "hasta", se filtran desde el principio hasta esa fecha. Si no hay ninguna, no se aplica filtro.

### 7.4 Numeración

- Cada tipo tiene su contador independiente: `E-`, `V-`, `S-`, `GG-`, `GI-`, `P-` seguido de al menos 5 dígitos con ceros a la izquierda (`E-00001`). **El contador vive en la tabla `invoiceCounters(type, nextNumber)`** y se incrementa dentro de la misma transacción que crea la factura. **Nunca se decrementa ni se reinicia**. Pasados 99999, el contador muestra los dígitos necesarios a partir de 6.
- **Descartado explícitamente:** `MAX(number) + 1` y `AUTOINCREMENT` sobre `id`.

### 7.5 Estados y transiciones

| Desde | Hacia | Efecto sobre el inventario | Otros efectos |
|---|---|---|---|
| Por Pagar / Por Cobrar | Pagada / Cobrada | Ninguno | Registra fecha de pago o cobro; método de pago opcional (solo entrada, venta y servicio) |
| Pagada / Cobrada | Por Pagar / Por Cobrar | Ninguno | Borra fecha y método |
| Por Pagar / Por Cobrar | Cancelada | Entrada: quita lo que queda de sus lotes. Salidas: devuelve los consumos a sus lotes | Reversión por línea en el historial |
| Pagada / Cobrada | Cancelada | Igual que la fila anterior | Igual |
| **Pendiente** (garantía/insumos) | **Completada** | Ninguno | Marca la garantía/insumo como procesado |
| **Pendiente / Completada** (garantía/insumos) | **Cancelada** | Devuelve los consumos a sus lotes | Reversión por línea en el historial |
| Cancelada | — | **Final**: no se reactiva ni se edita | — |

- Pedir el estado en el que ya está la factura se rechaza (`invalidState`, 8.2).
- **Editar:** solo en Por Pagar / Por Cobrar (Pendiente en garantía e insumos). **Eliminar:** en cualquier estado. Las pérdidas no tienen estado.
- Cada cambio de estado es una operación atómica (7.6).

**Opciones del `<select>` de estado por tipo de factura (etiquetas visibles):**

| Tipo | Opciones del `<select>` | Notas |
|---|---|---|
| Entrada (`E-`) | Por Pagar · Pagada · Cancelada | Sin "Vencida" |
| Venta (`V-`) | Por Cobrar · Cobrada · Cancelada | Sin "Pagada", sin "Vencida" |
| Servicio (`S-`) | Por Cobrar · Cobrada · Cancelada | Sin "Pagada", sin "Vencida" |
| Garantía (`GG-`) | Pendiente · Completada · Cancelada | Sin "Pagada", sin "Vencida" |
| Insumo (`GI-`) | Pendiente · Completada · Cancelada | Sin "Pagada", sin "Vencida" |
| Pérdida (`P-`) | — (sin estados) | No tiene `<select>` de estado |

Los `value` de cada `<option>` son los códigos internos (`pending`, `completed`, `cancelled`), no los textos visibles. En todos los formularios, la opción inicial del `<select>` es el estado por defecto de §5.3.

**Equivalencia de estados agrupados (usada por los filtros de FG-01):**

| Grupo | Estados incluidos |
|---|---|
| Pendientes | Por Pagar (entrada), Por Cobrar (venta/servicio), Pendiente (garantía/insumo) |
| Completadas | Pagada (entrada), Cobrada (venta/servicio), Completada (garantía/insumo) |
| Canceladas | Cancelada (todos los tipos) |

**Método de pago:** en el formulario, el `<select>` de método de pago **solo está visible y editable cuando el estado es Pagada/Cobrada**. Si el usuario crea la factura en Por Pagar/Por Cobrar, el `<select>` se oculta y no se envía. Al cambiar a Pagada/Cobrada, se muestra y puede elegirse (opcional). Al volver a Por Pagar/Por Cobrar, se oculta y se limpia. Las opciones son **Efectivo, Transferencia, Tarjeta, Zelle**. Se elimina "Efectivo CUP".

### 7.6 Atomicidad, integridad e inexistentes

- Cada operación ocurre en **una sola transacción** que incluye la factura, los lotes, los consumos, el costo del producto y los movimientos del historial. Si algo falla, no queda ningún cambio parcial.
- El esquema impide un lote con cantidad restante negativa o mayor que su cantidad inicial (`CHECK`), de modo que el stock nunca queda negativo aunque falle una validación del código (`Constitution.md` §4).
- **Cambios de estado entre la carga del formulario y el guardado:** dado el modelo de una sola sesión activa y una única conexión SQLite, los cambios de estado concurrentes **no ocurren durante el uso normal**. Sin embargo, la **purga** (P-08) puede eliminar productos descontinuados vencidos entre la carga y el guardado. En ese caso, los comandos de factura lo tratan como **inexistente** y responden `notFound`. Si el contacto o la factura asociada fueron eliminados, la factura responde `notFound` con el mensaje correspondiente.
- **Productos eliminados o purgados:** al eliminar, cancelar o editar una factura cuyo producto ya no existe, se omite el ajuste de inventario de esa línea y el mensaje de resultado lo avisa. Las líneas conservan su copia.
- **Productos descontinuados:** no pueden agregarse a facturas nuevas ni a líneas nuevas de entrada, venta, servicio, garantía o insumos (S-22). **Excepción:** sí se puede registrar la pérdida de un descontinuado con stock (FP-01). Las líneas ya existentes se conservan sin cambios al editar.
- **Eliminar una compra Cancelada con lotes restaurados (H-A1):** aunque el estado Cancelada exime a la factura de tocar el inventario, si algún lote suyo tiene `remainingMilli > 0` (porque una venta posterior se canceló y LT-04 restauró unidades sobre él), la eliminación **sí** ajusta el inventario: quita las unidades restantes, recalcula el costo del producto y registra una Reversión por línea. Es la única excepción a "Cancelada se elimina sin tocar el inventario"; su razón es evitar una pérdida silenciosa de stock por el `ON DELETE CASCADE`.
- **Tope de stock por producto (H-A3):** la suma de `remainingMilli` de todos los lotes de un producto no puede superar **999.999.999 milésimas** (999.999,999 unidades). El tope coincide con el máximo de `stockAfterMilli` y de cada lote, así que no se necesitan `CHECK` nuevos: se valida en el código antes de cada operación que pueda aumentar el stock (crear producto, agregar una línea a una compra, aumentar la cantidad de una línea existente, restaurar consumos). Si el tope se supera, el comando responde `stockLimitExceeded` con "El stock total de «X» no puede superar 999.999,999" y no aplica ningún cambio.
- Todo comando de esta spec que consulte o modifique un producto descontinuado ejecuta primero la **purga de P-08** (`spec-02`), con la misma semántica best-effort.

  **Comandos afectados (lista consolidada):**

  - **Facturas:** `listInvoices`, `getInvoice`, `createEntryInvoice`, `createSaleInvoice`, `createServiceInvoice`, `createGuaranteeInvoice`, `createInstallationInvoice`, `createLossInvoice`, `updateEntryInvoice`, `updateSaleInvoice`, `updateServiceInvoice`, `updateGuaranteeInvoice`, `updateInstallationInvoice`, `deleteInvoice`, `changeInvoiceStatus`.
  - **Historial:** `getProductHistory`, `exportHistoryExcel`, `exportHistoryPdf`.
  - **Facturas asociadas:** `getProductAssociatedInvoices`.
  - **Indicadores de contactos:** `getClientDetail`, `getSupplierDetail`.
  - **Búsqueda de productos para facturas:** `searchProductsForInvoice`.

  **No purgan:** `saveInvoiceAttachment`, `downloadInvoiceAttachment`, `listProducts`, `getDashboardSummary`, `previewSellingPrice`, `listHistoryProducts`.

  **Decisión de diseño sobre `listHistoryProducts` (C-7):** `listHistoryProducts` **no purga**. Su propósito es listar los productos que tienen historial (activos y eliminados con movimientos). Si un producto vencido aún no ha sido purgado, aparece en el grupo de activos con su nombre; cuando la purga se ejecute (por cualquier comando de la lista de arriba), el producto pasará al grupo de eliminados y seguirá apareciendo porque **sus movimientos se conservan** (`productMovements` no se purga). Purgar en este comando no aporta nada al usuario (el resultado es equivalente) y añade trabajo innecesario. **No es un hueco**, es una decisión de diseño. Se documenta aquí para que no se confunda con una omisión.

### 7.7 Copias y claves foráneas

- **Contacto:** la factura guarda el vínculo (`clientId`/`supplierId`) y la copia (nombre completo, empresa, CI, teléfono, **correo**). Al eliminar el contacto, el vínculo queda vacío y la copia permanece. Editar un contacto no modifica sus facturas.
- **Producto:** la línea guarda el vínculo y la copia (nombre, unidad, precios y cantidades). Al eliminar o purgar el producto, el vínculo queda vacío y la copia permanece. Los lotes del producto se eliminan con él (`ON DELETE CASCADE`). **Producto eliminado con consumos activos:** si un producto se elimina mientras tiene lotes consumidos por facturas de salida **activas**, al cancelar o editar esas facturas **la devolución no se puede aplicar** (el producto y sus lotes ya no existen). La operación continúa, registra una **Reversión** en el historial con cantidad 0 y el mensaje de resultado lo avisa.
- **Categoría de la línea (ventas y servicios):** `categoryId` y `categoryName` son **snapshots sin clave foránea**. Los "Ingresos" (SP-03) dependen de esta inmutabilidad.
- **Lote y consumo:** el consumo guarda el vínculo al lote (`ON DELETE SET NULL`, queda vacío si el lote se elimina) y **el `unitCostCents` del momento del consumo**. El costo de una Reversión sale siempre de `consumption.unitCostCents`, **no de `lot.unitCostCents`**.
- **Factura asociada** (garantía e insumos): vínculo con copia del número y tipo; si la original se elimina, el vínculo queda vacío.
- **Usuario:** la factura y el movimiento guardan el vínculo y la copia del nombre del usuario. Si el usuario se elimina, el vínculo queda vacío y el nombre se conserva.
- **Movimientos:** guardan el vínculo al producto y a la factura (vacíos si se eliminan), la copia del nombre y el ID original del producto, y el número y tipo de la factura.

### 7.8 Datos iniciales

La base de esta feature **empieza vacía** (sin clientes, proveedores, facturas, lotes ni movimientos semilla). Toda pantalla tiene un estado vacío claro en español ("Aún no hay clientes", "Aún no hay facturas"…). Nunca se rellena con datos de ejemplo (`Constitution.md` §2.I).

## 8. Manejo de errores

### 8.1 Mensajes y registro

Rige `spec-01` §8 y `spec-02` §8.1: mensajes en español y sin detalles técnicos; **específicos** en validación y reglas de negocio, **genéricos** ante errores internos. Ningún registro contiene datos personales, importes, nombres ni valores ingresados.

**Política de registro (H-A2):**
- `validationFailed` (rechazos de formato o de rango de campos: email, longitud, número mal escrito, fecha imposible, etc.) e `invalidCredentials` **no** se registran: son entradas del usuario que se repiten sin límite y no indican un fallo del sistema (`Constitution.md` §2.VI, excepción acotada).
- Los códigos nuevos que **sí** llevan estado de negocio (`stockInsuficiente`, `loteEnUso`, `attachmentInvalid`, `stockLimitExceeded`) **se registran con un límite de frecuencia**: como máximo un rechazo por mismo código de error y mismo `userId` cada **60 segundos**; los repetidos dentro de esa ventana se descartan sin contarse. El throttle vive en memoria del proceso (un `HashMap<(code, userId), lastLoggedAt>` bajo `Mutex` en `errors.rs`): se pierde al reiniciar la app, lo cual es aceptable porque el log es para diagnóstico, no para auditoría exacta. Motivo: estos códigos pueden indicar un estado real del inventario y merece la pena verlos sin llenar el log cuando un usuario reintenta la misma operación en bucle.
- El resto de los códigos (`duplicateField`, `notFound`, `invalidState`, `exportFailed`, `internal`) se registran siempre, sin throttle, porque son rechazos únicos o errores del sistema.
- **No hay desviación de `Constitution.md` v1.2.0 §2.VI que documentar:** la excepción de los rechazos esperados se mantiene para `validationFailed` e `invalidCredentials`, y los códigos nuevos no son rechazos "que se repiten sin límite" — precisamente porque el throttle los limita y quedan fuera de la excepción.

**Ampliación de `AppError::should_log()`:** `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded` devuelven `true` (aunque el throttle decida si la escritura se materializa). `validationFailed` e `invalidCredentials` siguen devolviendo `false`. Los códigos existentes de `spec-01` y `spec-02` no cambian de comportamiento.

### 8.2 Catálogo de mensajes (el `plan-03.md` fija códigos y textos exactos)

- **No existe:** "El cliente ya no existe", "El proveedor ya no existe", "La factura ya no existe", "El producto ya no existe".
- **Duplicado:** "Ya existe un cliente con ese carnet de identidad", "Ya existe un proveedor con ese carnet de identidad".
- **Estado inválido:** "Solo se pueden editar las facturas que están Por Pagar" (o "Por Cobrar" / "Pendiente" según el tipo), "Una factura cancelada no se puede reactivar ni editar", "La factura ya está en ese estado", "El producto «X» está descontinuado. Actívalo para usarlo en facturas".
- **Stock:** "No hay stock suficiente de «X» (disponible: N)".
- **Tope de stock (H-A3):** "El stock total de «X» no puede superar 999.999,999".
- **Lotes en uso (FE-04):** "No se puede cambiar el costo de «X»: ya se usaron unidades de esta compra en la factura V-00012", "No se puede reducir «X» por debajo de las N unidades ya usadas", "No se puede quitar «X»: ya se usaron unidades de esta compra".
- **Factura asociada:** "La factura original debe ser una venta o servicio de este cliente que no esté cancelada", "La factura de servicio debe ser de este cliente y no estar cancelada".
- **Totales y fechas:** "El total de la factura no puede superar $999.999.999,99", "La fecha no puede ser futura", "Una factura admite entre 1 y 100 productos", "El producto ya está en la factura".
- **PDF:** "El archivo debe ser un PDF de hasta 5 MB".
- **Exportación:** "No hay movimientos para exportar", "No se pudo guardar el archivo".
- **Inventario omitido:** aviso de 7.6 sobre productos que ya no existen.
- **Cambio entre carga y guardado:** "El cliente ya no existe", "El proveedor ya no existe", "La factura ya no existe", "El producto ya no existe".
- **Rango de fechas invertido:** "La fecha inicial no puede ser posterior a la final".
- **PDF no encontrado en disco:** "El archivo adjunto no se encuentra en el equipo. Puede haber sido movido o eliminado".

## 9. Interfaz

Se respetan las reglas de interfaz de `spec-02` §9: todo dato viene del backend (sin mock); todo dato del usuario se muestra como **texto, nunca como HTML**; las búsquedas y vistas previas esperan 300 ms y descartan respuestas viejas; todo botón que guarda o confirma se deshabilita durante la llamada; no se activa CSP (PEN-12); los buscadores con lista y los diálogos siguen las reglas de teclado y foco. Los textos visibles son en español y los números se muestran como `1.234,50`.

Adaptaciones del mockup `dashboard-unificado.html` que esta spec exige:

- **Clientes y Proveedores:** se eliminan los datos simulados y el campo "Fecha de Entrada" del formulario de alta/edición. La fecha de alta se calcula en el backend y se muestra solo en el detalle. Los indicadores del cliente y del proveedor y su historial de facturas se llenan con datos reales (con estado vacío si no hay facturas).
- **Facturas de entrada:** se eliminan **Descuento** e **Impuesto** (tanto los inputs del formulario como los bloques del detalle) y el selector de unidad por línea (la unidad es la del producto, de solo lectura); "Observaciones" y "Descripción" se unifican; el estado ofrece Por Pagar o Pagada al crear; el método de pago (Efectivo, Transferencia, Tarjeta, Zelle) aparece solo al marcar Pagada; los `datalist` se sustituyen por buscadores con lista y alta rápida; las cantidades admiten 3 decimales; se añade la elección de fecha.
- **Facturas de salida:** los estados pasan a Por Cobrar / Cobrada / Cancelada en venta y servicio, y a Pendiente / Completada / Cancelada en garantía e insumos (se elimina "Vencida"); se aplican las mismas reglas de método de pago, fecha y buscadores; garantía e insumos ofrecen el selector de la factura asociada del cliente; el "Costo Asociado" de pérdidas pasa a ser calculado y de solo lectura.
- **Métodos de pago:** las opciones se reducen a **Efectivo, Transferencia, Tarjeta y Zelle**. Se elimina "Efectivo CUP".
- **Confirmaciones con consecuencias:** eliminar o cancelar una compra muestra las unidades que se perderán por producto; eliminar un contacto muestra cuántas facturas conserva.
- **Historial:** la pestaña del producto y su botón "Ver Historial" quedan visibles; el selector incluye el grupo de productos eliminados; los botones de exportar se añaden junto a los filtros.
- **Productos y categorías:** se activan las columnas "Valor Total" e "Ingresos" y la sección "Facturas Asociadas".
- **Lista general de facturas:** se conecta a datos reales como lista de solo lectura (FG-01).
- Los indicadores, "Mejor Vendidos" y "Lento Movimiento" del dashboard siguen en estado vacío.
- **Sección Configuración:** no se modifica en esta spec.

### 9.X. Impacto en la interfaz existente

Esta sección lista **todos** los cambios que esta spec exige en `dashboard-unificado.html` y `dashboard.js`.

**9.X.1. Cambios en elementos existentes:**

| Elemento | Cambio | Motivo |
|---|---|---|
| `#factura-estado` | `<option value="pending">Por Pagar</option>` · `<option value="completed">Pagada</option>` · `<option value="cancelled">Cancelada</option>` | FE-06, §7.5 |
| `#factura-venta-estado` | `<option value="pending">Por Cobrar</option>` · `<option value="completed">Cobrada</option>` · `<option value="cancelled">Cancelada</option>` | FS-01, §7.5 |
| `#factura-servicio-estado` | Igual que `#factura-venta-estado` | FS-01, §7.5 |
| `#factura-insumo-estado` | `<option value="pending">Pendiente</option>` · `<option value="completed">Completada</option>` · `<option value="cancelled">Cancelada</option>` | FS-05, §7.5 |
| `#factura-garantia-estado` | Igual que `#factura-insumo-estado` | FS-04, §7.5 |
| `#factura-perdida-costo` | `<input type="text" readonly>` | FP-01 |
| `#factura-perdida-cantidad` | `<input type="text" inputmode="decimal">` (acepta 3 decimales) | §7.2 |
| `#factura-perdida-producto` | Combobox (`combobox.js`), sin "Agregar nueva" | §9.X.3 |
| `#factura-garantia-original` | **Crear** `<select required>` con las facturas de venta y servicio del cliente | FS-04 |
| `#factura-insumo-servicio` | **Crear** `<select required>` con las facturas de servicio del cliente | FS-05 |
| `#historial-producto-tipo` | `<select multiple>` con 3 `<optgroup>`; `value` = códigos internos; sin "Todos" | HP-02 |
| `#factura-general-tipo` | `<option value="">Todos</option>` + `<option value="E">Entrada</option>` + `<option value="V">Venta</option>` + `<option value="S">Servicio</option>` + `<option value="GG">Garantía</option>` + `<option value="GI">Insumo</option>` + `<option value="P">Pérdida</option>` | FG-01 |
| `#factura-general-estado` | `<option value="">Todos</option>` · `<option value="pendientes">Pendientes</option>` · `<option value="completadas">Completadas</option>` · `<option value="canceladas">Canceladas</option>` | FG-01 |
| `#historial-producto-select` | Añadir `<optgroup>` para productos eliminados | HP-02 |
| `#historial-exportar-excel`, `#historial-exportar-pdf` | **Crear** dos botones en `.historial-toolbar` (con `title` y `aria-live` para el caso "sin movimientos") | HP-03 |
| `#factura-garantia-ver-original` | Mostrar `{numero} · {tipo visible}` (por ejemplo `V-00005 · Venta`) | FS-04 |
| `#factura-insumo-ver-asociada` | Ídem | FS-05 |
| Botones "Ver Agotados" / "Ver Descontinuados" del dashboard | Reemplazar los dos `onclick` idénticos por listeners que abren la pestaña P-13 con filtro | DASH-01 (`spec-02`) |
| Los `<datalist>` de clientes, proveedores, productos e insumos | Sustituir por combobox (`spec-02`) | §9 |
| Vistas de facturas (12 funciones) | Reescribir con `textContent`/`createElement` en lugar de `innerHTML` con datos | XSS almacenado (§9) |
| `<select>` de método de pago (5 formularios) | Renombrar "Efectivo USD" → "Efectivo", "Transferencia USD" → "Transferencia". Eliminar "Efectivo CUP". | §9, §12 ter |
| `#producto-condiciones-toolbar` | Reutilizar el existente (verificado en el HTML real) | §9.X.2 |
| `#cliente-fecha-entrada` (y su `<label>`) | **Eliminar del formulario** de alta/edición | §5.1 |
| Bloques "Descuento (%)" e "Impuestos (%)" del **detalle** de las 6 facturas | **Eliminar** (`#factura-ver-descuento`, `#factura-ver-impuestos`, `#factura-venta-ver-descuento`, etc.) | §3.2 |

**9.X.2. Elementos ocultos a activar:**

| Elemento | Acción |
|---|---|
| Enlace del sidebar del submenú de facturas | Quitar `hidden`. **Nota:** el selector exacto (`id` o `data-page`) no está confirmado en el HTML real; se verifica en `plan-03` §0 antes de implementar y se documenta el valor real. |
| `#page-facturas-entrada` | Quitar `hidden` |
| `#page-facturas-venta` | Quitar `hidden` |
| `#page-facturas-servicios` | Quitar `hidden` |
| `#page-facturas-insumos` | Quitar `hidden` |
| `#page-facturas-garantia` | Quitar `hidden` |
| `#page-facturas-perdidas` | Quitar `hidden` |
| `#page-facturas-general` | Quitar `hidden` |
| `#tab-producto-historial` | Quitar `hidden` |
| `#producto-historial` | Quitar `hidden` |
| `#producto-ver-historial` | Quitar `hidden` |
| `#producto-ver-facturas` | Quitar `hidden` |
| `#producto-facturas-asociadas` | Quitar `hidden` |
| `#productos-valor-total` | Quitar `hidden` |
| `#producto-condiciones-toolbar` | Quitar `hidden` |

**Estructura de líneas de factura:** todos los formularios de facturas y sus vistas de detalle mantienen la estructura `div.li-row.li-body` (o `div.li-row.li-body.li-body-readonly`). El paginador global (`spec-02`) las reconoce con el selector `.li-row.li-body`.

**9.X.3. Elementos a eliminar:**

| Elemento | Motivo |
|---|---|
| `#factura-descuento`, `#factura-impuesto` | §3.2 |
| `#factura-venta-descuento`, `#factura-venta-impuesto` | §3.2 |
| `#factura-servicio-descuento`, `#factura-servicio-impuesto` | §3.2 |
| `#factura-insumo-descuento`, `#factura-insumo-impuesto` | §3.2 |
| `#factura-garantia-descuento`, `#factura-garantia-impuesto` | §3.2 |
| Los `<div class="factura-summary-row">` de Descuento e Impuestos en los 6 detalles de factura | §3.2 |
| `#factura-garantia-dropzone` | FS-04 (reemplazado por `<select>`) |
| `#factura-insumo-dropzone` | FS-05 (reemplazado por `<select>`) |
| `<option value="Vencida">` en los formularios de factura con selector de estado (entrada, venta, servicio, garantía, insumo; no en pérdida) | §7.5 |
| `<option>Efectivo CUP</option>` en los formularios de factura con selector de método de pago (entrada, venta y servicio) | §9 |
| `<datalist>` (`#proveedores-datalist`, `#clientes-datalist`, `#insumos-datalist`, `#productos-datalist`) | §9 |
| `#cliente-fecha-entrada` y su `<label>` | §5.1 |
| `<option value="">Todos</option>` de `#historial-producto-tipo` | HP-02 |

**9.X.4.a. Funciones de `dashboard.js` a eliminar:**

- **Clientes:** `clientesData`, `nextClienteSeq`, `editingClienteId`, `clienteVerId`, `showClienteTab`, `cancelarClienteForm`, `editarCliente`, `guardarCliente`, `renderClientes`, `eliminarCliente`, `verCliente`, `editarClienteDesdeVer`, `eliminarClienteDesdeVer`, `irCrearCliente`, `calcularResumenFacturasCliente`.
- **Proveedores:** `proveedoresData`, `nextProveedorSeq`, `editingProveedorId`, `proveedorVerId`, `proveedoresDisponibles`, `actualizarProveedoresDisponibles`, `showProveedorTab`, `cancelarProveedorForm`, `editarProveedor`, `guardarProveedor`, `renderProveedores`, `eliminarProveedor`, `verProveedor`, `editarProveedorDesdeVer`, `eliminarProveedorDesdeVer`, `irCrearProveedor`, `calcularResumenFacturasProveedor`, `poblarSelectHistorialProveedor`, `obtenerMovimientosProveedor`, `renderHistorialProveedor`, `verHistorialProveedor`.
- **Facturas de entrada:** `facturasEntradaData`, `nextFacturaEntradaSeq`, `editingFacturaId`, `facturaLineItems`, `nextLineItemId`, `facturaArchivoActual`, `renderFacturasEntrada`, `eliminarFactura`, `showFacturaEntradaTab`, `verFactura`, `eliminarFacturaDesdeVer`, `prepararNuevaFactura`, `cancelarFacturaEntradaForm`, `agregarLineaFactura`, `agregarLineaDesdeBusqueda`, `crearProductoRapido`, `quitarLineaFactura`, `actualizarLineaFactura`, `renderLineasFactura`, `recalcularTotalesFactura`, `manejarArchivoFactura`, `manejarDropArchivo`, `quitarArchivoFactura`, `guardarFactura`.
- **Facturas de venta:** `facturasVentaData`, `nextFacturaVentaSeq`, `editingFacturaVentaId`, `facturaVentaLineItems`, `nextVentaLineItemId`, `renderFacturasVenta`, `eliminarFacturaVenta`, `facturaVentaVerNumero`, `verFacturaVenta`, `eliminarFacturaVentaDesdeVer`, `showFacturaVentaTab`, `prepararNuevaFacturaVenta`, `cancelarFacturaVentaForm`, `renderLineasVenta`, `recalcularTotalesVenta`, `guardarFacturaVenta`.
- **Facturas de servicio:** `facturasServiciosData`, `nextFacturaServicioSeq`, `editingFacturaServicioId`, `facturaServicioLineItems`, `nextServicioLineItemId`, `renderFacturasServicios`, `eliminarFacturaServicio`, `facturaServicioVerNumero`, `verFacturaServicio`, `eliminarFacturaServicioDesdeVer`, `showFacturaServiciosTab`, `prepararNuevaFacturaServicio`, `cancelarFacturaServiciosForm`, `agregarLineaServicio`, `agregarLineaDesdeBusquedaServicio`, `crearInsumoServicioRapido`, `quitarLineaServicio`, `actualizarLineaServicio`, `renderLineasServicio`, `recalcularTotalesServicio`, `guardarFacturaServicio`.
- **Facturas de insumo:** `facturasInsumosData`, `nextFacturaInsumoSeq`, `editingFacturaInsumoId`, `facturaInsumoLineItems`, `nextInsumoLineItemId`, `facturaInsumoArchivoActual`, `insumosDisponibles`, `renderFacturasInsumos`, `eliminarFacturaInsumo`, `facturaInsumoVerNumero`, `verFacturaInsumo`, `eliminarFacturaInsumoDesdeVer`, `showFacturaInsumosTab`, `prepararNuevaFacturaInsumo`, `cancelarFacturaInsumosForm`, `agregarLineaInsumo`, `agregarLineaDesdeBusquedaInsumo`, `crearInsumoRapido`, `quitarLineaInsumo`, `actualizarLineaInsumo`, `renderLineasInsumo`, `recalcularTotalesInsumo`, `manejarArchivoInsumo`, `manejarDropArchivoInsumo`, `quitarArchivoInsumo`, `guardarFacturaInsumo`.
- **Facturas de garantía:** `facturasGarantiaData`, `nextFacturaGarantiaSeq`, `editingFacturaGarantiaId`, `facturaGarantiaLineItems`, `nextGarantiaLineItemId`, `facturaGarantiaArchivoActual`, `renderFacturasGarantia`, `eliminarFacturaGarantia`, `facturaGarantiaVerNumero`, `verFacturaGarantia`, `eliminarFacturaGarantiaDesdeVer`, `showFacturaGarantiaTab`, `prepararNuevaFacturaGarantia`, `cancelarFacturaGarantiaForm`, `agregarLineaGarantia`, `agregarLineaDesdeBusquedaGarantia`, `crearProductoGarantiaRapido`, `quitarLineaGarantia`, `actualizarLineaGarantia`, `renderLineasGarantia`, `recalcularTotalesGarantia`, `manejarArchivoGarantia`, `manejarDropArchivoGarantia`, `quitarArchivoGarantia`, `guardarFacturaGarantia`.
- **Facturas de pérdida:** `facturasPerdidasData`, `nextFacturaPerdidaSeq`, `editingFacturaPerdidaId`, `renderFacturasPerdidas`, `eliminarFacturaPerdida`, `facturaPerdidaVerNumero`, `verFacturaPerdida`, `eliminarFacturaPerdidaDesdeVer`, `showFacturaPerdidasTab`, `prepararNuevaFacturaPerdida`, `cancelarFacturaPerdidasForm`, `guardarFacturaPerdida`.
- **Lista general:** `facturaTipoInfo` y `obtenerFacturasCombinadas` se eliminan. `renderFacturasGeneral` e `irAFacturaDesdeGeneral` se reescriben para leer del backend con `listInvoices(filters)`.
- **Vistas de facturas a reescribir con `textContent`/`createElement`:** `verFactura`, `verFacturaVenta`, `verFacturaServicio`, `verFacturaInsumo`, `verFacturaGarantia`, `renderFacturasEntrada`, `renderFacturasVenta`, `renderFacturasServicios`, `renderFacturasInsumos`, `renderFacturasGarantia`, `renderFacturasPerdidas`, `renderFacturasGeneral`.
- **Bloque final de notificaciones:** el IIFE que recorre `facturasEntradaData`, etc. para crear notificaciones de facturas "Vencida", **y** la llamada `crearNotificacion('factura', 'Bienvenido al Sistema', ...)`, **y** el `.forEach` que marca las notificaciones como leídas, **se eliminan completos**.
- **`recalcularTotalesFactura`, `recalcularTotalesVenta`, `recalcularTotalesServicio`, `recalcularTotalesInsumo`, `recalcularTotalesGarantia`:** se eliminan. La vista previa del total la hace el nuevo módulo con una función interna.
- **`opcionesUnidadHTML`:** se elimina (código muerto).
- **`poblarDatalists` y `actualizarProveedoresDisponibles`:** se eliminan.
- **Llamadas a `crearNotificacion` y `notificarFactura` desde facturas y pérdidas:** se eliminan.

**9.X.4.b. Funciones de `dashboard.js` a conservar y adaptar (no eliminar):**

- `exportarFacturaPDF`, `exportarFacturaVentaPDF`, `exportarFacturaServicioPDF`, `exportarFacturaInsumoPDF`, `exportarFacturaGarantiaPDF`, `exportarFacturaPerdidaPDF`. **No se reescriben por XSS** (jsPDF no interpreta HTML). Se adaptan para leer del DTO de detalle (`getInvoice`) en vez de los arreglos simulados.

**9.X.5. Funciones de `dashboard.js` a mantener:**

- **Navegación SPA:** `showPage`, `toggleSidebar`, `toggleMobileSidebar`, `toggleSubMenu`, `knownPages`.
- **Sub-pestañas:** `showConfigTab`.
- **Ordenamiento por fecha:** `ordenFechaState`, `ordenFechaThIds`, `fechaAValorOrdenable`, `ordenarPorFecha`, `actualizarIconoOrdenFecha`, `inicializarIconosOrdenFecha`, `toggleOrdenFecha`.
- **Utilidades:** `formatearFecha`, `togglePassword`.
- **Paginación genérica:** todo el bloque de paginación (`PAGINABLES`, `aplicarPaginacion`, etc.).
- **Sección Configuración completa.**
- **Dashboard IIFE:** el IIFE del dashboard se mantiene **intacto** en esta spec. La retirada de la parte de agotados (que colisiona con la tarjeta nueva de `spec-02`) queda pendiente y se realizará en la spec de Dashboard (PEN-16). `spec-02` no la retiró; `spec-03` tampoco lo hace.
- **Notificaciones (UI completa):** `notificacionesData`, `nextNotificacionId`, `notifTabActual`, `notifIconos`, `crearNotificacion`, `showNotifTab`, `marcarNotificacionLeida`, `marcarTodasNotificacionesLeidas`, `actualizarBadgesNotificaciones`, `formatearFechaHoraNotif`, `renderNotificaciones`. Solo se eliminan las llamadas a `crearNotificacion` y `notificarFactura` desde facturas y pérdidas.

**`formatearFecha`:** se mantiene como wrapper del helper `formatDate` de `js/utils/formatters.js` (`spec-02`). Como `dashboard.js` es un script clásico, se usa **import dinámico cacheado al arranque** con un fallback mínimo (formateo por partes de la cadena) mientras resuelve el import. La salida es idéntica en ambos casos.

**Unificación del texto vacío en fechas:** `formatearFecha()` devuelve `"—"` cuando la fecha es nula o vacía. Los demás `|| ":"` del código legado (método de pago, CI, etc.) **se quedan como están**: la unificación a `"—"` aplica solo a fechas nulas o vacías en el código nuevo.

## 10. Pendientes para próximas specs

| # | Estado tras esta spec |
|---|---|
| PEN-01 | Sigue pendiente (Notificaciones). |
| PEN-02 | Sigue pendiente; ya existen ventas, así que N-02, N-03, N-04 y N-07 pueden especificarse con los umbrales de P-12. |
| PEN-04 | **Resuelto** (FE-01, LT-03: un descontinuado no puede incluirse en facturas nuevas). |
| PEN-05 | **Resuelto** (7.7: copias en facturas, líneas y movimientos; el usuario eliminado conserva su nombre). |
| PEN-06 | Sigue pendiente. |
| PEN-07 | Sigue pendiente. |
| PEN-08 | **Parcial**: el descuento de inventario queda resuelto (FS-01, FP-01); el aviso de producto agotado al intentar vender pasa a PEN-02. |
| PEN-09 | **Resuelto** (HP-01 a HP-04). |
| PEN-10 | **Parcial**: "Valor Total" e "Ingresos" resueltos (SP-03); KPIs, "Mejor Vendidos" y "Lento Movimiento" pasan a PEN-16. |
| PEN-11 | **Se mueve a PEN-15.** |
| PEN-12, PEN-14 | Siguen pendientes; PEN-12 se amplía a las secciones de esta spec que se migran. |
| PEN-13 | Sigue pendiente. Esta spec registra quién creó cada factura y quién ejecutó cada movimiento, pero no quién eliminó o canceló. |
| **PEN-15** (nuevo) | **Respaldo y migración de datos** (de `pendientes.md`). Absorbe PEN-11 y sustituye a RF-097. Los PDF adjuntos de las facturas de entrada se guardan en `app_data_dir/attachments/`; el respaldo tiene que copiar la base y ese directorio, o exportar todo a un único archivo comprimido. |
| **PEN-16** (nuevo) | **Dashboard** (de `pendientes.md`): KPIs, "Mejor Vendidos" y "Lento Movimiento", con definiciones propias, sobre las ventas y movimientos que esta spec ya genera. |
| **PEN-17** (nuevo) | **Exportación a PDF del historial con alfabetos no latinos.** `printpdf` con Helvetica integrada cubre Latin-1/WinAnsi (`á é í ó ú ü ñ ¿ ¡ €`). Si en el futuro el proyecto necesita otros alfabetos (cirílico, griego, árabe…), se incrusta una fuente TrueType en `export.rs`. Decisión: no se hace en `spec-03` porque el proyecto es 100 % en español. |

### Límites aceptados

- **Crecimiento de la tabla de movimientos:** no se purga. Los filtros por rango de fechas de HP-02 permiten consultar cualquier período.
- **PDFs en el sistema de archivos:** los PDF adjuntos se guardan en `app_data_dir/attachments/`, no en la base.
- **Eliminar un producto con consumos activos:** un producto eliminado no recibe devoluciones de facturas activas que lo consumieron. La Reversión queda registrada con cantidad 0, y el usuario recibe un aviso.
- **Adjuntos huérfanos:** el barrido del arranque es best-effort.
- **Cambio de costo por re-FIFO sin movimiento (H-A4):** editar una salida sin cambiar la cantidad de una línea puede darle un costo real distinto por re-FIFO. La factura actualiza su `unitPriceCents`, pero el historial no registra el cambio silencioso. Es una decisión aceptada para no complicar el cálculo del Ajuste.

## 11. Supuestos

Todas confirmadas.

| # | Supuesto | Estado | Notas |
|---|---|---|---|
| S-1 | Dos entidades (`clients`, `suppliers`); CI único, teléfono no único; correo, dirección e información de factura opcionales; fecha de alta automática. | Confirmado | |
| S-2 | Ambos roles pueden hacer todo. | Confirmado | |
| S-3 | Eliminar un contacto o producto es físico; las facturas conservan una copia (7.7). | Confirmado | |
| S-4 | Inventario por lotes FIFO por orden de **registro**; costo del producto = promedio de los lotes con existencia; el precio de venta sube cuando se agota un lote barato; se registra costo real en garantía, insumos y pérdidas. | Confirmado | |
| S-5 | Cancelar o eliminar una compra quita solo lo que queda de sus lotes, con aviso previo; el stock nunca es negativo. | Confirmado | |
| S-6 | Devolución a un lote inexistente: lote de restitución con el mismo costo, el más antiguo. | Confirmado | El costo sale del consumo, no del lote (§7.7). |
| S-7 | Restricciones de edición de una compra con lotes consumidos; costo bloqueado si el lote tiene consumo. | Confirmado | |
| S-8 | Estados Por Pagar/Por Cobrar, Pagada/Cobrada, Cancelada; Cancelada final; editar solo Por Pagar/Por Cobrar; eliminar en cualquier estado. | Confirmado | |
| S-9 | Método de pago opcional, elegido al marcar Pagada/Cobrada, en entrada, venta y servicio (garantía e insumos **sin método de pago**). | Confirmado | |
| S-10 | Al editar ventas y servicios, las líneas existentes conservan su precio. | Confirmado | |
| S-11 | Pérdidas: un producto, sin edición ni estados, costo calculado. | Confirmado | |
| S-12 | Numeración independiente por tipo; lista general de solo lectura. | Confirmado | El contador vive en `invoiceCounters` (§5.3, §7.4). |
| S-13 | PDF adjunto guardado en `app_data_dir/attachments/`, hasta 5 MB, validado por contenido (`%PDF-`). | Confirmado | |
| S-14 | Historial: tipos de HP-01, cantidad con signo, productos eliminados consultables. | Confirmado | |
| S-15 | Exportación a Excel y PDF incluida; importación sustituida por PEN-15. | Confirmado | |
| S-16 | Solo "Valor Total" e "Ingresos" en esta spec; el resto del dashboard a PEN-16. | Confirmado | |
| S-17 | Total de factura máximo $999.999.999,99; máximo 100 líneas. | Confirmado | El tope protege la precisión de enteros de JavaScript. |
| S-18 | Fecha de factura no futura y no anterior al 2000-01-01. | Confirmado | Decisión de producto. |
| S-19 | Largos máximos de 7.1. | Confirmado | |
| S-20 | Detalle de la relevancia de búsqueda (niveles, campo, 20 resultados) y búsqueda sin espacios ni `+` en teléfonos. | Confirmado | |
| S-21 | El cliente o proveedor creado desde el formulario rápido queda guardado aunque se cancele la factura. | Confirmado | |
| S-22 | Un descontinuado no se usa en facturas nuevas ni líneas nuevas. **Sí** se puede registrar su pérdida si tiene stock. Sus líneas existentes se conservan sin cambios al editar. | Confirmado | Excepción en pérdidas. |
| S-23 | Pagada/Cobrada → Cancelada permitido. | Confirmado | |
| S-24 | Se registra una Reversión por línea quitada; un Ajuste solo cuando la cantidad de la línea cambia. El cambio de costo por re-FIFO **no** genera movimiento (H-A4). El costo de la Reversión con cantidad 0 sale del consumo. | Confirmado | |
| S-25 | Garantía e insumos usan etiquetas propias **Pendiente / Completada / Cancelada**. | Confirmado | |
| S-26 | Los indicadores del cliente cuentan solo ventas y servicios no cancelados. | Confirmado | |
| S-27 | El historial del cliente muestra ventas, servicios, garantías e insumos. | Confirmado | |
| S-28 | "Ingresos" de una categoría usa la categoría **guardada en la línea**. | Confirmado | Las ventas anteriores no se mueven si el producto cambia de categoría. |
| S-29 | Tope de stock por producto: 999.999,999 unidades (H-A3). | Confirmado | Coincide con el máximo de `stockAfterMilli` y de cada lote. |
| S-30 | Throttle de registro: 60 s por `(code, userId)` para los códigos nuevos con estado (H-A2). | Confirmado | `validationFailed` e `invalidCredentials` siguen sin registrarse. |

## 12. Desviaciones respecto de `Requisitos_del_sistema.docx` y de specs anteriores

- **RF-097 (importar historial desde Excel):** no se implementa; lo sustituye el respaldo completo de PEN-15.
- **RF-051 (recalcular precio promedio):** sustituido por el cálculo sobre lotes (FE-08, LT-03). **RF-048 y RF-049:** cancelar o eliminar una compra no "elimina los productos del inventario" completos, sino lo que queda de sus lotes.
- **RF-075 y RF-084:** el costo de garantía e insumos es el costo real de los lotes consumidos, no el promedio actual. **RF-086:** el costo de la pérdida no se escribe, se calcula.
- **RF-044:** el producto nuevo sobre la marcha pide **% de ganancia** y no precio de venta; los descontinuados no se pueden incluir.
- **RF-047, RF-055, RF-064, RF-071, RF-080:** solo se editan las facturas Por Pagar o Por Cobrar. **RF-049, RF-057, RF-066, RF-073, RF-082:** Cancelada es final.
- **RF-052, RF-061, RF-068, RF-077:** se mantiene una página por tipo, como el mockup.
- **RF-032, RF-038:** los contactos añaden Correo y Dirección (clientes también Información de factura), tomados del mockup.
- **Estados:** "Pagado/Cobrado" se muestran como "Pagada/Cobrada"; "Vencida" del mockup no existe. Garantía e insumos usan Pendiente / Completada / Cancelada (S-25).
- **RF-091:** se añaden los tipos Inicial, Ajuste y Reversión y la cantidad con signo. El Ajuste solo se emite por cambio de cantidad (H-A4).
- **RF-033, RF-039 y siguientes:** donde el requisito dice "la contadora", esta spec habilita a Administrador y Contador.
- **Adiciones sin RF de origen:** lista general de facturas (FG-01), historial de compras del proveedor (PV-07), "Facturas Asociadas" (HP-04), "Valor Total" e "Ingresos" (SP-03) y movimientos de productos eliminados consultables (HP-02).
- **`spec-02`:** la cantidad y el costo dejan de fijarse solo al crear; los agotados pasan a existir en el uso real; P-10 se sustituye por el cálculo sobre lotes.
- **`spec-01` US-07:** el manejo de la referencia a un usuario eliminado queda definido en 7.7.
- **Sobre el PDF adjunto:** se guarda en `app_data_dir/attachments/`, con la base guardando solo la referencia.
- **Sobre las notificaciones del mockup:** se eliminan las llamadas a `crearNotificacion` y `notificarFactura` desde facturas y pérdidas. La UI de la pestaña de Notificaciones y su código se mantienen intactos.
- **Consistencia de las cachés `quantityMilli` y `costCents`:** la igualdad con los lotes no se puede expresar como `CHECK` en SQLite. Se garantiza con sincronización en código + tests. Se registrará en el Complexity Tracking de `plan-03.md` como desviación de `Constitution.md` §4.
- **Sobre los PDFs adjuntos y `Constitution.md` §2.I/§4:** los PDFs se guardan en el sistema de archivos. Justificación: (1) son archivos binarios grandes; (2) SQLite recomienda no almacenar blobs grandes; (3) la información mostrada al usuario sigue viniendo de SQLite; (4) el respaldo (PEN-15) copiará la base y el directorio de adjuntos.
- **Sobre `weighted_average_cost_cents` (P-10):** se **elimina** porque el costo del producto se calcula sobre lotes (LT-03).
- **Sobre las modificaciones al mockup y `Constitution.md` §2.V:** esta spec modifica numerosos elementos del mockup, todos justificados por un RF o por una decisión del dueño.
- **Sobre la creación sobre la marcha y `Constitution.md` §2.I:** la creación sobre la marcha de contactos, categorías y unidades **no** contradice §2.I: el dato se crea en la base de datos (a través del comando Tauri).
- **Sobre el registro de los códigos nuevos con estado (H-A2):** `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded` se registran con throttle de 60 s por `(code, userId)`. **No hay desviación** de `Constitution.md` v1.2.0 §2.VI: la excepción de los rechazos esperados sigue aplicando a `validationFailed` e `invalidCredentials`, y los códigos nuevos no son rechazos "que se repiten sin límite" gracias al propio throttle.
- **Sobre la regla de eliminación de Canceladas (H-A1):** es una excepción explícita al principio "Cancelada se elimina sin tocar el inventario", necesaria para preservar el invariante de caché.

## 12 bis. Impacto en `spec-02`

Esta spec **modifica** varias decisiones de `spec-02` e introduce consecuencias que afectan a sus tests y criterios de prueba. `spec-02` **no se reescribe**: sigue siendo la referencia histórica de la feature 02. Los cambios se implementan desde esta spec y se documentan aquí.

**Cambios de comportamiento:**

| Punto de `spec-02` | Comportamiento anterior | Comportamiento nuevo |
|---|---|---|
| `quantityMilli` de `products` | Se fija al crear y ningún comando la modifica después | Se deriva de los lotes; se mantiene sincronizada como caché |
| `weighted_average_cost_cents` (P-10) | Función sin consumidor | Se **elimina**; el costo se calcula sobre lotes |
| Productos agotados | "En el uso real, ningún activo está agotado" | **Sí existen**: las salidas llevan productos a 0 |
| Pestaña P-13 y filtros de agotados | Se verificaban solo con fixtures | **Datos reales** por salidas |
| DASH-01 | Se verificaba solo con fixtures | **Datos reales** por salidas |
| Crear producto (P-01) | Crea el producto con cantidad inicial, sin lote | Crea el producto **y su lote inicial**, y registra un movimiento Inicial |
| Editar producto (P-03) | No acepta cantidad, costo ni precio de venta | Igual |
| Eliminar producto (P-14) y purga (P-08) | No había lotes; no había cascada | Los lotes se eliminan con el producto |
| "Valor Total" (PEN-10) | Oculto | Se activa con datos reales |
| "Ingresos" (PEN-10) | Oculto | Se activa con datos reales, usando la categoría de la línea |

**Tests de `spec-02` que cambian o se adaptan:**

| Test de `spec-02` | Antes | Ahora |
|---|---|---|
| §6.1 "ningún comando de esta spec la modifica después" | Cierto (dentro de `spec-02`) | **Falso globalmente**: los comandos de `spec-03` sí la modifican. El test sigue siendo cierto **dentro de `spec-02`**. |
| §6.6 "los comandos... no alteran la cantidad" | Cierto | Igual que el anterior. |
| §14 "con datos reales ningún activo está agotado" | Cierto | **Falso** tras `spec-03`. |
| §8 "P-10 sin consumidor" | Cierto | P-10 **se elimina**. |
| Tests de P-13 y DASH-01 que crean agotados con fixtures | Correctos | Siguen siendo válidos, pero **ya no son la única forma**. |

**Decisión:** los tests de `spec-02` **no se borran**. Se mantienen como referencia de la feature 02, con las anotaciones anteriores. Los tests de `spec-03` que verifiquen los nuevos comportamientos se añaden en `tasks-03.md`. La coexistencia de las dos suites en el mismo `cargo test` la decide `plan-03.md` (por ejemplo, con módulos de test separados).

**Cambios de esquema:**

| Tabla | Cambio |
|---|---|
| `products` | `quantityMilli` **y** `costCents` se conservan como cachés sincronizadas con los lotes. |
| `pricing.rs` | Se elimina la función `weighted_average_cost_cents`. `selling_price_cents` se conserva. |
| Líneas de factura (nuevas) | Guardan `unitPriceCents`, `categoryId`/`categoryName` (snapshots sin FK; solo ventas y servicios) y `costAtSaleCents` (solo ventas y servicios). |
| Facturas (nuevas) | Tabla única `invoices` con discriminador `type`, más `invoiceCounters(type, nextNumber)`. |
| **`ProductListItem` (DTO)** | **Añadir `valorTotalCents: number`** (SP-03). |
| **`ProductDetail` (DTO)** | **Añadir `valorTotalCents: number`** (SP-03). |
| **`CategoryListItem` (DTO)** | **Añadir `ingresosCents: number`** (SP-03). |
| **`CategoryDetail` (DTO)** | **Añadir `ingresosCents: number`** (SP-03). |

## 12 ter. Desviaciones respecto del mockup `dashboard-unificado.html`

| Elemento del mockup | Desviación | Motivo |
|---|---|---|
| Estado "Vencida" | Eliminado | No existe en los RF (§12) |
| Estado "Pagada" en ventas y servicios | Cambiado a "Cobrada" | Consistencia con "factura" (§12) |
| Estado "Por Pagar" en ventas y servicios | Cambiado a "Por Cobrar" | Ídem |
| Estado "Pagada" en garantía e insumos | Cambiado a "Completada" | S-25 |
| Estado "Por Pagar" en garantía e insumos | Cambiado a "Pendiente" | S-25 |
| Opción "Efectivo CUP" | Eliminada | §9 |
| Inputs de descuento (5) | Eliminados | §3.2 |
| Inputs de impuesto (5) | Eliminados | §3.2 |
| Bloques de Descuento/Impuestos en los detalles (6) | Eliminados | §3.2 |
| Input de costo de pérdida editable | Solo lectura | FP-01 |
| Input de cantidad de pérdida `type="number"` | `type="text" inputmode="decimal"` | §7.2 |
| `dropzone` de garantía | Reemplazado por `<select>` | FS-04 |
| `dropzone` de insumos | Reemplazado por `<select>` | FS-05 |
| `<datalist>` (4) | Sustituidos por combobox | §9 |
| `<select>` de estado en ventas/servicios/insumos/garantía | Reemplazado por las opciones de §7.5 | §7.5 |
| `<select>` de tipo de movimiento en historial | `<select multiple>` con `<optgroup>` | HP-02 |
| `<select>` de estado en lista general | Agrupado en 4 opciones | FG-01 |
| Llamadas a `crearNotificacion`/`notificarFactura` | Eliminadas | §3.2 |
| Bloque final de notificaciones iniciales | Eliminado (incluida "Bienvenido al Sistema") | §3.2 |
| Cálculo del total en `recalcularTotales*` | Sustituido por función interna del módulo nuevo | §3.2 |
| Input de fecha de entrada del cliente | Eliminado | §5.1 |

## 13. Fuera de alcance (recapitulación)

- Respaldo y migración (PEN-15) y la importación de historial desde Excel.
- Notificaciones y sus tipos N-02, N-03, N-04, N-07 y el aviso de agotado al vender.
- KPIs, "Mejor Vendidos" y "Lento Movimiento" (PEN-16).
- Uso de conversiones de unidades en facturas (PEN-06); otras monedas (PEN-07).
- Descuentos, impuestos y estado "Vencida".
- Registro de quién eliminó o canceló (PEN-13).
- Exportación a PDF del historial con alfabetos no latinos (PEN-17).

## 14. Criterios de prueba

Los criterios generales de `spec-02` se mantienen. Los específicos de esta spec son:

- **Autorización:** matriz con una fila por cada uno de los **35 comandos** de §5.5.
- **Lotes y FIFO:** el ejemplo de LT-02; consumo que cruza varios lotes; orden por registro; cantidad mayor que el stock rechazada sin cambios; redondeo único por línea; costo conservado con stock 0; sin ventas, equivale al promedio de `spec-02` ($1,33).
- **Devolución:** cancelar, eliminar y editar una salida devuelve a los mismos lotes; con el lote inexistente se crea el lote de restitución más antiguo con el costo unitario del consumo original; con el producto inexistente la operación continúa con aviso.
- **Cachés de producto:** tras cualquier operación sobre lotes, `products.quantityMilli` coincide con la suma de `lotes.remainingMilli` y `products.costCents` coincide con el promedio ponderado de los lotes con existencia. Es el invariante que no se puede expresar como `CHECK`.
- **Compra:** crear con producto nuevo; rollback completo si falla cualquier línea; estado inicial Pagada con fecha de pago; editar con lote consumido; eliminar y cancelar quitan solo lo restante; Por Pagar ↔ Pagada no toca el inventario.
- **Salidas:** venta, servicio, garantía, insumos y pérdida descuentan por FIFO y registran su movimiento; total de venta y servicio a precio de venta vigente; total de garantía, insumos y pérdida a costo real; las líneas editadas conservan su precio.
- **Estados:** todas las transiciones de §7.5; Cancelada final.
- **Historial:** un movimiento por operación y por tipo; signo, stock resultante y precios correctos; no hay movimiento en cambios de estado ni al eliminar una Cancelada sin restos; el movimiento sobrevive a la eliminación del producto, de la factura y del usuario; filtros por rango de días locales y por varios tipos (semántica OR); selector con productos eliminados.
- **Exportación:** el `.xlsx` y el `.pdf` contienen exactamente los movimientos filtrados; sin movimientos, los botones están deshabilitados, con `title` y `aria-live`.
- **Adjunto PDF:** PDF válido aceptado; archivo no PDF, mayor de 5 MB o con ruta en el nombre rechazado; reemplazo y quitado en Por Pagar; descarga en cualquier estado; barrido del arranque borra huérfanos y es best-effort.
- **Numeración:** contadores independientes por tipo, sin reutilización; más de 5 dígitos pasados 99999.
- **Totales y fechas:** más de 100 líneas, producto repetido, total mayor que $999.999.999,99, fecha futura, anterior a 2000 e imposible rechazados.
- **Indicadores:** total facturado, saldo pendiente, número y última compra de clientes y proveedores.
- **Valor Total e Ingresos:** valor total por producto con varios lotes; ingresos por categoría usando la categoría guardada en la línea.
- **Atomicidad:** con un fallo forzado, no queda ningún cambio parcial.
- **Purga:** un descontinuado vencido se trata como inexistente en facturas y movimientos. `listHistoryProducts` **no purga** (decisión de diseño documentada en §7.6): un producto vencido aparece en el grupo de activos hasta que otro comando lo purgue, y luego en el de eliminados; en ambos casos sus movimientos se conservan.
- **`spec-02`:** tras cualquier operación, `quantityMilli` coincide con la suma de los lotes; crear producto crea su lote inicial y el movimiento Inicial.
- **Interfaz:** nombres con HTML se muestran como texto; los buscadores esperan 300 ms; un doble clic produce una sola llamada; ningún dato simulado queda en Clientes, Proveedores ni facturas.
- **`exportarFactura*PDF` conservadas y adaptadas:** cada función existe, no usa `innerHTML` y lee su contenido del DTO de detalle (`getInvoice`).
- **`formatearFecha` con import dinámico:** funciona síncronamente con el fallback antes de que el import resuelva y con el helper después; salida idéntica en ambos casos. Texto vacío unificado `"—"`.
- **Eliminar una compra Cancelada con lotes restaurados (H-A1):** el escenario del hallazgo — compra de 10 u, venta de 4 u, cancelar la compra (lote a 0), cancelar la venta (restaura 4 u al lote), eliminar la compra — debe quitar las 4 u restantes, registrar una Reversión y dejar `products.quantityMilli = 0` y la suma de `remainingMilli` de los lotes = 0. El invariante de caché se mantiene. Caso complementario: Cancelada sin restos (`remainingMilli = 0` en todos los lotes) se elimina sin tocar el inventario.
- **Tope de stock (H-A3):** una operación que llevaría la suma de `remainingMilli` por encima de 999.999.999 falla con `stockLimitExceeded`; la base no cambia. Se prueba en `createProduct`, `createEntryInvoice`, `updateEntryInvoice` y `restore_consumption`.
- **Cambio de costo por re-FIFO sin cambio de cantidad (H-A4):** editar una salida sin cambiar la cantidad de una línea, pero con un cambio en los lotes disponibles que altere su costo real, actualiza el `unitPriceCents` de la línea (en garantía, insumos y pérdidas) **sin registrar un Ajuste**. El historial solo tiene los movimientos originales y los de la edición para líneas con cantidad cambiada.
- **Throttle del log (H-A2):** el mismo `stockInsuficiente` con el mismo usuario produce 1 sola fila en 60 s; con `userId` distinto, 2 filas; tras 61 s (reloj inyectado) vuelve a registrar; `validationFailed` e `invalidCredentials` no generan fila nunca. 