# Spec: Categorías, Unidades de Medida y Productos

- **Feature:** `inventario-productos`
- **Estado:** **Confirmada.** Las decisiones del dueño están incorporadas y los supuestos S-1 a S-12 (sección 11) están confirmados. La revisión de la interfaz (PEN-03) está resuelta en `frontend-cambios.md` (sección 9).
- **Basada en:** `Requisitos_del_sistema.docx` (RF-010 a RF-031), `Constitution.md` v1.2.0, `Agents.md`, `specs/autenticacion-y-usuarios/spec.md`, `frontend-cambios.md`
- **Siguiente paso:** `plan.md` → `tasks.md`
- **Revisión posterior (auditoría de `conversacion.md` y revisión QA):** incorpora la cantidad inicial obligatoria al crear un producto, el rechazo de caracteres de control y de formato en los textos (7.3), un mensaje propio para el desborde del cálculo del Conversor (UM-07), las reglas de interfaz de la sección 9 (salida segura, pausa en búsquedas y bloqueo del doble envío), la purga best-effort con aviso visible, las columnas de auditoría (`createdAt`/`updatedAt`) y los pendientes PEN-11 a PEN-14. Una segunda revisión QA (Q-01 a Q-12) añadió el plegado parcial de acentos como límite documentado, la regla de doble coincidencia exacta en unidades, el aviso de purga sin cifra y las reglas de teclado de la interfaz. Las decisiones nuevas están confirmadas por el dueño (las de la segunda revisión, por delegación suya).

---

## 1. Resumen

Esta spec cubre tres módulos que se implementan juntos porque Producto depende de los otros dos y comparten flujos (crear sobre la marcha, eliminación con efecto sobre productos):

- **Categorías de productos** (RF-010 a RF-014), con su vista de detalle (CAT-06).
- **Unidades de medida**, incluida la gestión de conversiones entre ellas y el Conversor Rápido (RF-015 a RF-020, UM-07).
- **Productos** (RF-021 a RF-031): alta, detalle, edición, descontinuación, reactivación, eliminación manual y automática, listados, promedio ponderado del costo y precio de venta.

Incluye además:

- Una tabla mínima de notificaciones, solo para registrar los avisos que producen las eliminaciones de categorías y unidades (sección 3.2).
- La tarjeta de productos agotados del dashboard con datos reales (DASH-01).

## 2. Objetivo / Por qué

El negocio necesita saber qué productos tiene, cuánto le cuestan y a qué precio los vende. Esta spec crea el catálogo sobre el que se apoyarán después las facturas, el historial y los indicadores. Debe garantizarse que:

- Todo lo que se ve proviene de la base de datos (`Constitution.md` §2.I): desaparecen los datos simulados de Categorías, Unidades, Conversiones y Productos, y de la tarjeta de agotados del dashboard. Los KPIs, "Mejor Vendidos" y "Lento Movimiento" dependen de ventas que aún no existen y se muestran en estado vacío, nunca con datos inventados.
- El costo de un producto refleja sus compras reales (promedio ponderado) y el precio de venta se deriva siempre del porcentaje de ganancia que decide el usuario.
- Eliminar una categoría o una unidad nunca deja información inconsistente: los productos afectados quedan sin ella y el usuario es informado.

## 3. Alcance

### 3.1 Incluido en esta spec

- CRUD de categorías, con creación sobre la marcha desde el formulario de producto, y detalle de categoría (CAT-06).
- CRUD de unidades de medida, con creación sobre la marcha, CRUD de conversiones entre unidades y Conversor Rápido (UM-07).
- Alta, detalle, edición, descontinuación, reactivación, eliminación manual y eliminación automática de productos.
- Listados: productos activos, y una pestaña de agotados y descontinuados, con búsqueda y filtros.
- Cálculo del precio de venta a partir del % de ganancia y función del promedio ponderado del costo, con sus tests.
- Tabla `notifications` mínima y generación de las notificaciones N-08 (productos sin unidad de medida) y N-09 (productos sin categoría).
- Tarjeta de productos agotados del dashboard y sus botones "Ver Agotados" y "Ver Descontinuados" (DASH-01).
- Reemplazo de los datos simulados de estas secciones por datos reales, vía comandos Tauri.

### 3.2 Fuera de alcance de esta spec (registrado en la sección 10 y en `specs/pendientes.md`)

- La pestaña de Notificaciones (lista, contador, lectura, eliminación) y las notificaciones N-02, N-03, N-04 y N-07. Aquí las notificaciones N-08 y N-09 **se generan y se guardan**, pero el usuario aún no las ve en una pestaña hasta que se especifique el módulo de Notificaciones.
- Cualquier modificación de la cantidad o del costo de un producto **después de crearlo**: solo cambian mediante facturas de entrada (spec futura). En esta spec la cantidad se fija una única vez, al crear el producto (cantidad inicial, P-01), y no se modifica después.
- Aplicar las conversiones a productos, compras o ventas. (El Conversor Rápido, UM-07, las usa solo para calcular y mostrar un resultado; no modifica ningún dato.)
- Monedas distintas del USD y tasas de cambio (Configuración).
- Facturas, historial y su forma de conservar la información de un producto eliminado.
- Historial de producto y sección "Facturas Asociadas": quedan ocultos hasta que existan sus specs (PEN-09).
- Columnas "Valor Total" (productos) e "Ingresos" (categorías), KPIs, "Mejor Vendidos" y "Lento Movimiento" del dashboard: ocultas o en estado vacío y sin datos hasta la spec de Facturas de venta (PEN-10).

## 4. Roles y actores

**Ambos roles, Administrador y Contador, pueden realizar todas las acciones de esta spec.** Los requisitos originales dicen "la contadora" porque describen al usuario principal del negocio; el sistema solo define los dos roles de `autenticacion-y-usuarios`. Toda acción exige una sesión activa y válida (`require_session`); no hay acciones exclusivas del Administrador. Cada comando nuevo —incluidos la vista previa del precio de venta, el Conversor Rápido, el resumen del dashboard y la eliminación de producto— tiene su fila en la matriz de tests de autorización (`Agents.md` §3): sin sesión → rechazado; con sesión de cualquiera de los dos roles → permitido.

## 5. Modelo conceptual

(Conceptual — el esquema concreto corresponde a `plan.md`.)

**Identificadores:** todas las tablas (categorías, unidades, conversiones, productos y notificaciones) usan un ID entero autoincremental (`AUTOINCREMENT`) que nunca se reutiliza, ni siquiera tras eliminar el registro. El relleno de ceros que muestra la interfaz (`#00001`) es solo de presentación.

**Categoría:** nombre (obligatorio, único) y descripción (opcional).

**Unidad de medida:** nombre (obligatorio, único) y abreviación (obligatoria, única).

**Conversión entre unidades:** dos unidades distintas y un factor decimal mayor que 0. "1 Caja = 12 Unidades" es una sola conversión y sirve en ambos sentidos (1 Unidad = 1/12 Caja). Entre dos unidades existe a lo sumo **un camino** de conversiones, directo o a través de otras unidades (S-4).

**Producto:**

| Campo | Regla |
|---|---|
| ID | Número autoincremental, el mismo que se muestra al usuario (con relleno de ceros solo al mostrarlo). |
| Nombre | Obligatorio; único entre productos activos (ver 7.1). |
| Descripción | Opcional. |
| Categoría | Opcional. |
| Unidad de medida | Opcional. |
| Precio de entrada (costo promedio ponderado) | Centavos de USD, mayor que 0 y hasta $999.999,99. Se fija al crear, junto con la cantidad inicial a la que corresponde; después solo lo cambian las facturas de entrada. **No se corrige** después de crear el producto: si se escribió mal (o la cantidad inicial), la salida es eliminarlo (P-14) y crearlo de nuevo (decisión del dueño). |
| % de ganancia | Entre 0 y 1000,00 %, con dos decimales (supuesto S-6). Es el dato que el usuario decide. |
| Precio de venta | **Derivado**: costo × (1 + % de ganancia / 100). No es editable. |
| Caducidad | Opcional; fecha sin hora. |
| Cantidad | Decimal, hasta 3 decimales y hasta 999.999,999 (supuesto S-3). **Se ingresa al crear el producto y debe ser mayor que 0** (cantidad inicial, P-01); después no se edita y solo cambia por facturas. El valor guardado puede ser 0 (Agotado), pero eso solo ocurrirá por operaciones de specs futuras (ventas, pérdidas): ningún comando de esta spec lo produce. |
| Estado | Activo o descontinuado; si es descontinuado, guarda la fecha y hora en que se descontinuó. |

**Agotado** = producto con cantidad 0. Puede estar activo o descontinuado. Con la cantidad inicial obligatoria y sin ventas todavía, en el uso real de esta spec ningún producto activo está agotado; los casos con cantidad 0 se prueban con fixtures (sección 14).

**Notificación (mínima):** tipo, prioridad, mensaje, fecha y hora de creación y estado leído/no leído. Columnas: `id` (`AUTOINCREMENT`), `type`, `priority`, `message`, `createdAt` (ISO 8601 UTC) e `isRead` (0 o 1). El esquema **no** limita con `CHECK` la lista de tipos ni de prioridades, para no obligar a recrear la tabla cuando se especifique Notificaciones; la validez de ambos la garantiza el código Rust. Son globales (una sola base local), no por usuario.

