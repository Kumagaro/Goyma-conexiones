# Spec: Cierre del MVP — Notificaciones, Dashboard, Configuración, Impresión, Borradores y Respaldo

- **Feature:** `cierre-del-mvp`
- **Estado:** **Confirmada por el dueño (2026-10-28). Lista para implementar.** Recoge las respuestas de las nueve rondas de preguntas del 2026-10-28; los supuestos S-01 a S-11 de la §11 quedaron confirmados tal como están.
- **Basada en:** `Requisitos del sistema.docx` (RF-098 a RF-135 y los límites del sistema), `Constitution.md` v1.2.0 (sube a v1.3.0 con esta spec, §7.8), `Agents.md`, `spec-01`, `spec-02`, `spec-03`, `pendientes.md` (PEN-01, PEN-02, PEN-06, PEN-07, PEN-12 a PEN-16 y PEN-18 a PEN-22) y el frontend real `dashboard-unificado.html` + `dashboard.js`.
- **Convención de referencias:** `spec-01` a `spec-04` son las specs de `autenticacion-y-usuarios`, `inventario-productos`, `contactos-facturacion-historial` y `cierre-del-mvp`. `plan-04` y `task-04` están en esta misma carpeta. Las referencias internas usan `§N` o `§N.M`.
- **Siguiente paso:** ejecutar `task-04.md` por fases (§3.3).

---

## 1. Resumen

Es la última spec del MVP. Reúne todo lo que el documento de requisitos pide y las specs anteriores dejaron pendiente:

- **Mensajes como toast** (sugerencia del dueño, PEN-14): toda respuesta de la aplicación (éxito, error, aviso) se muestra como un toast con estilo y animación, en lugar del texto plano actual dentro de los formularios.
- **Notificaciones** (RF-115 a RF-124, PEN-01, PEN-02): pestaña con datos reales, contador de no leídas, avisos automáticos de stock, caducidad, descontinuados, reservas, facturas pendientes, respaldo y purga.
- **Reservas** (RF-119, RF-120, PEN-18): una venta o un servicio Por Cobrar puede tener fecha límite; avisa antes de vencer y, si vence sin cobrarse, se cancela sola.
- **Configuración** (RF-125 a RF-128, PEN-07, PEN-19): monedas visibles por usuario, tasas de cambio con historial, datos y logo de la empresa, y los umbrales de los avisos.
- **Impresión y PDF de facturas** (RF-129 a RF-132, PEN-20): vista previa A4 formal, imprimir y exportar a PDF desde el backend.
- **Conversiones en facturas** (RF-020, PEN-06): comprar o vender en otra unidad (por ejemplo, Caja) usando las conversiones existentes.
- **Dashboard** (RF-098 a RF-114, PEN-16): los KPIs del mockup con datos reales y un filtro de período.
- **Borradores** (RF-133 a RF-135, PEN-21): guardado automático de formularios y recuperación tras un cierre inesperado.
- **Registro de actividad** (PEN-13): quién eliminó, canceló, editó o configuró qué y cuándo.
- **CSV con Excel** (PEN-22): importar y exportar productos, contactos, categorías y unidades; exportar facturas.
- **Respaldo y restauración** (PEN-15): copias manuales y automáticas de toda la información y restauración, con versionado del esquema.
- **Endurecimiento del frontend** (PEN-12): sin manejadores inline ni `innerHTML` con datos, y con una política de seguridad de contenido (CSP) estricta.

## 2. Objetivo / Por qué

Con las specs 1 a 3 el negocio ya registra todo lo que compra, vende y pierde. Le falta **enterarse** de lo que pasa sin buscarlo (notificaciones y toasts), **entender** sus números (dashboard), **entregar** documentos formales (impresión y PDF), **no perder trabajo** (borradores) ni **datos** (respaldo), y saber **quién hizo qué** (registro de actividad). Debe garantizarse que:

- Todo lo que se ve sale de la base de datos vía comandos (`Constitution.md` §2.I): desaparecen los datos simulados de Notificaciones, Dashboard y Configuración del mockup.
- Los mensajes son claros, en español y no técnicos (§2.VI), y ahora además visibles y consistentes en toda la aplicación.
- Nada de esta spec necesita conexión a internet (§2.IV): la impresión usa el diálogo del sistema; el logo se guarda en la carpeta de datos de la aplicación.
- Los datos del negocio pueden sobrevivir a un cambio de equipo o de versión de la aplicación.

## 3. Alcance

### 3.1 Incluido en esta spec

Los doce bloques de la §1, con los requisitos de la §6.

### 3.2 Fuera de alcance (se registra en `pendientes.md`)

- **PEN-17** (PDF con alfabetos no latinos): el proyecto es 100 % en español; la impresión y el PDF de facturas usan la misma fuente integrada que el historial.
- **Importar facturas por archivo:** excluido por el propio documento de requisitos ("Introducción de facturas por archivo"). Las facturas solo se exportan a CSV.
- **Notificaciones fuera de la aplicación** (correo, móvil, sistema operativo): el documento excluye integraciones externas.
- **Cifrado de las copias de seguridad** (decisión del dueño): la copia es la base SQLite tal cual; las contraseñas siguen con bcrypt dentro.
- **Varias empresas o sucursales:** un solo juego de datos de empresa.
- **Gráficos nuevos:** el dashboard conserva el diseño del mockup actual (decisión del dueño).

### 3.3 Fases de implementación

Una sola spec en cinco fases que se cierran, prueban y confirman por separado (decisión del dueño):

