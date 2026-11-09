# Spec: Precios, Servicios y Entregas

- **Feature:** `precios-servicios-y-entregas`
- **Estado:** **Confirmada por el dueño (2026-11-08). Lista para implementar.** Recoge las siete rondas de preguntas y dos auditorías del 2026-11-08: tras la primera se reescribieron S-01, S-02 y S-14; tras la segunda se añadió EN-05 (reservas con entregas). El dueño aprobó ambas correcciones.
- **Basada en:** `Constitution.md` v1.3.0, `Agents.md`, `spec-02` (precio de venta, lotes), `spec-03` (facturas, contactos), `spec-04` (reservas RS, ajustes CF-04, dashboard DB, respaldo RP) y `specs/05-sistema-de-diseno/ui-spec.md`.
- **Convención de referencias:** `plan-06` y `task-06` están en esta misma carpeta.
- **Siguiente paso:** ejecutar `task-06.md` por fases (§3.3).
- **Datos anteriores** (indicación del dueño, 2026-11-08): la aplicación sigue en desarrollo; las migraciones solo cuidan lo necesario para que los datos existentes sigan siendo válidos y coherentes, sin conservar resultados históricos más allá de eso.

---

## 1. Resumen

Primera spec después del MVP (versión 0.2.0). Cambia cómo se calculan el costo y el precio, permite ajustar precios y hacer descuentos en las facturas, registra las entregas, crea el catálogo de **Servicios** y añade una contraseña para restaurar respaldos:

- **Costo solo con entradas:** el costo de un producto ya no cambia al vender; solo con sus facturas de entrada.
- **Valor del inventario al costo:** stock × costo de entrada, no × precio de venta.
- **Precio de venta redondeado** hacia arriba al entero, con el precio calculado a un lado y más claro.
- **Precio editable y descuentos** por línea en las facturas de Venta y Servicio.
- **Entregas:** lo vendido queda apartado y el stock baja al entregarlo, también por partes.
- **Servicios:** catálogo propio con precio sugerido y productos asociados; se usan en las facturas de servicio.
- **Umbral de pocas unidades por producto**, en lugar de uno general.
- **Cuentas por cobrar** en lugar de Reservas Activas en el dashboard.
- **Contactos con solo el nombre obligatorio.**
- **Contraseña de respaldo** para restaurar.

## 2. Objetivo / Por qué

- **Costo real:** hoy el costo es el promedio de los lotes que quedan, así que cada venta lo mueve. El dueño quiere que el costo refleje lo que pagó y que solo cambie cuando compra.
- **Precios redondos y negociables:** el cliente paga precios enteros; el vendedor puede cobrar más en casos especiales y hacer rebajas registradas como descuento, sin tocar el precio del catálogo.
- **Servicios recurrentes:** instalaciones y mantenimientos tienen siempre los mismos productos; el catálogo evita escribirlos en cada factura.
- **Control de entregas:** el negocio vende cosas que el cliente recoge después; debe saber qué falta entregar y no venderlo dos veces.
- **Avisos útiles:** 5 unidades es poco para un cable en metros y mucho para un inversor.
- **Respaldos protegidos:** restaurar sustituye toda la información; solo debe hacerlo quien conozca la contraseña de respaldo.

## 3. Alcance

### 3.1 Incluido

Los diez bloques de la §1, con los requisitos de la §6.

### 3.2 Fuera de alcance

- **Cifrar el archivo de respaldo** (decisión del dueño): la contraseña solo protege la restauración dentro de la aplicación; el archivo sigue siendo una base SQLite legible.
- **Descuento global de factura:** los descuentos son por línea (producto o servicio).
- **Precio editable y entregas en Insumos, Garantía y Pérdida:** siguen como hoy.
- **Guardar en la ficha del servicio los productos añadidos en una factura:** solo quedan en esa factura (decisión del dueño).
- **Redondeo configurable:** siempre al entero superior.

### 3.3 Fases de implementación

1. **Costo, precio y umbral:** CO, PV, VI, UM y KP. No toca el esquema de facturas.
2. **Facturas y contactos:** reconstrucción del esquema de facturas y contactos (CT), precio editable y descuentos (PF).
3. **Entregas:** EN.
4. **Servicios:** SV (depende de 2 y 3).
5. **Contraseña de respaldo:** RP.

## 4. Roles y actores

Como en las specs anteriores, **ambos roles pueden hacer todo**, incluido subir precios, aplicar descuentos, registrar entregas y gestionar servicios (decisión del dueño), salvo:

- **Solo el Administrador:** definir o cambiar la contraseña de respaldo (RP-01).

Restaurar sigue permitido a ambos roles (`spec-04`), ahora con la contraseña de respaldo.

## 5. Modelo conceptual

(El esquema concreto es de `plan-06.md`.)

- **Costo base:** costo de un producto en el momento de empezar a contar entradas (al migrar, su costo actual; al crear un producto, su costo inicial). Es el punto de partida para recalcular (CO-01).
- **Costo:** promedio ponderado móvil: cada entrada lo mezcla con el stock que había en ese momento (CO-01).
- **Precio calculado:** costo × (1 + % de ganancia), con céntimos.
- **Precio de venta:** el precio calculado redondeado hacia arriba al entero.
- **Precio de lista de una línea:** el precio de venta del producto cuando se agregó a la factura; el precio cobrado nunca baja de él.
- **Descuento de una línea:** porcentaje o importe que rebaja el subtotal de esa línea.
- **Stock:** lo que físicamente hay. **Apartado:** lo vendido en Venta y Servicio aún no entregado. **Disponible:** stock − apartado.
- **Entrega:** cantidad de una línea que el cliente recibió, con fecha y usuario. Puede haber varias por línea.
- **Servicio:** nombre, descripción, precio sugerido y productos asociados con cantidad sugerida. Puede no tener productos.
- **Servicio en factura:** copia del servicio con su precio (editable) y descuento, y sus productos con la cantidad usada en esa factura.
- **Umbral de pocas unidades:** cantidad por producto, en su unidad, por debajo de la cual se avisa.
- **Contraseña de respaldo:** una sola para el sistema, guardada con hash; cada respaldo lleva la que estaba vigente al crearse.

## 6. Requisitos

### Módulo: Costo y valor del inventario

#### CO-01 — El costo solo cambia con entradas

- **Dado** una factura de entrada que incluye un producto, **entonces** su costo pasa a ser **(stock antes de la entrada × costo actual + cantidad que entra × costo de entrada) / (stock antes + cantidad que entra)**. Con stock 0, el costo es el de la entrada.
  - Ejemplo: 100 unidades a 10 USD se venden todas; entran 10 a 20 USD → costo 20 USD. Con 50 unidades a 10 USD en almacén, entran 50 a 20 USD → costo 15 USD.
- **Dado** una venta, servicio, insumo, garantía, pérdida o entrega, o su cancelación, **entonces** el costo del producto **no** cambia.
- **Dado** que una factura de entrada se anula, elimina o edita, **entonces** el costo se recalcula repasando en orden, desde el costo base, todas las entradas vigentes posteriores a él, con sus valores actuales (decisión del dueño). El stock antes de cada entrada es el que quedó guardado en ella al registrarla, menos lo que aportaron las entradas anteriores que ya no están vigentes, sin bajar de 0.
- **Migración:** los productos existentes conservan su costo actual (decisión del dueño), que pasa a ser su costo base. Anular, eliminar o editar una entrada **anterior** a esta versión no cambia el costo, porque el recálculo empieza en el costo base (aceptado: la aplicación sigue en desarrollo).

#### VI-01 — Valor del inventario al costo

- El **valor del inventario** de un producto es su **stock × costo** (S-07). Se usa en la columna **Valor Total** del listado de productos, en el **Resumen de Inventario** del dashboard y en las **exportaciones** CSV y Excel.

### Módulo: Precio de venta

#### PV-01 — Redondeo hacia arriba al entero

- El **precio de venta** es el precio calculado redondeado hacia arriba al entero: 18,37 → 19,00 USD; 19,00 → 19,00 USD.
- Es el precio que se propone en las facturas y el que se exporta.

#### PV-02 — El precio calculado se ve al lado

- Junto al precio de venta se muestra el **precio calculado** en un tono más claro, solo si son distintos: en el **listado de productos**, el **detalle**, el **formulario** (mientras se escriben costo y % de ganancia) y las **líneas de las facturas** de salida.

### Módulo: Precios y descuentos en facturas

#### PF-01 — Subir el precio de un producto

- **Dado** una línea de producto en una factura de **Venta** o **Servicio**, **entonces** el precio unitario es editable y empieza en el precio de venta.
- **Dado** un precio menor que el precio de lista de la línea, **entonces** no se guarda y se avisa junto al campo: "El precio no puede ser menor que X".
- Al editar una factura, el mínimo es el precio de lista guardado en esa línea, aunque el producto haya cambiado de precio (S-03).