## 6. Requisitos funcionales

Cada requisito indica el RF de origen y sus escenarios de aceptación en formato Dado/Cuando/Entonces.

### Módulo: Categorías

#### CAT-01 — Crear categoría (RF-010)

- **Dado** un usuario autenticado, **cuando** completa Nombre (y opcionalmente Descripción) con un nombre que no existe, **entonces** la categoría se crea.
- **Dado** que el usuario escribe el nombre, **cuando** escribe, **entonces** el sistema busca las categorías existentes (7.2) y las muestra como **aviso de duplicado**; si alguna coincide exactamente con lo escrito (7.1), el aviso indica que esa categoría ya existe. Guardar es la única acción de alta: no hay un botón adicional de "agregar".
- **Dado** el nombre ya existe (sin distinguir mayúsculas ni acentos, 7.1), **cuando** intenta crearla, **entonces** el sistema deniega la creación indicando que la categoría ya existe.
- **Dado** un nombre vacío o que excede el largo permitido (7.3), **entonces** el sistema rechaza el formulario con un mensaje claro.

#### CAT-02 — Ver todas las categorías (RF-011)

- **Dado** un usuario autenticado, **cuando** abre la sección Categorías, **entonces** ve todas las categorías con Nombre, Descripción y **cantidad de productos** (activos y descontinuados), en el orden alfabético de 7.1.
- Antes de contar, el comando ejecuta la purga de P-08, de modo que no se cuenten productos ya vencidos.

#### CAT-03 — Editar categoría (RF-012)

- **Dado** una categoría existente, **cuando** el usuario abre la edición, **entonces** el formulario aparece precargado con su nombre y descripción, y puede modificarlos y guardar.
- **Dado** que guarda sin cambiar el nombre, o cambiando solo la descripción, **entonces** el cambio se acepta (la unicidad excluye la propia fila).
- **Dado** que el nuevo nombre coincide con otra categoría, **entonces** se deniega con el mismo mensaje de CAT-01.
- **Dado** que la categoría ya no existe, **entonces** se responde "La categoría ya no existe" (8.2).

#### CAT-04 — Eliminar categoría (RF-013)

- **Dado** el usuario elige eliminar una categoría, **cuando** lo hace, **entonces** el sistema pide confirmación antes de borrar. Si la categoría está asignada a productos, el mensaje de confirmación indica cuántos productos quedarán sin categoría (el conteo incluye descontinuados y se calcula tras la purga de P-08). Ese conteo proviene del último listado (CAT-02); el mensaje de resultado informa los reales.
- **Dado** que confirma y la categoría no está asignada a ningún producto, **entonces** se elimina sin más.
- **Dado** que confirma y la categoría está asignada a uno o más productos (activos o descontinuados), **entonces** se elimina, el campo Categoría de esos productos queda vacío y se genera **una** notificación N-09 —"Los siguientes productos quedaron sin categoría: [lista de nombres]", prioridad ADVERTENCIA— con todos los productos afectados (formato en NT-01). Todo ocurre de forma atómica: nunca queda la categoría eliminada sin la notificación ni al revés.
- **Dado** que la eliminación termina, **entonces** el mensaje de resultado informa cuántos productos quedaron sin categoría (mientras la pestaña de Notificaciones no exista, PEN-01).

#### CAT-05 — Crear categoría sobre la marcha desde producto (RF-014)

- **Dado** el usuario está en el formulario de creación o edición de producto, **cuando** escribe en el campo Categoría, **entonces** el sistema busca las categorías existentes (7.2).
- **Dado** que no hay ninguna coincidencia exacta con lo escrito, **entonces** se muestra la opción "Agregar nueva categoría" con el texto escrito; al elegirla, la categoría se crea en ese momento (con las mismas reglas de CAT-01) y queda seleccionada para el producto.
- **Dado** que el usuario escribe un texto en el campo y guarda el producto **sin haber elegido** una categoría de la lista ni "Agregar nueva categoría", **entonces** se rechaza con un mensaje que pide elegir una opción o vaciar el campo. Nunca se guarda ni se crea nada a partir de texto sin elegir.
- **Dado** que está **editando** un producto que tiene categoría, **entonces** dispone de un botón "Sin categoría" que deja el campo vacío.
- La categoría creada de este modo **existe aunque el usuario luego no guarde el producto** (supuesto S-7).
- **Dado** que la categoría elegida ya no existe al guardar el producto, **entonces** se responde "La categoría ya no existe" (8.2).

#### CAT-06 — Detalle de categoría

- **Dado** un usuario autenticado, **cuando** abre el detalle de una categoría, **entonces** ve su nombre, su descripción, la **cantidad de productos** y la **lista de esos productos** (activos y descontinuados, con su ID, nombre y condición), en el orden alfabético de 7.1.
- El detalle **no** muestra ingresos: dependen de ventas que aún no existen (PEN-10).
- Antes de contar y listar, el comando ejecuta la purga de P-08.
- **Dado** que la categoría ya no existe, **entonces** se responde "La categoría ya no existe" (8.2).

### Módulo: Unidades de medida

#### UM-01 — Crear unidad de medida (RF-015)

- **Dado** un usuario autenticado, **cuando** completa Nombre y Abreviación con valores que no existen, **entonces** la unidad se crea.
- **Dado** que escribe el nombre, **entonces** el sistema busca las unidades existentes (7.2) y las muestra como aviso de duplicado, igual que en CAT-01; Guardar es la única acción de alta.
- **Dado** el nombre o la abreviación ya existen (7.1), **cuando** intenta crearla, **entonces** se deniega indicando cuál de los dos ya está registrado.
- **Dado** un campo vacío o fuera de largo (7.3), **entonces** se rechaza con un mensaje claro.

#### UM-02 — Ver todas las unidades (RF-016)

- **Dado** un usuario autenticado, **cuando** abre la sección Unidades de Medida, **entonces** ve todas las unidades con Nombre, Abreviación y **cantidad de productos** (activos y descontinuados), en el orden alfabético de 7.1.
- Antes de contar, el comando ejecuta la purga de P-08.
- El listado entrega además, para cada unidad, la **cantidad de conversiones** en las que participa (como origen o destino). La interfaz la usa en la confirmación de UM-04; no es una columna obligatoria.

#### UM-03 — Editar unidad (RF-017)

- Igual que CAT-03, con Nombre y Abreviación: formulario precargado, unicidad que excluye la propia fila y mensaje que indica qué dato ya está registrado.

#### UM-04 — Eliminar unidad (RF-018)

- **Dado** el usuario elige eliminar una unidad, **entonces** el sistema pide confirmación. El mensaje **debe advertir todas las consecuencias**: cuántos productos quedarán sin unidad de medida (si los hay; incluye descontinuados y se calcula tras la purga de P-08) y cuántas conversiones se borrarán junto con ella (si las hay). Ambos conteos provienen del último listado (UM-02); el mensaje de resultado informa los reales.
- **Dado** que confirma y la unidad no tiene productos ni conversiones, **entonces** se elimina directamente.
- **Dado** que confirma y la unidad tiene conversiones, **entonces** todas sus conversiones se eliminan en cascada.
- **Dado** que confirma y la unidad está asignada a productos (activos o descontinuados), **entonces** el campo Unidad de Medida de esos productos queda vacío y se genera **una** notificación N-08 —"Los siguientes productos quedaron sin unidad de medida: [lista de nombres]", prioridad ADVERTENCIA, formato en NT-01—. Todo ocurre de forma atómica.
- **Dado** que la eliminación termina, **entonces** el mensaje de resultado informa cuántos productos quedaron sin unidad y cuántas conversiones se borraron.

#### UM-05 — Crear unidad sobre la marcha desde producto (RF-019)

- **Dado** el usuario escribe en el campo Unidad de Medida del formulario de producto, **entonces** el buscador encuentra unidades por **nombre y por abreviación** (7.2).
- **Dado** que no hay coincidencia exacta ni por nombre ni por abreviación, **entonces** se muestra "Agregar nueva unidad" con el texto escrito.
- **Dado** que la elige, **entonces** el sistema pide la **abreviación**: el texto escrito aparece precargado como nombre y **es editable**; al confirmar, la unidad se crea con las reglas de UM-01 y queda seleccionada para el producto (supuesto S-2). No se sugiere ninguna abreviación automática. Igual que en CAT-05, la unidad existe aunque no se guarde el producto.
- **Dado** que el usuario **cancela** este paso, **entonces** no se crea nada y el campo queda sin cambios.
- **Dado** que escribe un texto y guarda el producto **sin haber elegido** una unidad de la lista ni "Agregar nueva unidad", **entonces** se rechaza con un mensaje que pide elegir una opción o vaciar el campo.
- **Dado** que está **editando** un producto que tiene unidad, **entonces** dispone de un botón "Sin unidad" que deja el campo vacío.
- **Dado** que la unidad elegida ya no existe al guardar el producto, **entonces** se responde "La unidad ya no existe" (8.2).

#### UM-06 — Gestionar conversiones (RF-020)

**Qué:** relaciones entre unidades, globales, de carácter informativo en esta spec: no modifican productos, cantidades ni precios. Solo las usa el Conversor Rápido (UM-07) para calcular un resultado.