1. **Base:** versionado del esquema y migraciones (MG), toasts (TO) y endurecimiento del frontend (SG).
2. **Notificaciones y reservas:** NT y RS.
3. **Configuración, impresión y conversiones:** CF, IM y CV.
4. **Dashboard:** DB.
5. **Datos:** borradores (BR), registro de actividad (RA), CSV y respaldo (RP).

La fase 1 va primero porque las demás añaden tablas (necesitan migraciones) y mensajes (necesitan toasts). La 2 depende de la 1. La 3 depende de la 1. La 4 usa los ajustes de la 3 (monedas). La 5 depende de todas porque registra y respalda lo que las demás crean. **La verificación manual de spec-01, spec-03 y spec-04 se hace al final de la fase 5**, en una sola pasada (decisión del dueño).

## 4. Roles y actores

Como en `spec-02` y `spec-03`, **ambos roles pueden hacer todo** salvo estas excepciones:

- **Solo el Administrador:** ver el registro de actividad (RA-02) y ver las notificaciones de gestión de usuarios (NT-02, N-13).
- **Ambos roles** (decisión del dueño, según el documento): cambiar tasas de cambio, datos y logo de la empresa, umbrales, crear copias y **restaurar** una copia.

Toda acción exige una sesión activa (`require_session`). Los procesos automáticos (generación de avisos, vencimiento de reservas, copia diaria, purgas) **no** exigen sesión: se ejecutan al arrancar la aplicación y en cada operación posterior, como la purga de `spec-02` (§7.6).

## 5. Modelo conceptual

(El esquema concreto es de `plan-04.md`.)

- **Notificación:** tipo (N-02 a N-13), prioridad (Crítico, Advertencia, Informativo), mensaje en texto plano, entidad afectada opcional (producto o factura) para abrirla, visibilidad (todos o solo Administrador), leída o no, fecha. **Una sola lista compartida** (decisión del dueño): si un usuario la marca como leída o la elimina, cambia para ambos.
- **Situación vigilada:** recuerda que un producto o factura ya generó su aviso (stock bajo, por vencer, etc.) para no repetirlo mientras siga en esa situación (NT-03).
- **Reserva:** no es un tipo de factura; es una **venta o servicio Por Cobrar con fecha límite** ("Reservar hasta").
- **Ajuste global:** umbral de stock bajo, días para "factura pendiente antigua", días para "respaldo pendiente", datos de la empresa y logo.
- **Ajuste por usuario:** monedas visibles y tamaño de página.
- **Tasa de cambio:** una por moneda (EUR, CUP, MLC, Saldo Clásica) respecto al USD, con su **historial** de cambios.
- **Unidad de la línea:** cada línea de factura guarda la unidad y la cantidad que escribió el usuario y su equivalente en la unidad del producto.
- **Borrador:** el contenido de un formulario sin guardar, por usuario y por formulario (uno como máximo por formulario nuevo y por registro en edición).
- **Entrada del registro de actividad:** fecha, usuario (con copia de su nombre), acción, entidad y una descripción en texto plano.
- **Copia de seguridad:** un archivo con toda la base y la versión de su esquema.
- **Versión del esquema:** número entero que identifica la estructura de la base; cada migración la sube en uno.

## 6. Requisitos

### Módulo: Base

#### MG-01 — Versionado del esquema y migraciones (PEN-15, prerequisito)

- **Dado** que la aplicación arranca, **entonces** lee la versión del esquema de la base y aplica en orden las migraciones pendientes, cada una en su propia transacción. Una base nueva se crea directamente en la última versión.
- La base actual de un equipo de desarrollo (creada con las specs 1 a 3) se considera la **versión 1**; las tablas y columnas de esta spec llegan como migraciones a partir de la 2.
- **Dado** que una migración falla, **entonces** la base queda en la versión anterior, el error se registra (`errors.log` si la base no está disponible) y la aplicación muestra un mensaje claro sin arrancar a medias.
- **Dado** una base con una versión **mayor** que la que conoce la aplicación (por ejemplo, una copia hecha con una versión más nueva), **entonces** no se abre y se informa al usuario.
- Cambia `Constitution.md` §4: desde esta spec el esquema se versiona y se migra (§7.8).

#### TO-01 — Toasts de respuesta (PEN-14, sugerencia del dueño)

- **Dado** cualquier acción del usuario que termina con éxito, error general o aviso, **entonces** el resultado se muestra como un **toast**: un recuadro emergente arriba a la derecha, con icono y color según el tipo, título corto y mensaje, y un botón para cerrarlo.
- **Tipos y duración** (decisión del dueño): **éxito** e **informativo** se cierran solos a los ~4 s; **advertencia** a los ~7 s; **error** y **crítico** se quedan hasta que el usuario los cierra. Pasar el ratón por encima (o tener el foco dentro) pausa el cierre.
- **Qué pasa a toast** (decisión del dueño): los resultados ("Cliente creado", "Factura guardada"), los errores generales del backend (`internal`, `notFound`, `invalidState`, `stockInsuficiente`…) y los avisos (por ejemplo, la purga fallida de `spec-02` §7.6). Los **errores de un campo concreto** (CI duplicado, fecha inválida, cantidad vacía) se muestran **además** junto al campo, para que el usuario sepa qué corregir.
- Los toasts se **apilan** hacia abajo; como máximo se ven 4 a la vez (§11, S-01) y el más antiguo de los que se cierran solos sale primero.
- **Accesibilidad:** el éxito y el aviso se anuncian sin interrumpir (`role="status"`); el error y el crítico, de inmediato (`role="alert"`). El texto se pinta como texto, nunca como HTML (A-01 de `spec-02`). Con `prefers-reduced-motion` no hay animación de desplazamiento.
- **Sustituye** a los mensajes de texto plano actuales (`.form-feedback`, `role="status"`) de todas las vistas de las specs 1 a 3, salvo los errores por campo, que se conservan junto al campo.