#### PF-02 — Descuento por línea

- **Dado** una línea de producto o de servicio en Venta o Servicio, **entonces** se puede aplicar un descuento en **porcentaje** o en **importe**.
- El descuento puede dejar la línea por debajo del precio de lista (decisión del dueño): es la vía para rebajar.
- El descuento no puede superar el importe bruto de la línea (cantidad × precio): el importe de una línea nunca es negativo (S-05).
- El **importe de la línea** es el bruto menos el descuento, y es el que cuenta en todo cálculo: total de la factura, ingresos del dashboard, estadísticas de clientes, proveedores y categorías, exportaciones y búsqueda.
- La factura, la vista previa, la impresión y el PDF muestran el bruto, el descuento y el importe de cada línea con descuento.

### Módulo: Entregas

#### EN-01 — Lo vendido queda apartado

- **Dado** una factura de **Venta** o **Servicio**, **entonces** sus productos quedan **apartados**: no se descuenta el stock todavía, pero no se pueden vender a otro cliente.
- El límite de cantidad de cualquier factura de salida es el **disponible** (stock − apartado), no el stock.
- La columna **Disponible en Stock** del listado de productos muestra el **disponible** y, si hay algo apartado, al lado un **badge naranja** "N reservado" (pedido del dueño, 2026-11-08), con el stock en almacén en su texto de ayuda para que cuadre con el Valor Total (S-07). Hasta la fase 3 el badge cuenta las reservas activas; desde ella, todo lo apartado.
- **El stock nunca baja de lo apartado.** Anular, editar o eliminar una factura de entrada se rechaza si dejaría el stock de un producto por debajo de su cantidad apartada: "No se puede: N unidades de este producto están vendidas y sin entregar". (Pérdida, Garantía e Insumos ya lo cumplen porque su límite es el disponible.)
- **No se puede eliminar** un producto con cantidad apartada: "Tiene N unidades sin entregar en facturas". La purga automática de descontinuados lo salta hasta que se entregue o se cancele lo apartado. Un producto descontinuado sí se puede seguir entregando.
- El detalle del producto muestra stock, apartado y disponible, con enlace a las facturas con productos sin entregar. Sustituye a la "cantidad reservada" de `spec-04` RS-03: lo reservado es parte de lo apartado.
- **Garantía e Insumos** asociadas a una venta o servicio solo admiten, por producto, hasta la cantidad **ya entregada** en esa factura.

#### EN-02 — Registrar entregas

- **Dado** una factura de Venta o Servicio no cancelada, **entonces** se puede registrar una entrega por línea, de toda la cantidad pendiente o de una parte (decisión del dueño).
- **Dado** una entrega, **entonces** baja el stock por la cantidad entregada (consume lotes por FIFO), guarda el **costo vigente** del producto en esa entrega y registra el movimiento de salida con la fecha de entrega.
- No se puede entregar más que lo pendiente de la línea.
- Cada factura muestra **Entregada**, **Entregada parcial** o **Sin entregar**; los listados de Venta y Servicio pueden filtrarse por ese estado.
- Cobrar una factura no exige haberla entregado, y entregarla no exige cobrarla.

#### EN-03 — Cancelar, editar y migrar

- **Dado** que se cancela o se elimina una factura de Venta o Servicio, **entonces** lo entregado vuelve al stock (como hoy) y lo apartado se libera (S-11).
- Al editar una factura no se puede dejar una línea con menos cantidad que la ya entregada, ni quitar una línea o un servicio que tenga productos entregados.
- **Migración:** las facturas de Venta y Servicio existentes quedan como **entregadas** (decisión del dueño); el stock actual no cambia.
- "Reservar hasta" (`spec-04` RS) sigue con su fecha límite y es independiente de la entrega (decisión del dueño).
- **Cambia `spec-04` RS-01:** una reserva ya no descuenta el stock al guardarse; como toda venta o servicio, aparta y descuenta al entregar.

#### EN-05 — Reservas con entregas (cambia `spec-04` RS-02)