- **Dado** un usuario autenticado, **cuando** abre la sección Conversiones, **entonces** ve todas las conversiones en la forma "1 [unidad origen] = [factor] [unidad destino]", ordenadas por nombre de la unidad de origen y luego por el de la de destino (7.1).
- **Dado** que selecciona una unidad de origen, una de destino distinta y un factor decimal mayor que 0 (hasta 6 decimales, S-4 y 7.4), **cuando** guarda, **entonces** la conversión se crea y queda disponible en ambos sentidos: el sentido inverso se calcula (1 destino = 1/factor origen) y no se registra por separado.
- **Dado** que elige la misma unidad como origen y destino, **entonces** se rechaza con un mensaje claro.
- **Dado** que ya existe una conversión entre esas dos unidades (en cualquiera de los dos sentidos), **entonces** se rechaza indicando que ya existe; para cambiarla debe editarla.
- **Regla de camino único:** entre dos unidades solo puede haber un camino de conversiones, directo o a través de otras. **Dado** que la conversión nueva haría que las dos unidades quedaran relacionadas por un segundo camino —es decir, ya están relacionadas a través de otras (A↔B y B↔C impiden registrar A↔C)—, **entonces** se rechaza con un mensaje que explica que las unidades ya están relacionadas entre sí. Gracias a esta regla, cuando existe un camino entre dos unidades es **único**, y el Conversor Rápido (UM-07) no necesita elegir entre alternativas.
- **Dado** una conversión existente, **cuando** el usuario edita su factor, **entonces** se actualiza (solo el factor; para cambiar las unidades se elimina y se crea otra).
- **Dado** una conversión existente, **cuando** el usuario la elimina, **entonces** se pide confirmación y se borra.
- **Dado** que la conversión o alguna de sus unidades ya no existe, **entonces** se responde con el mensaje de 8.2.
- Los cambios y borrados de conversiones no afectan a ningún producto en esta spec.

#### UM-07 — Conversor Rápido

**Qué:** calcula cuánto equivale una cantidad expresada en una unidad en otra unidad, usando las conversiones registradas. El cálculo lo hace un comando del backend; la interfaz no calcula ni recorre conversiones por su cuenta.

- **Dado** un usuario autenticado, **cuando** elige una unidad de origen, una de destino y escribe una cantidad mayor que 0 con hasta 6 decimales (7.4), **entonces** ve el resultado de convertirla.
- **Cálculo:** se sigue el único camino entre las dos unidades (UM-06). En cada conversión "1 A = f B", pasar de A a B multiplica por f y pasar de B a A divide entre f. La aritmética es exacta (sin coma flotante). El resultado se **redondea a 6 decimales** (mitad hacia arriba) y se muestra **sin ceros finales**. El backend lo devuelve como **texto decimal**, no como número, para no perder precisión en la interfaz.
- **Dado** que no existe ningún camino de conversiones entre las dos unidades, **entonces** se responde con un mensaje claro que indica que no hay conversión entre ellas; no se inventa ningún resultado.
- **Dado** que origen y destino son la misma unidad, **entonces** el resultado es la cantidad escrita (S-12).
- **Dado** que alguna de las unidades ya no existe, **entonces** se responde "La unidad ya no existe" (8.2).
- **Dado** que el resultado excede 999.999.999,999999, **entonces** se rechaza con el mensaje "El resultado es demasiado grande para mostrarse" (`validationFailed`, S-12).
- **Dado** que el cálculo exacto no puede completarse sin desbordar la capacidad de la aritmética entera (caminos largos o factores muy grandes, aunque el resultado final fuera pequeño), **entonces** se rechaza con otro mensaje, "No se pudo calcular esta conversión: la cadena de unidades es demasiado larga" (`validationFailed`), para no atribuir a un resultado grande lo que es un límite de cálculo. Nunca se desborda en silencio.
- El Conversor Rápido solo lee: no crea ni modifica conversiones, unidades ni productos.

### Módulo: Productos

#### P-01 — Agregar producto (RF-021)

- **Dado** un usuario autenticado, **cuando** abre "Agregar Nuevo Producto" y completa Nombre, Categoría (opcional, con buscador y creación sobre la marcha), Unidad de Medida (opcional, con buscador y creación sobre la marcha), Precio de entrada (costo), **Cantidad inicial**, % de ganancia y opcionalmente Descripción y Caducidad con datos válidos, **entonces** el producto se crea activo, con esa cantidad inicial.
- El precio de venta **no se ingresa**: la interfaz lo muestra como valor calculado mientras el usuario escribe el % (solo comodidad; el valor lo calcula el backend con un comando de vista previa, 7.4, y es el mismo que luego se guarda). Al **editar**, el comando de vista previa recibe **solo el `%` y el `id` del producto**; el costo lo lee el backend **directamente de la base de datos** en el momento del cálculo, para que el precio nunca diverja del costo real. Al **crear**, como el producto aún no existe y no hay costo en la base, recibe el `%` y el costo escrito en el formulario (texto), que valida con las mismas reglas del costo (7.4).
- **Dado** que el costo es 0, negativo, no numérico, tiene más de 2 decimales o excede $999.999,99, **entonces** se rechaza con un mensaje claro (7.4).
- **Dado** que la cantidad inicial no se escribe, es 0, negativa, no numérica, tiene más de 3 decimales o excede 999.999,999, **entonces** se rechaza con un mensaje claro (7.4). La cantidad inicial es obligatoria.
- **Dado** que el % de ganancia es negativo, mayor que el máximo o tiene más de 2 decimales, **entonces** se rechaza (P-11).
- **Dado** que el nombre coincide con el de otro producto activo (7.1), **entonces** se deniega indicando que ya existe un producto con ese nombre.
- **Dado** que la caducidad es una fecha pasada, **entonces** se rechaza (7.5).
- **Dado** que se envía un **precio de venta** al crear, **entonces** el sistema lo rechaza con el mensaje genérico "Los datos enviados no son válidos": el contrato del comando no admite ese campo (el precio de venta se deriva). El rechazo ocurre al recibir los datos, antes de ejecutar el comando, por lo que no devuelve `validationFailed` ni se registra. Es un error de contrato, no alcanzable desde la interfaz.
- **Dado** que el `categoryId` o el `unitId` enviados ya no existen, **entonces** se responde "La categoría ya no existe" o "La unidad ya no existe" (8.2).

#### P-02 — Ver detalles del producto (RF-022)

- **Dado** un producto activo, **cuando** el usuario lo selecciona en el listado, **entonces** ve: ID, Nombre, Descripción, Categoría, Unidad de Medida, Precio de entrada promedio ponderado, Precio de venta, % de Ganancia, Caducidad (si tiene) y Cantidad disponible. Los precios se muestran en USD con dos decimales.
- **Dado** que el producto ya no existe, **entonces** se responde "El producto ya no existe" (8.2).

#### P-03 — Editar producto (RF-023)

- **Dado** un producto activo, **cuando** el usuario abre la edición, **entonces** el formulario muestra los datos actuales y permite modificar Nombre, Descripción, Categoría, Unidad de Medida, % de Ganancia y Caducidad. En Categoría y Unidad, un botón "Sin categoría" / "Sin unidad" deja el campo vacío (CAT-05, UM-05).
- **No** son editables: Cantidad, Precio de entrada (costo) ni Precio de venta (derivado del %). Se muestran, si acaso, como valores de solo lectura, y el comando de edición **rechaza** con el mensaje genérico "Los datos enviados no son válidos" cualquier envío de esos tres campos: su contrato no los admite. El rechazo ocurre al recibir los datos, antes de ejecutar el comando, por lo que no devuelve `validationFailed` ni se registra (error de contrato, no alcanzable desde la interfaz). El costo **no se corrige** después de crear el producto: si se escribió mal, la salida es eliminarlo (P-14) y crearlo de nuevo (decisión del dueño).
- **Dado** que cambia el % de ganancia, **entonces** el precio de venta se recalcula con el costo promedio vigente (P-11). El cálculo lo hace **siempre el backend** con el costo **actual en la base de datos** (no el que el usuario vio al abrir el formulario): al editar, el frontend envía solo el `%` y el `id` del producto. Así nunca hay divergencia entre el precio mostrado y el guardado.
- **Dado** que guarda sin cambios, o cambiando solo algunos campos, **entonces** se acepta; la unicidad del nombre excluye la propia fila, y una caducidad ya vencida que **no se modifica** no se rechaza (solo se rechaza asignar una fecha pasada, 7.5).
- Un producto descontinuado no se edita: primero debe activarse (P-07). **Dado** que se intenta editar un producto descontinuado, **entonces** se rechaza con "Un producto descontinuado no se puede editar. Actívalo primero" (`invalidState`, 8.2).
- **Dado** que el producto, la categoría o la unidad ya no existen, **entonces** se responde con el mensaje de 8.2.

#### P-04 — Descontinuar producto (RF-024)