#### TO-02 — Toasts de notificaciones

- **Dado** que se genera una notificación del sistema (NT-02) mientras hay una sesión abierta, **entonces** aparece también como toast con su prioridad (Crítico → tipo crítico, Advertencia → advertencia, Informativo → informativo) y queda guardada en la pestaña (decisión del dueño).
- **Dado** que al iniciar sesión hay notificaciones no leídas generadas mientras la aplicación estaba cerrada, **entonces** se muestra **un solo** toast resumen ("Tienes N notificaciones sin leer") que abre la pestaña, en lugar de un toast por cada una.

#### SG-01 — Endurecimiento del frontend heredado (PEN-12)

- No queda ningún manejador inline (`onclick`, `oninput`, `onchange`, `onsubmit`…) en `dashboard-unificado.html` ni HTML construido con `innerHTML` que interpole datos (todo con DOM y `textContent`).
- Los `style=` inline del mockup se sustituyen por clases cuando sea necesario para la CSP.
- La aplicación activa una **CSP estricta** (sin `unsafe-inline` ni `unsafe-eval` para scripts) y sigue funcionando entera.
- No se cambia el aspecto visual (Constitution §2.V): el endurecimiento es invisible para el usuario.

### Módulo: Notificaciones

#### NT-01 — Pestaña de Notificaciones (RF-115, RF-124, PEN-01)

- La pestaña del mockup se conserva (decisión del dueño): "Todas" / "No leídas", buscador por texto y filtro por fecha, ahora con datos reales.
- Cada notificación muestra su prioridad (Crítico, Advertencia, Informativo), el mensaje y la fecha y hora en que se generó.
- **Contador de no leídas** en la cabecera (`#topbar-notif-badge`) y en la pestaña; se actualiza al instante al leer, eliminar o recibir notificaciones.
- **Leída** (decisión del dueño): al hacer clic en una notificación se marca como leída; hay un botón "Marcar todas como leídas".
- **Eliminar** una o todas, **sin confirmación** (RF-124). "Eliminar todas" elimina las que el usuario ve con los filtros actuales.
- **Conservación** (decisión del dueño): una notificación se borra sola a los **90 días** de generarse.
- El Contador no ve las notificaciones de gestión de usuarios (N-13).

#### NT-02 — Notificaciones automáticas (RF-116 a RF-123, PEN-02)

| Código | Cuándo | Mensaje | Prioridad |
|---|---|---|---|
| N-02 | Un producto activo queda con cantidad 0, o se intenta guardar una salida de un producto agotado (RF-116) | "Producto agotado: [nombre]" | Crítico |
| N-03 | Un producto activo queda con 0 < cantidad < **umbral de stock bajo** (CF-04; por defecto 5) | "Producto con stock bajo: [nombre] - Cantidad actual: [X] [unidad]" | Advertencia |
| N-04 | La caducidad de un producto activo está entre hoy y hoy + 29 días (RF-118, mismo umbral que el filtro de `spec-02` P-12) | "Producto próximo a vencer: [nombre] - Caducidad: [fecha] - Días restantes: [X] días" | Crítico |
| N-05 | A una reserva le quedan menos de 3 días para su fecha límite y no se ha cobrado (RF-119) | "Reserva por vencerse: Factura # [número] - Cliente: [nombre] - Días restantes: [X] días" | Advertencia |
| N-06 | Una reserva se cancela sola por vencimiento (RF-120) | "Factura de reserva cancelada: Factura # [número] - Cliente: [nombre] - Motivo: Tiempo límite excedido" | Informativo |
| N-07 | A un descontinuado le quedan menos de 10 días para la eliminación automática (RF-121) | "Producto descontinuado próximo a eliminar: [nombre] - Días restantes para eliminación: [X] días" | Advertencia |
| N-08, N-09 | Ya existen (`spec-02`): productos sin unidad o sin categoría al eliminar una unidad o categoría | Sin cambios | Advertencia |
| N-10 | Una compra Por Pagar o una venta/servicio Por Cobrar supera los **días de factura pendiente antigua** (CF-04; por defecto 30) desde su emisión | "Factura pendiente antigua: [número] - [proveedor o cliente] - [X] días sin [pagar/cobrar]" | Advertencia |
| N-11 | Pasan más de los **días de respaldo pendiente** (CF-04; por defecto 7) sin una copia manual ni automática correcta | "No se ha hecho una copia de seguridad en [X] días" | Advertencia |
| N-12 | La purga automática de `spec-02` P-08 elimina productos descontinuados | "Se eliminaron automáticamente estos productos descontinuados: [lista]" | Informativo |
| N-13 | Un usuario se crea, se edita o se elimina (solo Administrador) | "Usuario [creado/editado/eliminado]: [nombre de usuario]" | Informativo |

- Los avisos de situación (N-02, N-03, N-04, N-05, N-07, N-10, N-11) se evalúan **al arrancar** la aplicación y **tras cada operación** que pueda cambiar la situación (por ejemplo, N-03 tras una venta; N-04 tras editar la caducidad).
- N-02 "al intentar vender" se genera aunque la venta se rechace por falta de stock (`stockInsuficiente`); se rige por NT-03.
- Los mensajes se guardan como texto plano estructurado (PEN-14) y se pintan con `textContent`.