- **Dado** una reserva **sin** ninguna entrega cuya fecha límite pasó, **entonces** se cancela sola, como hasta ahora: lo apartado se libera.
- **Dado** una reserva con **alguna** entrega, **entonces** ya **no** se cancela sola (decisión del dueño): la entrega confirma la venta. Sigue Por Cobrar y su fecha límite queda como dato. Si hay que anularla, se cancela a mano (EN-03).
- Todo se notifica (decisión del dueño), una vez por situación (`spec-04` NT-03), con enlace a la factura:
  - **N-14 · Informativo:** "La reserva V-00012 tiene productos entregados: ya no se cancelará sola". Se genera con la primera entrega de una reserva activa.
  - **Aviso de reserva por vencer** (`spec-04`): si la reserva tiene entregas, el mensaje dice que vence pero que no se cancelará porque tiene productos entregados.
  - **N-15 · Advertencia:** "La reserva V-00012 venció con productos entregados y sigue por cobrar". Se genera al pasar su fecha límite.
  - **Reserva cancelada por vencimiento** (`spec-04`): sin cambios para las reservas sin entregas.

#### EN-04 — Costo de lo vendido en el dashboard

- El costo de una línea es la suma, por cada entrega, de cantidad × costo guardado en esa entrega; lo aún no entregado se valora al costo actual del producto (S-02).
- El costo se imputa a la **fecha de la factura**, igual que su ingreso, para que la ganancia de un período no muestre ventas sin costo.
- Insumos, Garantía y Pérdida usan el costo vigente al crearse.

### Módulo: Servicios

#### SV-01 — Catálogo de servicios

- Nueva entrada **Servicios** en el menú lateral, debajo de **Productos**, con listado, búsqueda, crear, editar, ver y eliminar.
- Un servicio tiene **nombre** (obligatorio, único entre los activos), **descripción** (opcional), **precio sugerido** (opcional) y **productos asociados** con su **cantidad sugerida**. Puede no tener productos, por ejemplo "Mantenimiento de paneles solares".
- Eliminar un servicio no cambia las facturas que lo usaron: guardan su copia (S-13).
- Si un producto asociado se descontinúa, la ficha del servicio lo marca como descontinuado. Al agregar el servicio a una factura se omite, con el aviso "Se omitió X: está descontinuado".

#### SV-02 — Servicios en la factura de servicio

- **Dado** una factura de servicio, **entonces** se pueden agregar servicios del catálogo o crear uno nuevo desde la misma factura (como el alta rápida de productos de `spec-03`).
- Al agregar un servicio aparecen su precio sugerido y sus productos con la cantidad sugerida. En la factura:
  - el **precio del servicio** se puede cambiar libremente (S-04) y admite descuento (PF-02);
  - la **cantidad** de cada producto asociado se puede cambiar, y un producto se puede quitar;
  - se pueden **agregar productos extra** a ese servicio, solo para esta factura (decisión del dueño).
- El total del servicio es su precio más sus productos a precio de venta, con sus descuentos (decisión del dueño): **servicio + productos**.
- Una factura de servicio puede tener varios servicios y también productos sueltos (S-15).
- Los productos de los servicios quedan apartados y se entregan como cualquier línea (EN).

### Módulo: Umbral de pocas unidades

#### UM-01 — Umbral por producto

- Cada producto tiene su **umbral de pocas unidades**, obligatorio al crearlo y al editarlo, en la unidad del producto.
- Desaparece el ajuste general "Umbral de stock bajo" de Configuración.
- **Migración:** los productos existentes quedan **sin umbral** hasta que se editen (decisión del dueño): no avisan de pocas unidades, y el listado muestra "Sin umbral".
- El aviso y la etiqueta "Stock Bajo" comparan el **disponible** con el umbral (S-06).
- **Agotado** también se basa en el disponible: un producto está agotado cuando su disponible es 0, aunque tenga unidades en almacén apartadas. Aplica a la etiqueta, la pestaña Agotados y Descontinuados, el aviso de agotado, los contadores del dashboard y las categorías.
- La importación CSV de productos exige la columna de umbral.

### Módulo: Dashboard

#### KP-01 — Cuentas por cobrar

- La tarjeta **Reservas Activas** pasa a llamarse **Cuentas por cobrar** y muestra el **total pendiente de cobro** y la **cantidad** de facturas de Venta y Servicio **Por Cobrar**, incluidas las reservas activas (S-10).
- Las reservas que vencen pronto siguen avisándose en Notificaciones (`spec-04` NT).

### Módulo: Contactos

#### CT-01 — Solo el nombre es obligatorio

- En clientes y proveedores solo el **nombre** es obligatorio (decisión del dueño); apellidos, carné, teléfono y el resto son opcionales.
- Si se escriben, conservan su formato y unicidad: carné de 11 dígitos y único; teléfono `+53XXXXXXXX`.
- Las facturas aceptan contactos sin carné ni teléfono; la impresión omite lo que falte.

### Módulo: Respaldo