- **Dado** un producto activo, **cuando** el usuario elige "Descontinuar", **entonces** el sistema pide confirmación —si el producto tiene cantidad mayor que 0, el mensaje lo advierte— y, al confirmar, el producto pasa a descontinuado y se registra la fecha y hora (reloj del sistema operativo, UTC).
- Un producto descontinuado deja de aparecer en el listado de productos activos (P-09).
- **Dado** un producto que ya está descontinuado, **cuando** se intenta descontinuarlo, **entonces** se rechaza con "El producto ya está descontinuado" (`invalidState`, 8.2).
- Si un producto descontinuado puede incluirse en facturas nuevas se define en la spec de Facturas (PEN-04).

#### P-05 — Ver productos descontinuados (RF-025) y P-13 — Pestaña "Agotados y Descontinuados" *(extensión pedida por el dueño)*

- **Dado** un usuario autenticado, **cuando** abre la pestaña **"Agotados y Descontinuados"**, **entonces** ve todos los productos que estén **descontinuados o agotados** (un mismo producto aparece una sola vez).
- **Columnas ("datos básicos"):** Nombre, Categoría (o "Sin categoría"), Unidad (o "—"), Cantidad, **Condición** (etiqueta: Agotado, Descontinuado o ambas), **Días restantes** (solo para descontinuados) y Acciones.
- **Orden:** alfabético por nombre (7.1); a igualdad, por ID.
- La pestaña permite filtrar por condición: Todos / Agotados / Descontinuados, y buscar por nombre (7.2). El dashboard abre esta pestaña con el filtro ya aplicado (DASH-01).
- **Dado** un producto descontinuado, **entonces** su fila muestra los días que faltan para su eliminación definitiva (60 días desde que se descontinuó, redondeado hacia arriba y acotado a 0–60, 7.5; supuesto S-5) y las acciones **"Activar"** y **"Eliminar"** (P-14).
- **Dado** un producto agotado que sigue activo, **entonces** su fila ofrece las mismas operaciones que en el listado de productos (P-09).
- El comando que lista esta pestaña ejecuta primero la purga de P-08, de modo que no se muestren productos ya vencidos.
- Con la cantidad inicial obligatoria (P-01) y sin ventas, en el uso real esta pestaña solo muestra productos **descontinuados** hasta que existan operaciones que lleven un producto a 0; los agotados se verifican con fixtures (sección 14).

#### P-06 — Ver detalles de producto descontinuado (RF-026)

- **Dado** un producto descontinuado, **cuando** el usuario lo selecciona, **entonces** ve los mismos datos de P-02, la fecha en que se descontinuó, los días restantes y los botones **"Activar"** y **"Eliminar"** (P-14). No se ofrece "Editar" ni "Descontinuar".
- El comando ejecuta primero la purga de P-08; si el producto ya venció, responde "El producto ya no existe".

#### P-07 — Activar producto descontinuado (RF-027)

- **Dado** un producto descontinuado, **cuando** el usuario elige "Activar", **entonces** pasa a activo, se borra su fecha de descontinuación y conserva todos sus datos, incluida la cantidad.
- **Dado** que existe otro producto **activo** con el mismo nombre, **entonces** la activación se rechaza con un mensaje que lo explica.
- **Dado** un producto que ya está activo, **cuando** se intenta activarlo, **entonces** se rechaza con "El producto ya está activo" (`invalidState`, 8.2).
- El comando ejecuta primero la purga de P-08; si el producto ya venció, responde "El producto ya no existe".

#### P-08 — Eliminar producto automático (RF-028)

- **Dado** un producto descontinuado cuya fecha de descontinuación es hace 60 días exactos o más (60 × 24 horas, reloj del sistema operativo), **cuando** se ejecuta la purga, **entonces** se elimina físicamente y no se puede recuperar.
- **La purga la ejecuta todo comando que liste, cuente o toque un producto descontinuado**: los listados de productos que incluyen descontinuados (P-13), el detalle, la activación y la eliminación de un descontinuado (P-06, P-07, P-14), y los comandos que cuentan o listan productos por categoría o unidad (CAT-02, CAT-04, CAT-06, UM-02, UM-04, y las listas de N-08 y N-09). Se ejecuta **antes** de contar, listar o actuar, de modo que nunca se cuente, muestre ni opere un producto ya vencido. Si el producto pedido ya venció, el comando responde "El producto ya no existe".
- **Si la purga falla**, no se aborta el comando: se registra el error y se continúa con el listado o la operación, **excluyendo en la consulta** los productos que ya cumplieron los 60 días (para no mostrarlos ni contarlos nunca vencidos). La interfaz muestra un **aviso visible** ("No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación") para que el usuario sepa que hay algo pendiente sin que la aplicación deje de funcionar. La purga se reintentará en la próxima operación. Si tampoco se puede contar cuántos son (por ejemplo, la base está dañada), el aviso omite la cifra: "No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación".
- **No hay purga de productos al iniciar la aplicación ni proceso de fondo**: se evita un temporizador permanente (`Constitution.md` §2.II), misma idea de `autenticacion-y-usuarios` AU-04 (sin proceso de fondo). Consecuencia: un producto vencido puede permanecer más de 60 días en la base si ningún comando lo toca, pero nunca se cuenta, se muestra ni se opera vencido.
- **Dado** que el producto se elimina, **entonces** no se genera ninguna notificación (N-07 queda pendiente, PEN-02).
- La eliminación es **física**; cómo conservan su información las facturas y el historial se define en sus specs (PEN-05).

#### P-09 — Ver todos los productos (RF-029) y P-12 — Búsqueda y filtros

- **Dado** un usuario autenticado, **cuando** abre la pestaña "Productos", **entonces** ve todos los productos **activos** (incluidos los agotados, marcados como tales) con su información básica y las operaciones disponibles (ver, editar, descontinuar y eliminar), ordenados alfabéticamente por nombre (7.1). El listado usa el **paginador global** de la aplicación (el tamaño de página se elige en Configuración).
- **Búsqueda:** por nombre, según 7.2.
- **Filtro por categoría:** una categoría concreta u "Sin categoría".
- **Filtro por stock:** Todos / Disponibles (cantidad > 0) / Agotados (cantidad = 0) / **Stock bajo (0 < cantidad < 5)**. Los agotados quedan aparte del stock bajo; el 5 es el mismo para cualquier unidad.
- **Filtro por caducidad:** Todos / Con caducidad (incluye los ya vencidos, S-11) / Sin caducidad / Vencidos (fecha anterior a hoy) / **Próximos a vencer (de hoy a hoy + 29 días, ambos inclusive)**.
- Los filtros se combinan entre sí y con la búsqueda. Los umbrales coinciden con los de las notificaciones futuras (N-02 cubre cantidad 0; N-03, 0 < cantidad < 5; N-04, de hoy a hoy + 29 días) y deben mantenerse iguales cuando se especifique Notificaciones (PEN-02).

#### P-10 — Calcular precio promedio ponderado (RF-030)

**Qué:** cálculo del costo promedio de un producto al recibir una compra. **Se especifica e implementa completo desde esta spec** (decisión del dueño), con sus tests, aunque su primer uso real llegará con las facturas de entrada (PEN-04). Al no tener todavía ningún comando que lo invoque, vive como módulo de librería del crate, con sus tests y sin warnings de `clippy`; el `plan.md` lo registra en Complexity Tracking.

- **Fórmula:** costo nuevo = (cantidad actual × costo actual + cantidad comprada × costo de la compra) / (cantidad actual + cantidad comprada), redondeado al centavo (mitad hacia arriba). Todos los costos están en centavos de USD y la aritmética es entera y exacta.
- **Ejemplo del requisito:** 10 unidades a $1,00 y 5 a $2,00 → (10×100 + 5×200) / 15 = 133,33 → **$1,33**.
- **Costo y cantidad iniciales:** el costo y la cantidad ingresados al crear el producto son su existencia inicial y **pesan** en el promedio con esa cantidad. Con el ejemplo del requisito, un producto creado con 10 unidades a $1,00 que recibe una compra de 5 a $2,00 queda en $1,33.
- **Producto sin existencias:** si la cantidad actual es 0 (un producto agotado por operaciones de specs futuras), el costo anterior pesa 0 y el resultado es exactamente el costo de la compra.
- La cantidad comprada debe ser mayor que 0 y el costo de la compra, mayor que 0; en caso contrario la función rechaza el cálculo.
- **Limitación aceptada:** al guardar en centavos, cada recálculo redondea y puede arrastrar una diferencia de menos de un centavo por unidad. Se acepta por simplicidad (`Constitution.md` §2.II).
- Cuando el costo promedio cambie, el precio de venta se recalcula manteniendo el % de ganancia (P-11).
- Mientras no existan facturas de entrada, el "Precio de entrada promedio ponderado" mostrado en P-02 es el costo inicial.

#### P-11 — Precio de venta a partir del % de ganancia (RF-031, **con desviación**)

**Qué:** el precio de venta lo determina el % de ganancia, nunca al revés.