#### NT-03 — Una vez por situación (decisión del dueño)

- **Dado** que un producto o factura entra en una situación (stock bajo, agotado, por vencer, reserva por vencerse, descontinuado por eliminarse, pendiente antigua) o el respaldo queda pendiente, **entonces** se genera **una** notificación.
- Mientras siga en esa situación **no se repite**, aunque haya más operaciones o reinicios.
- **Dado** que sale de la situación (por ejemplo, se repone el stock) y luego vuelve a entrar, **entonces** se avisa de nuevo.
- Agotado (N-02) y stock bajo (N-03) son situaciones distintas: pasar de stock bajo a agotado genera N-02.

#### NT-04 — Abrir lo afectado (decisión del dueño)

- **Dado** una notificación de un producto o de una factura, **cuando** el usuario hace clic, **entonces** se marca como leída y se abre el detalle del producto o de la factura.
- **Dado** que lo afectado ya no existe, **entonces** solo se marca como leída y un toast informativo lo dice ("El producto ya no existe").

### Módulo: Reservas

#### RS-01 — Reservar una venta o un servicio (RF-119, PEN-18)

- **Dado** una factura de venta o de servicio en estado Por Cobrar (al crearla o al editarla), **entonces** el usuario puede indicar una fecha **"Reservar hasta"** (opcional, decisión del dueño). Sin fecha, es una factura normal.
- La fecha debe ser **posterior a la fecha de emisión** y no más de 365 días después (§11, S-07).
- Solo venta y servicio: garantía e insumos no se cobran (decisión del dueño).
- **Stock** (decisión del dueño): la reserva consume los lotes por FIFO al guardarse, igual que cualquier venta Por Cobrar (`spec-03` LT-02). No hay "stock apartado" separado.
- **Dado** que la reserva se cobra (Cobrada) o se cancela, **entonces** deja de ser una reserva activa; la fecha límite se conserva solo como dato histórico.
- Quitar la fecha al editar la convierte en una venta normal.

#### RS-02 — Vencimiento automático (RF-120)

- **Dado** una reserva activa cuya fecha límite ya pasó (el día siguiente a "Reservar hasta", en hora local), **entonces** la factura pasa sola a **Cancelada**, sus consumos vuelven a los lotes y se registra la Reversión, exactamente como una cancelación manual (`spec-03` FS-01, LT-04); se genera N-06 y una entrada del registro de actividad con el usuario "Sistema".
- Se comprueba al arrancar la aplicación y antes de cada operación posterior (§11, S-04). Si la aplicación estuvo cerrada, al abrirse cancela todas las vencidas.
- **Excepción documentada a `spec-03` §3.2** ("las facturas no cambian de estado por el paso del tiempo"): solo las reservas vencen; el resto de facturas no cambia.

#### RS-03 — Ver las reservas y los productos reservados ("mostrar productos reservados")

- En los listados de ventas y de servicios, el filtro de estado gana la opción **"Reservas activas"**; las reservas muestran su fecha límite en una columna.
- El detalle del producto muestra la **cantidad reservada** (suma de sus líneas en reservas activas) con enlace a esas facturas.
- El dashboard tiene una tarjeta de **reservas activas** con las que vencen en los próximos 3 días (DB-07).
- El detalle de la factura muestra "Reservada hasta [fecha]".

### Módulo: Configuración

#### CF-01 — Monedas visibles por usuario (RF-125, PEN-07)

- Cada usuario elige qué monedas adicionales al USD ve: EUR, CUP, MLC y Saldo Clásica (o ninguna). La elección se guarda **por usuario**.
- Los equivalentes se muestran en el **detalle del producto** (costo, precio de venta, valor total) y en el **dashboard** (ingresos, ganancias, gastos, valor del inventario) (decisión del dueño). Las facturas, los listados y los PDF siguen solo en USD.
- Se calculan con la **tasa actual** (decisión del dueño) y se redondean a dos decimales. Una moneda sin tasa configurada no se muestra y un aviso lo indica en Configuración.
- Todo se sigue guardando en centavos de USD (`spec-02` §7.4); las conversiones son solo de presentación.

#### CF-02 — Tasas de cambio e historial (RF-126, RF-127)

- Cualquier usuario puede fijar la tasa de cada moneda respecto al USD (por ejemplo, 1 USD = 670 CUP), con hasta 6 decimales y mayor que 0.
- Cada cambio queda en el **historial**: fecha y hora, moneda, tasa anterior, tasa nueva y usuario. El historial se ve en Configuración (orden por fecha alternable) y no se edita ni se borra.
- Las tarjetas de tasas del mockup (`#rate-cards`) y el historial (`#historial-tasas-body`) muestran datos reales.

#### CF-03 — Datos y logo de la empresa (RF-128, PEN-19)

- Cualquier usuario puede editar el **nombre de la empresa**, la **dirección**, el **teléfono**, el **NIT o código fiscal** y el **logo** (decisión del dueño).
- El logo es una imagen **PNG o JPG** de hasta 2 MB, que se valida por su contenido (no solo la extensión), se copia a la carpeta de datos de la aplicación y sustituye al anterior (que se borra). Se puede quitar.
- Todos estos datos encabezan las facturas impresas y exportadas (IM-01). Si faltan, el encabezado se compone con lo que haya.

#### CF-04 — Umbrales y preferencias