#### RP-01 — Definir la contraseña de respaldo

- En **Configuración > Respaldos**, el **Administrador** define la contraseña de respaldo (mínimo 8 caracteres, con confirmación).
- Para definirla o cambiarla se pide su **contraseña de usuario**; así también se redefine si la olvidó (decisión del dueño, S-09).
- Hasta que se defina, la sección avisa de que los respaldos que se hagan mientras tanto no quedan protegidos: se podrán restaurar con la contraseña de usuario (RP-02).

#### RP-02 — Restaurar con la contraseña de respaldo

- Restaurar pide la **contraseña de respaldo** en lugar de la del usuario (decisión del dueño).
- Se valida contra la contraseña **guardada dentro del archivo** que se restaura (decisión del dueño): funciona en un equipo nuevo, y un respaldo antiguo pide la contraseña que tenía cuando se hizo.
- Los respaldos que no llevan contraseña de respaldo (los hechos antes de esta spec o antes de definirla) se restauran con la contraseña de respaldo actual del sistema; si no hay ninguna, con la del usuario (S-08).
- Contraseña incorrecta: "La contraseña de respaldo no es correcta", sin más detalle.

## 7. Requisitos no funcionales

- **Integridad:** cantidades en milésimas e importes en céntimos enteros, como hasta ahora; el redondeo hacia arriba se hace en el backend.
- **Migraciones:** numeradas y con test (`Constitution.md` §4). Ninguna puede perder datos ni dejarlos incoherentes (algunas los completan a propósito, como marcar entregadas las ventas existentes). Se prueban con una base de la versión 0.2.0 con datos.
- **Validación:** todo límite (precio mínimo, descuento, entrega, disponible, umbral, contraseña) se valida en el backend; el frontend solo adelanta el aviso.
- **Autorización:** cada comando nuevo tiene su fila en la matriz de autorización.
- **Diseño:** las pantallas nuevas siguen `ui-spec.md`.

## 8. Supuestos (confirmados el 2026-11-08; S-01, S-02 y S-14 corregidos tras la auditoría del mismo día)

- **S-01 — Fórmula del costo:** promedio ponderado móvil (CO-01): cada entrada se mezcla con el stock que había en ese momento, y anular, eliminar o editar una entrada repasa las entradas vigentes desde el costo base. Respeta "se queda el costo actual" y "anular recalcula sin la entrada". *(La primera versión promediaba con el stock al migrar y no seguía las compras recientes.)*
- **S-02 — Costo de lo vendido:** para las ganancias del dashboard, el costo de una salida es el costo vigente del producto al **entregarla** (y al crear Insumos, Garantía y Pérdida), en lugar del costo FIFO de los lotes; se imputa a la fecha de la factura (EN-04). Los lotes siguen llevando las cantidades.
- **S-03 — Mínimo al editar:** el precio de lista guardado en la línea, no el precio actual del producto.
- **S-04 — Precio del servicio libre:** sin mínimo (puede ser 0); solo los productos tienen precio mínimo.
- **S-05 — Tope del descuento:** hasta el importe bruto de la línea; no hay descuento global.
- **S-06 — Stock bajo con el disponible:** lo apartado ya no está a la venta.
- **S-07 — Valor del inventario con el stock físico:** incluye lo apartado sin entregar, porque aún está en el almacén.
- **S-08 — Respaldos anteriores:** se restauran con la contraseña de respaldo actual o, si no hay, con la del usuario.
- **S-09 — Cambiar la contraseña:** siempre con la contraseña de usuario del Administrador, no con la de respaldo anterior.
- **S-10 — Cuentas por cobrar:** Venta y Servicio Por Cobrar, en USD; Entrada Por Pagar no cuenta.
- **S-11 — Cancelar:** devuelve lo entregado y libera lo apartado.
- **S-12 — Quitar productos del servicio en la factura:** permitido; un servicio puede quedar sin productos.
- **S-13 — Eliminar un servicio:** permitido aunque esté en facturas, que conservan su copia.
- **S-14 — Insumos y Garantía asociadas:** funcionan como en `spec-03`, pero solo sobre la cantidad ya entregada de la factura asociada (EN-01).
- **S-15 — Contenido de una factura de servicio:** varios servicios y productos sueltos.
- **S-16 — Mismo producto dos veces:** puede aparecer en varios servicios y como suelto dentro de la misma factura; cada aparición es su propia línea.
- **S-17 — Precio exportado:** las exportaciones de productos llevan el precio de venta redondeado y el calculado.