- **Fórmula:** precio de venta = costo promedio × (1 + % de ganancia / 100), redondeado al centavo (mitad hacia arriba). Ejemplo: costo $12,50 y 20 % → $15,00.
- El % de ganancia es un valor entre 0 y 1000,00 (supuesto S-6), con hasta dos decimales. **No se permite ganancia negativa**; 0 % es válido (se vende al costo).
- El precio de venta no es un campo editable en ninguna pantalla ni comando: se calcula siempre. `plan.md` decide si se guarda o se calcula al leer; en cualquier caso, todos los módulos ven un valor coherente con el costo y el % vigentes.
- Al cambiar el costo promedio (facturas de entrada) o el %, el precio de venta se recalcula con el % guardado.
- La vista previa que muestra la interfaz al escribir el % **la calcula un comando del backend** con la misma función que el guardado, de modo que nunca difiera del valor guardado (`Constitution.md` §2.I). El comando no escribe nada. **Contrato:** recibe el **`%` como texto** y una de dos fuentes de costo: al editar, el **`id` del producto** (el **costo lo lee el backend de la base de datos**, no lo envía el frontend); al crear, el **costo escrito como texto** (aún no hay producto). Enviar ambas o ninguna es un error de contrato. Valida el `%` con las mismas reglas que el guardado (7.4) y devuelve el precio de venta en centavos. Así el precio mostrado nunca diverge del que se guardará. Mientras el valor escrito esté incompleto o sea inválido, la interfaz no muestra ningún error: deja la vista previa en blanco y el mensaje aparece al salir del campo o al guardar.

#### P-14 — Eliminar producto manualmente *(añadido a pedido del dueño)*

- **Dado** un producto, activo o descontinuado, **cuando** el usuario elige "Eliminar", **entonces** el sistema pide confirmación y, al confirmar, el producto se elimina físicamente y no se puede recuperar.
- La acción está disponible en las filas del listado de productos (P-09), en las filas de la pestaña P-13 (incluidas las de descontinuados) y en el detalle de cualquier producto (P-02, P-06).
- **Dado** que el producto tiene cantidad mayor que 0, **entonces** el mensaje de confirmación advierte con claridad que **se pierde el stock**. La eliminación se permite igualmente.
- **Dado** que el producto se elimina, **entonces** no se genera ninguna notificación. Cómo conservan su información las facturas y el historial se define en sus specs (PEN-05).
- **Dado** un producto descontinuado que ya venció, **entonces** la purga previa de P-08 lo habrá eliminado y el comando responde "El producto ya no existe".
- Es una alternativa a Descontinuar (P-04): descontinuar es reversible durante 60 días; eliminar no.

### Módulo: Dashboard

#### DASH-01 — Tarjeta de productos agotados

- **Dado** un usuario autenticado, **cuando** abre el Dashboard, **entonces** la tarjeta "Productos Agotados" muestra datos reales: el **total de productos agotados activos** (activos con cantidad 0), **hasta 3 categorías** con más productos agotados (nombre y cuántos agotados tiene cada una; en orden descendente por cantidad y, a igualdad, alfabético según 7.1) y **cuántos agotados activos no tienen categoría**.
- Los productos descontinuados no entran en estos números.
- Con la cantidad inicial obligatoria (P-01) y sin ventas, en el uso real la tarjeta muestra el estado vacío; sus cifras se verifican con fixtures (sección 14).
- **Dado** que no hay productos agotados, **entonces** la tarjeta muestra total 0, sin categorías y 0 sin categoría (estado vacío), sin datos de ejemplo.
- Los botones **"Ver Agotados"** y **"Ver Descontinuados"** abren la pestaña "Agotados y Descontinuados" (P-13) con el filtro de condición ya aplicado (Agotados o Descontinuados, respectivamente).
- Todos los datos de la tarjeta los entrega **un único comando de resumen** del backend.
- Los **KPIs**, "Mejor Vendidos" y "Lento Movimiento" se muestran en **estado vacío** (sin cifras ni listas inventadas) hasta la spec de Facturas de venta (PEN-10).

### Notificaciones mínimas

#### NT-01 — Registro de notificaciones N-08 y N-09

- **Dado** que se elimina una categoría o unidad con productos asignados, **entonces** se inserta una notificación con tipo `N-09` (categoría) o `N-08` (unidad), prioridad `ADVERTENCIA`, el mensaje indicado en CAT-04 / UM-04, fecha y hora de creación y estado "no leída". Los nombres de los productos quedan escritos en el mensaje en ese momento.
- **Formato de la lista de nombres:** separados por coma, en el orden alfabético de 7.1; se listan hasta **20** y, si hay más, se añade "y N más" (por ejemplo, "…: A, B, C y 5 más").
- **Dado** que no hay productos afectados, **entonces** no se genera ninguna notificación.
- No existe todavía ninguna pantalla ni comando para leer, marcar o borrar notificaciones: eso es PEN-01. Las notificaciones generadas se conservan hasta entonces.

---

## 7. Reglas de negocio transversales

### 7.1 Unicidad de nombres y normalización

- Categoría (nombre), unidad (nombre y abreviación) y producto (nombre, entre productos **activos**) son únicos **sin distinguir mayúsculas ni acentos**: "Camión", "camion" y "CAMIÓN" son el mismo valor. La **ñ se considera una letra distinta** de la n ("año" ≠ "ano").
- **Tabla de normalización** (se aplica igual a la unicidad y a la búsqueda de 7.2, sin dependencias nuevas):
  1. Se recortan los espacios de los extremos y los espacios internos repetidos se reducen a uno.
  2. Todo el texto pasa a minúsculas (`Á→á`, `Ñ→ñ`, etc.).
  3. Se pliegan únicamente estas vocales: `á→a`, `é→e`, `í→i`, `ó→o`, `ú→u`, `ü→u`.
  4. La `ñ` se conserva. Cualquier otro carácter, incluidos otros acentos o signos, se compara tal cual tras pasar a minúsculas. **Limitación aceptada (plegado parcial):** solo se pliegan las 6 vocales del punto 3. Otras letras latinas acentuadas (`è`, `â`, `ç`, `ï`…) se comparan tal cual: `Cafè` y `Cafe` son productos distintos y esas letras se ordenan después de la `z`. Se acepta porque el uso previsto es el español de Cuba; si el negocio lo necesitara, se amplía la tabla de plegado, un cambio local en `text.rs` que no toca el esquema.
  - No se hace normalización Unicode NFC/NFD (sin dependencia nueva); los teclados habituales producen texto compuesto, así que una letra acentuada escrita en forma descompuesta no se equipara con la compuesta. **Limitación aceptada conscientemente** por el dueño: el proyecto es local, offline y de un solo usuario, que usa el mismo sistema operativo y el mismo teclado, por lo que el riesgo de mezclar NFC y NFD es bajo en la práctica. Si el proyecto cambia de contexto (multi-usuario, exportación, red), se reevaluará añadiendo `unicode-normalization`.
- **Orden alfabético español** para **todos** los listados (categorías, unidades, conversiones, productos, pestaña P-13, listas de nombres de NT-01): se compara por el texto normalizado de arriba, con la `ñ` ubicada justo después de la `n` (antes de la `o`); a igualdad, por ID. Así "Ángel" va con la A y "ñame" va después de "nuez" y antes de "oso". Las conversiones se ordenan por la unidad de origen y, a igualdad, por la de destino.
- La unicidad se declara en el esquema (`Constitution.md` §4), además de normalizarse en el código. `COLLATE NOCASE` no sirve aquí porque solo pliega ASCII: se usa una columna normalizada con `UNIQUE` (índice parcial para productos activos); el detalle corresponde a `plan.md`.
- Un producto descontinuado no bloquea el nombre: se puede crear uno nuevo con ese nombre, pero entonces el descontinuado no podrá activarse mientras exista el activo (P-07).
- Las validaciones de unicidad al editar excluyen la propia fila (mismo criterio que `autenticacion-y-usuarios` US-04).

### 7.2 Búsqueda "a medida que escribe"

- Coincidencia por subcadena sobre el texto normalizado de 7.1 (sin distinguir mayúsculas ni acentos, con la ñ como letra distinta).
- Los caracteres `%` y `_` son **texto literal**: no actúan como comodines.
- Los buscadores de **unidades** buscan por nombre y por abreviación.
- Se ofrece "Agregar nueva…" solo si ninguna existente coincide **exactamente** con lo escrito según 7.1 (en las unidades, ni por nombre ni por abreviación).
- **Unidades con doble coincidencia exacta:** como la unicidad es por columna (7.1), puede existir una unidad llamada `kg` y otra con abreviación `kg`. Si lo escrito coincide exactamente con ambas, el buscador muestra **las dos**, cada una con su nombre y abreviación para distinguirlas, y no ofrece "Agregar nueva unidad". El sistema no prohíbe ese caso.

### 7.3 Largos de texto (supuesto S-8)