- **Globales** (en la base, para ambos usuarios): umbral de **stock bajo** (por defecto 5, entero de 1 a 9 999; también cambia el filtro "Stock bajo" de `spec-02` P-12 y la tarjeta de DASH-01), días de **factura pendiente antigua** (por defecto 30, de 1 a 365) y días de **respaldo pendiente** (por defecto 7, de 1 a 90).
- **Por usuario:** monedas visibles (CF-01) y **tamaño de página** (10, 20, 50 o 100), que hoy se guarda en el navegador y pasa a la base.
- Cambiar un umbral reevalúa las situaciones de NT-03 con el valor nuevo.
- La sección "Datos de Factura" del mockup (casillas de qué campos mostrar por tipo de factura) se trata en la §11, S-06.

### Módulo: Impresión de facturas

#### IM-01 — Imprimir con vista previa (RF-129, RF-131, RF-132, PEN-20)

- El detalle de **los seis tipos** de factura tiene un botón **"Imprimir"**, disponible siempre, en cualquier estado (RF-131).
- Abre una **vista previa A4 formal** dentro de la aplicación y, desde ella, el diálogo de impresión del sistema (que permite elegir impresora o "Guardar como PDF") (decisión del dueño).
- **Contenido:** encabezado con logo y datos de la empresa (CF-03); título según el tipo ("Factura de Compra", "Factura de Venta", "Factura de Servicio", "Factura de Garantía", "Factura de Insumos", "Factura de Pérdida"); número y fecha; datos del cliente o proveedor (la copia guardada); líneas con cantidad, unidad, precio o costo unitario y subtotal; total; descripción u observaciones si las tiene; factura asociada en garantía e insumos; "Reservada hasta" en las reservas. **No muestra el estado** (RF-129). Las cantidades se muestran en la unidad escrita en la línea (CV-01).
- Se imprime con los datos guardados de la factura (RF-131): reimprimir no recalcula nada.

#### IM-02 — Exportar a PDF (RF-130)

- El botón **"Exportar a PDF"** genera, **desde el backend**, un PDF con el **mismo diseño** de IM-01 y lo guarda donde el usuario elija con el diálogo "Guardar como", como la exportación del historial (`spec-03` HP-03) (decisión del dueño).
- Sustituye a la exportación actual con jsPDF (`exportarFactura*PDF`), que se retira junto con la librería.
- Si el usuario cancela el diálogo, no pasa nada; si falla la escritura, un toast lo explica y el error se registra.

### Módulo: Conversiones en facturas

#### CV-01 — Unidad por línea (RF-020, PEN-06)

- En las facturas de entrada, venta, servicio, garantía, insumos y pérdida, cada línea tiene un **selector de unidad** con la unidad del producto y las que tienen una conversión con ella (`spec-02` UM-06, camino directo o inverso de un paso; §11, S-08).
- La cantidad se escribe en la unidad elegida; el sistema calcula su **equivalente en la unidad del producto**, redondeado a 3 decimales (mitad hacia arriba), que es lo que mueve el inventario, el FIFO y el historial. **El inventario siempre va en la unidad del producto.**
- Los precios de la línea siguen las mismas reglas de `spec-03`, convertidos: en una compra, el costo unitario se escribe **por la unidad elegida** y se divide entre el factor para el lote; en una venta, el precio unitario por la unidad elegida es el precio vigente × el factor.
- La línea guarda la unidad elegida, la cantidad escrita y su equivalente; el detalle y la impresión muestran lo escrito ("2 Caja"), y el historial lo que se movió ("24 u").
- Un producto sin unidad no ofrece conversiones. Si una conversión se elimina después, las líneas ya guardadas conservan su copia.

### Módulo: Dashboard

El dashboard **conserva el diseño del mockup** (decisión del dueño): tarjetas principales, secundarias, gráfico de ingresos, resumen de gastos, estrellas, clientes frecuentes, resumen de inventario, más vendidos y lento movimiento.

#### DB-01 — Período y actualización (RF-098, RF-099)

- **Un selector de período global** (decisión del dueño) aplica a todos los KPIs con período: **Hoy, Semana actual, Mes actual, 3 meses, 6 meses, 1 año** y **rango personalizado** (fecha inicial y final, día local, ambos incluidos). El mockup tiene hoy, 7 días, mes y año: se ajusta a estas opciones.
- Los KPIs se recalculan al abrir el dashboard y cada vez que se vuelve a él después de guardar algo (decisión del dueño). No hay consultas periódicas.
- Un fallo muestra el estado de error del mockup (`#dash-error`) con "Reintentar".

#### DB-02 — Qué es un ingreso, un costo y un gasto

- **Ingreso** (decisión del dueño): el total de las ventas y servicios **Cobrados**, ubicados en el período por su **fecha de cobro** (decisión del dueño). Por Cobrar no suma hasta cobrarse.
- **Costo de lo vendido:** el costo real FIFO de esas mismas líneas, es decir, la suma de sus consumos de lotes (cantidad × costo del lote, `spec-03` LT-02). No se usa `costAtSaleCents`, que es el costo promedio del producto en el momento de la venta.
- **Gastos:** garantías e insumos **Completados** y pérdidas, a su costo real, ubicados por su **fecha de emisión** (§11, S-05).
- **Ganancia neta** (decisión del dueño, RF-111): (ingresos − costo de lo vendido) − gastos.

#### DB-03 — Tarjetas principales y secundarias (RF-100 a RF-105, RF-107, RF-111)