- El largo se cuenta en **caracteres** (puntos de código Unicode), no en bytes ni en unidades UTF-16; el límite del cliente puede contar distinto y la regla autoritativa es la del backend.
- Se recortan los espacios de los extremos, se reducen los espacios internos repetidos a uno, y no se aceptan campos obligatorios vacíos. Se consideran espacios los caracteres de espacio en blanco de Unicode. En nombres, cualquier secuencia de espacios (incluidos saltos de línea y tabulaciones) se reduce a un único espacio; en descripciones se conservan los saltos de línea y solo se reducen los espacios y tabulaciones repetidos dentro de cada línea. El texto se guarda **tal como se escribió** (mayúsculas y acentos incluidos) una vez aplicado ese recorte; la versión normalizada de 7.1 es un dato aparte, solo para unicidad y búsqueda.
- Una descripción vacía tras el recorte se guarda como `NULL`.
- **Caracteres no permitidos (se rechazan):** una vez aplicado el recorte, se rechaza cualquier texto (nombre, abreviación o descripción) que contenga caracteres de **control** (categoría Unicode Cc, incluido el carácter nulo) o de **formato** (categoría Cf; por ejemplo, el espacio de ancho cero U+200B, el conector de ancho cero U+200D (ZWJ) —que usan algunos emojis compuestos—, las marcas de dirección de texto y U+FEFF). Los espacios, tabulaciones y saltos de línea ya se resolvieron en el recorte y no cuentan como control. Un texto compuesto solo por caracteres invisibles se rechaza con este mensaje, no como "vacío".
- **Saltos de línea en la descripción:** se permiten; `\r\n` y `\r` se guardan como `\n`.
- Categoría: nombre 1–50, descripción hasta 200. Unidad: nombre 1–30, abreviación 1–10. Producto: nombre 2–100, descripción hasta 500.

### 7.4 Números y dinero

- Todo precio se guarda como número entero de **centavos de USD**. Al mostrarlo, la interfaz lo convierte a dólares con dos decimales mediante una división sencilla. Al enviarlo, el usuario escribe dólares y el backend convierte el texto a centavos con aritmética entera exacta, sin usar coma flotante.
- **Formato de entrada** (precio, %, cantidad, factor y cantidad del Conversor): dígitos con, opcionalmente, **un** separador decimal, que puede ser **coma o punto**, con al menos un dígito a cada lado (se rechazan `,5` y `5,`). **No existe separador de miles.** Se rechazan tal como se escriben: notación científica (`1e3`), signos (`+5`, `-5`), espacios y más decimales de los permitidos.
- **Ambigüedad del punto:** la interfaz muestra los números con formato `1.234,50`, pero al escribir el punto es un decimal. Por eso se **rechaza** un valor con un solo punto, seguido de exactamente 3 dígitos, sin coma y con parte entera distinta de 0 (por ejemplo `1.234`), con un mensaje que explica que no se usa separador de miles y que los decimales se escriben con coma. Quien quiera el decimal escribe `1,234`.
- **Ceros a la izquierda:** se aceptan (`007` equivale a 7).
- **Orden de evaluación de los mensajes:** 1.º formato (vacío, caracteres no permitidos, `,5`, `5,`); 2.º ambigüedad del punto; 3.º exceso de decimales; 4.º rango. Se informa el primero que falle.
- **Rangos y decimales:**

| Dato | Rango | Decimales |
|---|---|---|
| Precio de entrada (costo) | mayor que 0 y hasta 999.999,99 | hasta 2 |
| % de ganancia | 0 a 1000,00 | hasta 2 |
| Cantidad inicial del producto (P-01) | mayor que 0 y hasta 999.999,999 | hasta 3 |
| Factor de conversión | 0,000001 a 999.999.999,999999 | hasta 6 |
| Cantidad del Conversor Rápido (UM-07) | mayor que 0 y hasta 999.999.999,999999 (S-12) | hasta 6 |
| Resultado del Conversor Rápido (UM-07) | hasta 999.999.999,999999 (S-12) | 6 (redondeado) |

- Todos los cálculos (precio de venta, promedio ponderado, conversiones) usan **aritmética entera exacta**, sin coma flotante, con redondeo mitad hacia arriba donde se indica.
- El backend es el único que valida y calcula; los valores derivados que ve el usuario provienen de él, incluida la **vista previa del precio de venta** (P-11).
- Las demás monedas se resolverán en Configuración (PEN-07).

### 7.5 Fechas

- **Caducidad:** se guarda como fecha `YYYY-MM-DD`, sin hora, y se compara con la fecha local del sistema operativo. Al crear un producto o al cambiar su caducidad, no puede ser anterior a hoy (hoy es válido). Debe ser una fecha de calendario válida (`2026-02-30` se rechaza) con año máximo 2100; fuera de ese rango se rechaza con un mensaje claro.
- **Fecha de descontinuación:** fecha y hora UTC en ISO 8601, con el reloj del sistema operativo, igual que las sesiones (`autenticacion-y-usuarios` AU-05). Cambiar la fecha del equipo puede adelantar o retrasar la purga; se acepta por el mismo motivo allí documentado.
- **Visualización:** toda fecha se muestra en **hora local** con formato `dd/mm/aaaa`.
- **Días restantes** de un descontinuado: 60 días menos el tiempo transcurrido, redondeado hacia arriba y **acotado al rango 0–60** (un cambio de reloj nunca produce valores negativos ni mayores que 60).

### 7.6 Cantidades

Decimales de hasta 999.999,999, con hasta 3 decimales (S-3), se muestran sin ceros finales. Se fijan al crear el producto (mayores que 0, 7.4) y después solo cambian por facturas (spec futura); ninguna pantalla ni comando de esta spec las modifica.

### 7.7 Eliminaciones y claves foráneas

- La relación producto→categoría y producto→unidad se declara con clave foránea en el esquema, de modo que al eliminar la categoría o la unidad el campo del producto quede vacío por definición del esquema, no por lógica dispersa en los comandos. Las conversiones dependen de sus dos unidades y se borran con ellas.
- La eliminación de productos, categorías, unidades y conversiones es **física**.

### 7.8 Datos iniciales

La base de datos de esta feature **empieza vacía**: no hay categorías, unidades, conversiones ni productos semilla. Toda pantalla tiene un estado vacío claro en español (por ejemplo, "Aún no hay categorías") y el dashboard muestra ceros y estados vacíos hasta que existan datos. Nunca se rellena con datos de ejemplo (`Constitution.md` §2.I).

## 8. Manejo de errores

### 8.1 Mensajes y registro

Rige `autenticacion-y-usuarios` §8: mensajes en español, sin detalles técnicos; errores de validación y de reglas de negocio con mensaje **específico** (qué dato está duplicado, qué regla se incumple, qué camino de conversiones se evita); cualquier error interno con un mensaje **genérico**.

- **Registro:** todo error **inesperado** se registra en el log local con módulo, operación, causa y fecha. Los rechazos de validación esperados (`validationFailed`, `invalidCredentials`) **no** se registran, igual que en la spec 1. Los demás rechazos de reglas de negocio (duplicados, entidad que ya no existe, conversión ya existente o segundo camino, estado inválido) usan sus propios códigos (`duplicateField`, `notFound`, `invalidState`, etc.), que **sí** se registran, como en la spec 1 (`should_log()` no cambia).
- **Privacidad:** ningún registro contiene datos personales ni valores ingresados por el usuario.
- **Desviación:** alineado con `Constitution.md` v1.2.0 §2.VI: los rechazos esperados no se registran; no hay desviación que documentar en el `plan.md`.

### 8.2 Entidad inexistente y catálogo de mensajes

- Si un comando recibe el identificador de una entidad que ya no existe (porque se eliminó, o porque la purga de P-08 la borró), responde con un mensaje específico: "El producto ya no existe", "La categoría ya no existe", "La unidad ya no existe" o "La conversión ya no existe".
- Lo mismo aplica cuando un producto que se crea o edita referencia un `categoryId` o `unitId` que ya no existe.
- **Estado inválido** (código `invalidState`): "Un producto descontinuado no se puede editar. Actívalo primero", "El producto ya está descontinuado" y "El producto ya está activo" (P-03, P-04, P-07). **Resultado demasiado grande** del Conversor (UM-07, `validationFailed`): "El resultado es demasiado grande para mostrarse"; **cálculo fuera de capacidad** (`validationFailed`): "No se pudo calcular esta conversión: la cadena de unidades es demasiado larga".
- El `plan.md` define el **catálogo de códigos de error y los textos exactos** de cada mensaje, ampliando el catálogo existente de `errors.rs` sin cambiar los códigos actuales.

## 9. Interfaz

La revisión del HTML completo (PEN-03) está hecha y sus resultados están en `frontend-cambios.md`, que es el insumo de interfaz del `plan.md`. La interfaz se adapta a esta spec, y no al revés, con los ajustes aquí descritos: columnas "Valor Total" e "Ingresos", Historial y Facturas Asociadas ocultos o vacíos (sección 3.2); Conversor Rápido conservado con comando de backend (UM-07); detalle de categoría (CAT-06); pestaña "Agotados y Descontinuados".

**Ya decididos y que la interfaz debe respetar:** todo dato proviene del backend (sin mock); el precio de venta no es editable y su vista previa la calcula el backend; el costo y la cantidad no se editan; las validaciones del cliente son solo una ayuda de UX y las reglas autoritativas son las del backend; los buscadores de categoría y unidad admiten creación sobre la marcha, con error si se guarda texto sin elegir opción; los campos numéricos aceptan coma o punto como decimal (7.4); los textos visibles son en español.