- **Ingresos totales** (RF-105), **cantidad de ventas** (número de ventas Cobradas, RF-102), **promedio de ventas diarias** (cantidad y monto entre los días del período, RF-107) y **ganancia neta** (RF-111).
- **Margen de ganancia** (RF-104): margen total (ingresos − costo de lo vendido) y margen promedio (promedio de los % de ganancia de los productos activos).
- **Total de gastos** (RF-110), **valor del inventario** (suma de cantidad restante × costo de cada lote de los productos activos, igual que "Valor Total" de `spec-03` SP-03, RF-101) y **productos agotados** (RF-103, ya existe en DASH-01). La cantidad de productos en inventario (RF-100) es la suma de cantidades de los activos y se muestra en "Resumen de Inventario".
- Las cifras en USD muestran además las monedas del usuario (CF-01).

#### DB-04 — Ingresos por tipo y gastos por tipo (RF-109, RF-110)

- El gráfico de ingresos del mockup muestra los ingresos por día (o por semana o mes según la longitud del período) y el reparto **venta / servicio** con su porcentaje.
- "Resumen de Gastos" muestra garantía, insumos y su total con el porcentaje de cada uno; las pérdidas aparecen como línea aparte (DB-02).

#### DB-05 — Rankings (RF-106, RF-108, RF-112, RF-113)

- **Más vendidos (Top 10):** por unidades vendidas en ventas y servicios Cobrados del período; con nombre, cantidad y margen individual.
- **Clientes frecuentes (Top 10):** por número de facturas Cobradas del período; con nombre (la copia guardada), número de compras y monto total.
- **Productos estrella (Pareto 80/20):** por **ganancia real** (ingreso − costo FIFO) en el período, ordenados de mayor a menor, hasta acumular el 80 % (decisión del dueño). **Clientes estrella:** por monto cobrado, igual.
- Un producto o cliente eliminado sigue apareciendo con su nombre guardado.

#### DB-06 — Lento movimiento (RF-114)

- Todos los productos activos ordenados de menor a mayor cantidad vendida en el período, con nombre, cantidad vendida, precio de venta y **días sin venderse** (desde su última venta Cobrada; si nunca se vendió, desde que se creó; §11, S-09).

#### DB-07 — Reservas activas (RS-03)

- Tarjeta con el número de reservas activas, su monto y las que vencen en los próximos 3 días, con enlace a cada una.

### Módulo: Borradores

#### BR-01 — Guardado automático (RF-133, PEN-21)

- **Todos los formularios salvo los de sesión** (decisión del dueño): productos, categorías, unidades, conversiones, clientes, proveedores, los seis tipos de factura, usuarios, perfil y configuración (incluidos los diálogos de alta rápida).
- El contenido se guarda en la base **mientras el usuario escribe** (como mucho, un segundo después de la última tecla) como borrador de ese usuario y ese formulario.
- **Nunca se guardan contraseñas** ni campos de contraseña, ni archivos (la ruta del PDF adjunto o del logo no se guarda en el borrador).
- **Uno por formulario y usuario** (decisión del dueño): un borrador de "nueva venta", uno de "nuevo producto"… y uno por cada registro que se esté editando.
- Al **guardar** o **cancelar** el formulario, su borrador se borra.

#### BR-02 — Recuperar (RF-134)

- **Al iniciar la aplicación** (decisión del dueño), si el usuario tiene borradores, un toast informativo dice "Tienes N formularios incompletos" con un enlace a Borradores.
- **Al abrir un formulario** con borrador, un diálogo pregunta "Tienes un formulario incompleto. ¿Deseas continuar?" con **Continuar** (carga el borrador) y **Descartar** (lo borra).
- Al continuar una factura, se vuelven a validar contacto y productos: si alguno ya no existe, se avisa y se quita.

#### BR-03 — Sección "Borradores" y caducidad (RF-135)

- Una sección lista los borradores del usuario con el formulario, el registro (si es una edición) y la fecha de su último cambio; cada uno se puede **abrir** o **eliminar**.
- **Caducidad** (decisión del dueño): un borrador sin cambios en **30 días** se borra solo.

### Módulo: Registro de actividad

#### RA-01 — Qué se registra (PEN-13)

Decisión del dueño: cada una de estas acciones queda registrada con fecha y hora, usuario (con copia de su nombre; "Sistema" para los procesos automáticos), acción y descripción:

- **Eliminaciones y cancelaciones:** productos (incluida la purga automática), categorías, unidades, conversiones, clientes, proveedores, usuarios, facturas eliminadas y facturas canceladas (incluidas las reservas vencidas).
- **Ediciones de facturas:** qué factura se editó y quién (hoy solo se guarda la fecha).
- **Cambios de configuración:** tasas, datos y logo de la empresa, umbrales.
- **Respaldos, restauraciones e importaciones CSV:** con el archivo (solo el nombre) y el resumen del resultado.

La descripción no contiene contraseñas, hashes ni datos personales de contacto (teléfono, CI, correo): solo nombres y números de documento (`Constitution.md` §2.VI).

#### RA-02 — Consultar el registro (solo Administrador)

- Pantalla **"Registro de actividad"** solo para el Administrador, con filtros de fecha, usuario y tipo de acción, búsqueda por texto y orden por fecha alternable.
- El registro es **inmutable** (no se edita ni se borra desde la aplicación) y **no se purga** (§11, S-02).

### Módulo: CSV

#### CSV-01 — Exportar (PEN-22)

- Exportar a CSV desde los listados de **productos, categorías, unidades, clientes, proveedores** y de **facturas** (cada tipo y la lista general), respetando los filtros activos. Se guarda con el diálogo "Guardar como".
- **Formato** (decisión del dueño): Excel en español: separador punto y coma, decimales con coma, fechas `dd/mm/aaaa`, codificación UTF-8 con BOM, primera fila con los encabezados en español.
- Los textos que empiezan por `=`, `+`, `-` o `@` se escapan para que Excel no los ejecute como fórmula.

#### CSV-02 — Importar (PEN-22)

- Importar desde un CSV con el mismo formato: **productos, categorías, unidades, clientes y proveedores** (las facturas no se importan; el documento lo excluye).
- **Productos:** nombre, descripción, categoría, unidad, costo, % de ganancia, cantidad inicial y caducidad. Cada producto importado se crea como con el formulario (`spec-03` SP-01: lote inicial y movimiento Inicial). La **categoría o unidad que no exista se crea sobre la marcha** (decisión del dueño).
- **Validación:** cada fila se valida con las mismas reglas que el formulario correspondiente (backend autoritativo).
- **Duplicados y errores** (decisión del dueño): la fila que choca con un registro existente (mismo nombre normalizado o mismo CI) o que no es válida **se omite**; las válidas se importan. Al final, un resumen dice cuántas se importaron y cuántas se omitieron, con el número de fila y el motivo de cada una.
- Un archivo vacío, con encabezados desconocidos o de más de 5 000 filas se rechaza completo con un mensaje claro (§11, S-10).

### Módulo: Respaldo y restauración

#### RP-01 — Copia manual (PEN-15)

- Botón **"Crear copia de seguridad"** en Configuración: guarda toda la base en un archivo donde el usuario elija ("Guardar como"), con la fecha en el nombre propuesto (`goyma-AAAA-MM-DD.sqlite3`).
- La copia es consistente aunque haya operaciones en curso.
- **Sin cifrado** (decisión del dueño). Un aviso en pantalla recuerda que el archivo contiene todos los datos del negocio.

#### RP-02 — Copia automática

- Una vez al día, al **primer arranque** del día (§11, S-03), se crea una copia en la carpeta de datos de la aplicación; se conservan las **7 más recientes** (decisión del dueño).
- Si falla, se registra y la aplicación sigue; el aviso N-11 sale cuando se cumplen los días configurados.
- Configuración lista las copias automáticas disponibles (fecha y tamaño) para restaurarlas.

#### RP-03 — Restaurar

- Desde Configuración, se elige un archivo de copia (o una copia automática) y se **reemplaza toda la base** (decisión del dueño). Pueden hacerlo **ambos roles**.
- Antes de restaurar: se confirma con la **contraseña del usuario** y se crea automáticamente una copia de lo actual.
- La copia se valida: debe ser una base de Goyma Conexiones con una versión de esquema igual o menor que la de la aplicación; si es menor, se migra (MG-01). Un archivo inválido se rechaza sin tocar la base actual.
- Tras restaurar, se **cierra la sesión** y se vuelve al login (las sesiones guardadas en la copia se invalidan).
- Los adjuntos PDF y el logo viven en la carpeta de datos, no en la base: una copia **no los incluye** (§11, S-11).

## 7. Reglas de negocio transversales

### 7.1 Procesos automáticos

- Al arrancar, en este orden: migraciones (MG-01), copia automática del día (RP-02), purga de vencidos (`spec-02` P-08), vencimiento de reservas (RS-02), evaluación de avisos (NT-02), purga de notificaciones de más de 90 días (NT-01) y de borradores de más de 30 (BR-03).
- Antes de cada operación posterior se repiten, de forma best-effort como la purga de `spec-02` §7.6, la purga de vencidos y el vencimiento de reservas; un fallo de estos procesos nunca impide la operación del usuario: se registra y se avisa con un toast.

### 7.2 Toasts y mensajes

- Todo mensaje al usuario sigue `Constitution.md` §2.VI: específico cuando debe corregir algo, genérico ante errores internos, nunca técnico.
- Los mensajes de éxito usan verbos en pasado y nombran lo afectado ("Factura V-00012 guardada").

### 7.3 Fechas y horas

- Como en `spec-03` §7.3: los días se cuentan en hora local; las fechas y horas se guardan en UTC.

### 7.4 Dinero, cantidades y monedas

- Igual que `spec-02` §7.4 y `spec-03` §7.2. Las monedas distintas del USD son solo de presentación (CF-01): nunca se guardan importes en otra moneda.

### 7.5 Integridad

- Cada operación que cambie inventario, estado de facturas o configuración, junto con su notificación y su entrada del registro de actividad, ocurre en una sola transacción: si algo falla, no queda nada a medias.

### 7.6 Concurrencia

- Sigue habiendo una sola sesión activa a la vez (`spec-01`). Los procesos automáticos y la restauración toman la misma conexión exclusiva que los comandos.

### 7.7 Privacidad

- Los borradores, el registro de actividad, las notificaciones y los CSV exportados contienen datos del negocio: viven en la base o donde el usuario los guarde. Ningún registro de errores contiene datos ingresados por el usuario (§2.VI).

### 7.8 Cambio de `Constitution.md` (v1.3.0)

- §4 "Base de datos": el esquema se **versiona** desde esta spec; cada cambio de esquema es una **migración** numerada, probada y aplicada en orden al arrancar; una base de una versión mayor no se abre. Se registra en el historial de versiones de la Constitution junto con el motivo (respaldo y migración de datos reales, PEN-15).

## 8. Errores y mensajes

### 8.1 Registro