Además, por la auditoría de seguridad y robustez: (1) **todo dato que vino del usuario se muestra como texto, nunca como HTML** (un nombre como `<img src=x onerror=…>` se ve tal cual y no ejecuta nada); (2) las búsquedas a medida que se escribe, la vista previa del precio y el Conversor se envían al backend con una pausa de **300 ms** tras la última pulsación, y la respuesta de una consulta ya superada por otra más reciente se descarta; (3) todo botón que guarda, crea o confirma una acción se deshabilita mientras dura la llamada, para que un doble clic no la repita; (4) **CSP:** no se activa en esta feature. El proyecto es local, offline y de un solo usuario, sin exposición externa; activar CSP exigiría migrar los manejadores y estilos inline de todo el HTML heredado (incluidas secciones sin spec todavía), lo que rompería la aplicación. La defensa de esta feature es la salida segura del punto (1). La activación de CSP queda como pendiente de baja prioridad (PEN-12) y se reevaluará si el proyecto cambia de contexto; (5) **teclado y foco:** los buscadores con lista (categoría y unidad) se operan con el teclado —flechas para recorrer las opciones, Enter para elegir, Escape para cerrar la lista sin elegir— y los diálogos (confirmación y alta de unidad) mantienen el foco dentro mientras están abiertos, se cierran con Escape y devuelven el foco al elemento que los abrió.

## 10. Pendientes para próximas specs

Lo que esta spec deja intencionalmente fuera, y que debe retomarse. Se mantiene también en `specs/pendientes.md`, el registro común a todas las specs.

| # | Pendiente | Origen | Spec destino |
|---|---|---|---|
| PEN-01 | Módulo de Notificaciones completo: pestaña, contador de no leídas, marcar como leída, eliminar una/todas (RF-115, RF-124), y la visualización de las notificaciones N-08/N-09 que `inventario-productos` ya genera, y las notificaciones de usuario (crear, editar y eliminar un usuario; `crearNotificacion('usuario', …)` del mockup) que `autenticacion-y-usuarios` dejó diferidas. También definir cuánto tiempo se conservan las notificaciones: la tabla `notifications` crece sin límite y desde esta spec se llena con N-08/N-09. | `inventario-productos` §3.2; `autenticacion-y-usuarios` (`plan-01` §7) | Notificaciones |
| PEN-02 | Notificaciones automáticas N-02 (producto agotado, cantidad 0; también al intentar vender uno agotado), N-03 (stock bajo, 0 < cantidad < 5), N-04 (próximo a vencer, de hoy a hoy + 29 días) y N-07 (descontinuado próximo a eliminarse, < 10 días). Deben reutilizar los mismos umbrales que los filtros de stock y caducidad del listado de productos (P-12). | `inventario-productos` §3.2 | Notificaciones (N-02/N-03/N-04/N-07 también dependen de Facturas de venta) |
| PEN-03 | ~~Revisión completa de `dashboard-unificado.html`.~~ **Resuelto** por `frontend-cambios.md`. | `inventario-productos` §9 | — |
| PEN-04 | Facturas de entrada: invocar el cálculo del promedio ponderado (P-10), actualizar cantidad y costo del producto, recalcular el precio de venta manteniendo el % de ganancia (P-11) y definir si un producto descontinuado puede incluirse en facturas nuevas. | `inventario-productos` P-10 | Facturas de entrada (RF-044 a RF-051) |
| PEN-05 | Facturas (todos los tipos) e Historial de productos: guardar siempre una copia de la información del producto (al menos nombre y precios del momento) para que, si el producto se elimina (automáticamente, P-08, o manualmente, P-14), la factura y el historial conserven su información. Definir el manejo de la referencia como en `autenticacion-y-usuarios` US-07. | Respuesta del dueño (pregunta 20) | Facturas e Historial de productos |
| PEN-06 | Uso real de las conversiones entre unidades (RF-020) en productos, compras y ventas: en `inventario-productos` solo las usa el Conversor Rápido (UM-07) para mostrar un resultado, y no afectan a ningún producto ni cantidad. | `inventario-productos` UM-06 | Facturas de entrada / venta |
| PEN-07 | Configuración: monedas adicionales al USD (EUR, CUP, MLC, Saldo Clásica) y tasas de cambio. `inventario-productos` guarda y expone todos los precios solo en centavos de USD. | `inventario-productos` §7.4 | Configuración |
| PEN-08 | Notificación de producto agotado al intentar vender uno (RF-116) y descuento de inventario: la cantidad solo se fija al crear el producto y después solo cambia por facturas; en esta spec ningún comando la modifica ni la lleva a 0. | `inventario-productos` P-01 | Facturas de venta |
| PEN-09 | Historial de producto (pestaña y botón "Ver Historial") y sección "Facturas Asociadas" del formulario y del detalle: ocultos hasta que existan sus specs. El historial debe registrar la cantidad inicial de cada producto como su primer movimiento. | `inventario-productos` §3.2 | Historial de productos / Facturas |
| PEN-10 | Columnas "Valor Total" (productos) e "Ingresos" (categorías y detalle de categoría), KPIs, "Mejor Vendidos" y "Lento Movimiento" del dashboard: ocultos o en estado vacío y sin datos hasta que existan ventas. | `inventario-productos` DASH-01, CAT-06 | Facturas de venta |
| PEN-11 | Copias de seguridad de la base de datos (por ejemplo `VACUUM INTO` a un archivo con fecha, con aviso periódico y opción de restaurar). Esta spec elimina datos de forma física e irreversible (purga automática, P-14, categorías y unidades) y un adelanto de la fecha del equipo puede adelantar la purga (7.5); con cortes de energía frecuentes conviene tener una copia. | Auditoría de `inventario-productos` (M-03) | Configuración / Respaldo |
| PEN-12 | Endurecimiento de seguridad del frontend heredado: reemplazar los `onclick`/`oninput`/`onchange` y los `style=` inline por listeners y clases, construir con DOM (o escapar) los `innerHTML` que interpolan datos en el código aún no migrado (facturas, clientes, proveedores…) y, solo entonces, activar una CSP estricta en `tauri.conf.json` (hoy `csp: null`). Con los manejadores inline actuales una CSP estricta rompería la aplicación y, con `'unsafe-inline'`, no frenaría un script inyectado. | Auditoría de `inventario-productos` (A-01) | Una spec de endurecimiento y cada spec al migrar su sección |
| PEN-13 | Registro de quién eliminó o descontinuó qué y cuándo (productos, categorías y unidades): ambos roles pueden eliminar y hoy no queda rastro del autor. | Auditoría de `inventario-productos` (M-03) | Historial / auditoría |
| PEN-14 | **Rediseño de notificaciones tipo toast.** En el módulo de Notificaciones se evaluarán **todas** las notificaciones que el sistema genera al usuario (N-01 a N-09, tanto de `autenticacion-y-usuarios` como de `inventario-productos` y futuras) y se rediseñará su UI y forma de mostrarse. En lugar de ser texto plano insertado en el HTML (como hoy), serán notificaciones tipo **toast** (emergentes, no intrusivas, con estilo consistente, tiempo de vida configurable y opción de descartar o marcar como leída). Este pendiente es **transversal**: cualquier notificación nueva que se genere en specs futuras debe diseñarse ya pensando en ese rediseño (mantener los datos estructurados y el renderizado separado; no acoplar el mensaje a HTML específico). Las notificaciones N-08 y N-09 de esta spec se guardan como **texto plano** en `message`; el renderizado (toast) se hará con `textContent` a partir de ese texto. | Auditoría QA de `inventario-productos` | Notificaciones |

## 11. Supuestos

Puntos que el dueño no decidió expresamente y que esta spec fija con una propuesta razonable.

| # | Supuesto | Estado |
|---|---|---|
| S-1 | **Pestaña P-13:** una sola pestaña "Agotados y Descontinuados" que reemplaza a la pestaña "Descontinuados" de RF-025, con un filtro por condición. El listado de Productos incluye los agotados activos. | Confirmado |
| S-2 | **Unidad sobre la marcha (RF-019):** el requisito solo aporta el texto escrito, pero la unidad necesita también abreviación; se pide en un paso breve tras elegir "Agregar nueva unidad". | Confirmado |
| S-3 | **Decimales de cantidad:** hasta 3. | Confirmado |
| S-4 | **Factor de conversión:** hasta 6 decimales, mayor que 0. Entre dos unidades solo puede haber un camino: registrar A↔C se rechaza si ya hay un camino A↔B↔C. | Confirmado |
| S-5 | **Días restantes** de un descontinuado: se redondean hacia arriba. | Confirmado |
| S-6 | **Tope del % de ganancia:** 1000,00 %. | Confirmado |
| S-7 | **Creación sobre la marcha:** la categoría o unidad se crea al elegirla, aunque luego se cancele el formulario del producto. | Confirmado |
| S-8 | **Largos máximos** de la sección 7.3. | Confirmado |
| S-9 | **Promedio ponderado:** el costo y la cantidad iniciales ingresados al crear el producto pesan en el promedio (si la cantidad actual llegara a ser 0, el resultado es el costo de la compra), y cada recálculo se redondea al centavo. Confirmado junto con la cantidad inicial obligatoria (> 0). | Confirmado |
| S-10 | **Eliminar categoría/unidad:** el mensaje de resultado informa cuántos productos quedaron afectados aunque aún no exista la pestaña de Notificaciones. | Confirmado |
| S-11 | **Filtro "Con caducidad" (P-12):** incluye también los productos ya vencidos; "Vencidos" y "Próximos a vencer" son subconjuntos de "Con caducidad". | Confirmado |
| S-12 | **Conversor Rápido (UM-07):** la cantidad admite hasta 999.999.999,999999; con la misma unidad de origen y destino el resultado es la cantidad escrita; un resultado mayor que 999.999.999,999999 se rechaza con un mensaje claro, y un cálculo que desborde la aritmética entera con otro distinto (8.2). | Confirmado |