- Igual que `spec-03` §8.1: los rechazos esperados (`validationFailed`, `invalidCredentials`) no se registran; el resto sí, sin datos del usuario.

### 8.2 Mensajes nuevos (el `plan-04.md` fija códigos y textos exactos)

- Migración fallida o base de una versión más nueva.
- Copia o restauración fallida; archivo de copia no válido; contraseña incorrecta al restaurar.
- Logo no válido o demasiado grande.
- Tasa no válida; umbral fuera de rango.
- Fecha "Reservar hasta" no válida.
- CSV vacío, con encabezados desconocidos, demasiado grande o con filas omitidas (resumen).
- Conversión inexistente para la unidad elegida.

## 9. Impacto en la interfaz existente

- **Todas las vistas** de las specs 1 a 3 cambian sus mensajes de respuesta a toast (TO-01). Es un cambio visual transversal y deliberado (Constitution §2.V): se documenta en `plan-04` y no altera el resto del diseño.
- **Notificaciones, Dashboard y Configuración** del mockup pierden sus datos simulados de `dashboard.js` y pasan a vistas propias.
- **Facturas:** campo "Reservar hasta" en venta y servicio, filtro "Reservas activas", selector de unidad por línea, botones "Imprimir" y "Exportar a PDF".
- **Productos:** cantidad reservada en el detalle.
- **Nuevas secciones:** Borradores y Registro de actividad (esta solo para el Administrador).
- **Configuración:** pestañas actuales (Facturación, Moneda, Tasas, General) con datos reales, más Umbrales y Respaldo.
- **Endurecimiento** (SG-01): sin cambios visibles.

## 10. Límites aceptados

- **Una sola lista de notificaciones compartida** (decisión del dueño): si un usuario la lee o la elimina, el otro tampoco la ve como pendiente.
- **El registro de actividad crece sin límite** (como el historial de movimientos de `spec-03`).
- **Las copias no incluyen los adjuntos PDF ni el logo** (§11, S-11).
- **Los equivalentes en otras monedas usan la tasa actual**, también para datos pasados del dashboard (decisión del dueño).
- **Una reserva consume el stock** como cualquier venta Por Cobrar; no existe "stock apartado".

## 11. Supuestos (confirmados por el dueño el 2026-10-28)

Se aplican tal como están en la columna "Supuesto"; la alternativa queda solo como registro.

| # | Supuesto | Alternativa |
|---|---|---|
| S-01 | Los toasts usan la paleta de `dashboard.css` (verde éxito, azul informativo, ámbar advertencia, rojo error y crítico) y se ven como máximo 4 a la vez. | Otra paleta o sin límite. |
| S-02 | El registro de actividad no se purga. | Purgarlo tras N meses. |
| S-03 | La copia automática se hace al primer arranque de cada día. | A una hora fija mientras la app está abierta. |
| S-04 | Las reservas vencidas se cancelan al arrancar y antes de cada operación (no con un temporizador en segundo plano). | Además, cada N minutos con la app abierta. |
| S-05 | Gastos (garantía, insumos, pérdidas) se ubican en el período por su fecha de emisión, porque no tienen fecha de pago. | Guardar una fecha de completado. |
| S-06 | La sección "Datos de Factura" del mockup (casillas por tipo de factura) se **elimina**: el diseño de impresión lo fija IM-01 y RF-129 excluye el estado. | Conservarla para elegir qué campos se imprimen. |
| S-07 | "Reservar hasta" admite como máximo 365 días después de la emisión. | Otro máximo o sin máximo. |
| S-08 | El selector de unidad ofrece solo conversiones directas o inversas de un paso (como el Conversor de `spec-02`). | Encadenar conversiones. |
| S-09 | En lento movimiento, un producto que nunca se vendió cuenta sus días sin venderse desde que se creó. | Mostrar "Nunca". |
| S-10 | El CSV admite como máximo 5 000 filas por archivo. | Otro máximo. |
| S-11 | La copia de seguridad es solo la base; los adjuntos y el logo no se incluyen (se pueden copiar a mano desde la carpeta de datos). | Empaquetar base + archivos en un `.zip`. |

## 12. Trazabilidad con el documento de requisitos

| Requisito | Dónde |
|---|---|
| RF-020 (conversiones en compras y ventas) | CV-01 |
| RF-098, RF-099 | DB-01 |
| RF-100 a RF-105, RF-107, RF-111 | DB-02, DB-03 |
| RF-106, RF-108, RF-112, RF-113 | DB-05 |
| RF-109, RF-110 | DB-04 |
| RF-114 | DB-06 |
| RF-115, RF-124 | NT-01 |
| RF-116 a RF-123 | NT-02, NT-03 |
| RF-119, RF-120 | RS-01, RS-02 |
| RF-125 | CF-01 |
| RF-126, RF-127 | CF-02 |
| RF-128 | CF-03 |
| RF-129, RF-131, RF-132 | IM-01 |
| RF-130 | IM-02 |
| RF-133 | BR-01 |
| RF-134 | BR-02 |
| RF-135 | BR-03 |
| Límites: "Mostrar productos reservados" | RS-03, DB-07 |
| Límites: "Mostrar compras pendientes" | NT-02 (N-10) y los filtros de estado de `spec-03` |
| Límites: "Excel (importación y exportación en CSV)" | CSV-01, CSV-02 |
| PEN-12, PEN-13, PEN-14, PEN-15 | SG-01, RA-01/RA-02, TO-01/TO-02, MG-01 + RP-01 a RP-03 |