## 12. Desviaciones respecto de `Requisitos_del_sistema.docx`

- **RF-031 (Vincular precio de venta y % de ganancia):** el requisito permite ingresar cualquiera de los dos y recalcula el otro. Por decisión del dueño, **solo el % de ganancia determina el precio de venta**; el precio de venta no es editable (Opción B del RF-031 se elimina). Afecta también a RF-021 y RF-023, donde el precio de venta deja de ser un campo de entrada.
- **RF-021:** el precio de entrada inicial es obligatorio, mayor que 0 y hasta $999.999,99 (el requisito no fijaba el mínimo ni el tope). La cantidad inicial es obligatoria y mayor que 0 (decisión del dueño).
- **RF-025:** la pestaña de descontinuados se amplía para incluir también a los agotados y pasa a llamarse "Agotados y Descontinuados" (P-13), a pedido del dueño. Los descontinuados ofrecen además "Eliminar" (P-14).
- **RF-029:** se añaden búsqueda y filtros por categoría, stock y caducidad (P-12), a pedido del dueño, y se usa el paginador global.
- **RF-020:** las conversiones son bidireccionales, con factor decimal de hasta 6 decimales, con **camino único entre dos unidades** (más estricto que "sin ciclos") y editables; el requisito no lo detallaba.
- **RF-013 / RF-018:** el mensaje de confirmación informa las consecuencias concretas (cantidad de productos y conversiones afectados) y la eliminación de una unidad borra sus conversiones en cascada.
- **RF-028:** la eliminación automática **no** se ejecuta "en segundo plano" ni al iniciar la aplicación: la ejecuta todo comando que liste, cuente o toque un descontinuado (P-08). Se mantiene la idea de AU-04 (sin temporizador permanente), pero sin purga al iniciar la aplicación. Si la purga falla, el comando **no se aborta**: se registra el error, se excluyen en la consulta los vencidos y se muestra un aviso visible al usuario (purga best-effort).
- **RF-010, RF-011 y siguientes:** donde el requisito dice "la contadora", esta spec habilita a Administrador y Contador.
- **Adiciones sin RF de origen, a pedido del dueño:** DASH-01 (tarjeta de agotados del dashboard), UM-07 (Conversor Rápido con comando de backend), CAT-06 (detalle de categoría) y P-14 (eliminación manual de producto).

## 13. Fuera de alcance (recapitulación)

- Pestaña de Notificaciones y notificaciones N-02, N-03, N-04, N-07.
- Cambios de cantidad y de costo (facturas), historial, ventas.
- Uso real de las conversiones en productos, compras y ventas.
- Otras monedas y tasas de cambio.
- Historial de producto y Facturas Asociadas (ocultos).
- Columnas "Valor Total" e "Ingresos", KPIs, "Mejor Vendidos" y "Lento Movimiento" con datos (estado vacío hasta Facturas de venta).

## 14. Criterios de prueba

Esta sección fija qué debe poder verificarse; el detalle de cada test corresponde a `plan.md` y `tasks.md`.

- **Cantidad inicial:** todo producto creado con los comandos nace con la cantidad inicial indicada (mayor que 0) y ningún comando de esta spec la modifica después. En consecuencia, **con datos reales** la pestaña P-13 muestra solo los productos descontinuados (ningún activo está agotado), la tarjeta de agotados del dashboard queda en cero y el filtro "Agotados" devuelve vacío; "Disponibles" devuelve todos los productos activos y "Stock bajo" los creados con menos de 5.
- **Cantidad inicial inválida:** omitida, 0, negativa, no numérica, con más de 3 decimales o mayor que 999.999,999 se rechaza (7.4).
- **Casos con cantidad 0 o exacta:** como ningún comando produce un producto agotado, se prueban con **fixtures insertados directamente en la base de datos de pruebas** (`setup_test_db`), nunca con comandos de la aplicación ni con código de producción que altere la cantidad. Cubren: la condición Agotado, el filtro de stock "Agotados", la pestaña P-13, la tarjeta de agotados del dashboard y, con cantidades fijadas por fixture, el filtro Stock bajo (0 < cantidad < 5 y su frontera en 5) y la advertencia de stock al descontinuar y al eliminar.
- **Autorización:** matriz con una fila por cada comando nuevo (incluidos vista previa de precio, Conversor, resumen del dashboard y eliminación de producto): sin sesión → rechazado; con cualquiera de los dos roles → permitido.
- **Esquema:** un test por cada `CHECK`, `UNIQUE` (incluidos los de columnas normalizadas y el índice parcial de productos activos) y clave foránea (`ON DELETE SET NULL`, cascada de conversiones).
- **Normalización y orden:** tests de la tabla de 7.1 (`Camión`/`CAMIÓN`/`camion` iguales; `año` distinto de `ano`; espacios internos colapsados) y del orden alfabético español (acentos ignorados, ñ después de n).
- **Números:** tests de 7.4: coma y punto, rechazo de `1e3`, `+5`, espacios, exceso de decimales, `1.234` y topes de cada rango.
- **Conversiones:** rechazo de misma unidad, de duplicado y de segundo camino (A↔B, B↔C, A↔C); Conversor Rápido en ambos sentidos, con varios tramos, sin camino, con redondeo a 6 decimales y sin ceros finales.
- **Purga:** con fixtures de fechas, un descontinuado de 60 días exactos se purga y uno de 59 días no; los conteos de categorías y unidades y las listas de N-08 y N-09 no incluyen productos vencidos; el comando responde "El producto ya no existe" para uno vencido.
- **Errores:** `validationFailed` e `invalidCredentials` no generan fila en `errorLogs`; los demás códigos (`duplicateField`, `notFound`, `invalidState`, etc.) sí, y ningún registro contiene valores ingresados.
- **Atomicidad:** si falla la inserción de la notificación, la categoría o la unidad no se elimina (CAT-04, UM-04).
- **NT-01:** fronteras de 20 y 21 nombres ("y N más") y orden alfabético español en la lista.
- **Filtros (P-12):** frontera de caducidad (hoy incluido, hoy + 29 incluido, hoy + 30 excluido) y combinación de filtros con la búsqueda.
- **Dashboard (DASH-01):** límite de 3 categorías y desempate alfabético.
- **Arranque:** iniciar la aplicación no purga productos.
- **Contratos y estados:** precio de venta (al crear) y cantidad, precio de venta y costo (al editar) rechazados con "Los datos enviados no son válidos"; editar un descontinuado, descontinuar uno ya descontinuado y activar uno ya activo rechazados con su mensaje de `invalidState`.
- **Conversor:** resultado mayor que 999.999.999,999999 y desbordamiento en caminos largos rechazados, cada uno con su propio mensaje (UM-07); el resultado se devuelve como texto.
- **Números y texto:** `0.123`, `1.234` y `007`, el orden de mensajes de 7.4, caducidad `2026-02-30` y año mayor que 2100, y el colapso de espacios y saltos de línea de 7.3 (nombres frente a descripciones).
- **Listado de unidades:** `conversionCount` correcto (unidad como origen o como destino).
- **Caracteres no permitidos:** nombre, abreviación y descripción con NUL, otro carácter de control, U+200B, U+200D, U+FEFF, U+202E o U+00AD rechazados con su mensaje; un texto de solo caracteres invisibles se rechaza como "no permitido" y no como vacío; los saltos de línea (`\n`, `\r\n`) se aceptan en la descripción y se reducen a un espacio en los nombres.
- **Interfaz:** un nombre de producto, categoría o unidad con HTML (`<img src=x onerror=…>`) se muestra como texto y no crea ningún elemento; las búsquedas, la vista previa y el Conversor esperan 300 ms y descartan respuestas viejas; un doble clic en un botón de guardar produce una sola llamada.
- **Purga best-effort:** si la purga falla (por ejemplo, con un trigger de prueba que haga fallar el `DELETE`), el listado **sí se devuelve**, sin los productos vencidos (excluidos en la consulta), y el error queda registrado en `errorLogs`; la interfaz muestra el aviso "No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación". Si el propio `SAVEPOINT` de la purga falla (caso no soportado, transacción irrecuperable), el comando responde `internal` y **no** devuelve el listado.
- **Teclado y foco (Q-06):** el buscador de categoría y de unidad se opera con flechas, Enter y Escape; los diálogos atrapan el foco, se cierran con Escape y lo devuelven al elemento que los abrió.
- **Doble coincidencia en unidades (Q-07):** con una unidad cuyo nombre y otra cuya abreviación coinciden exactamente con lo escrito, el buscador muestra ambas y no ofrece "Agregar nueva unidad".
- **Fechas imposibles (Q-05):** un timestamp con mes 13, hora 25 o `02-30` es rechazado por el esquema aunque tenga el patrón `YYYY-MM-DDTHH:MM:SSZ`.
- **Auditoría al eliminar (Q-02):** eliminar una categoría o unidad actualiza `updatedAt` de los productos que quedaron sin ella.
- **Aviso de purga sin cifra:** si falla la purga y también el conteo de vencidos, el aviso se muestra sin número y el comando no se aborta.
