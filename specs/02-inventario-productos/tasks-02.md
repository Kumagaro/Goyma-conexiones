# Tareas — Categorías, Unidades de Medida y Productos (`inventario-productos`)

- **Basado en:** `spec.md` y `plan.md` (esta misma carpeta; hoy `spec-2.md` y `plan-2.md`), la revisión de interfaz de `plan.md` §7 (FE-*, T-*, D-*) y la trazabilidad C-01 a C-10 de `plan.md` §10, `Constitution.md` v1.2.0 y `Agents.md`.
- **Estado:** **Confirmado / listo para implementar.** El dueño confirmó las decisiones de `plan-2.md` §10 (las 8 originales más las 10 de la revisión QA: cantidad inicial > 0, sin adaptador de facturas ni CSP, purga best-effort, columnas de auditoría, límite de dígitos por campo, costo vigente en la vista previa, NFC/NFD sin dependencia, notificaciones tipo toast como PEN-14) y las 8 de la segunda revisión QA (Q-01 a Q-12, decisiones 19 a 26), que el dueño delegó en el criterio del auditor. Quedan por ejecutar las verificaciones de entorno de la §0 (`chrono`, SQLite, helpers de test).
- **Enfoque:** vertical (por funcionalidad, de punta a punta: test de Rust → comando → servicio/vista JS), igual que `tasks-01.md`. Primero la infraestructura compartida (§1), después Categorías, Unidades, Conversiones, Precios, Productos y Dashboard.
- **Convención de nombres:** los comandos se nombran en `camelCase` (nombre lógico / wrapper JS); la función Rust es su equivalente `snake_case` (`createProduct` → `create_product`). Sin `#[allow(non_snake_case)]`.
- **Estructura de los comandos:** cada comando es un envoltorio delgado (lock + `now` + [`today`] + llamada + registro de error) sobre `xxx_inner(conn: &mut Connection, now, [today,] …)`. `today` lo calcula el envoltorio con `chrono::Local::now().date_naive()`; **no es un argumento que envíe el frontend** (Q-11). **Los tests de Rust ejercitan la función interna** sobre `setup_test_db()`. Los casos con cantidad 0 (agotados) o con una cantidad exacta se crean **solo con `insert_product_fixture(...)`** (inserción directa en la BD de pruebas), nunca con comandos ni con código de producción que altere la cantidad; `createProduct` solo crea con la cantidad inicial indicada (> 0).
- **Granularidad:** una tarea de test por requisito, que cubre todos sus escenarios Dado/Cuando/Entonces de la spec. Cuando una tarea agrupa más de unas 8 reglas, se divide en **sub-casillas** (`- [ ]` anidadas) por grupo de reglas, y la tarea padre solo se marca al completarlas todas (M-08, Q-03): así no se puede dar por hecha con cobertura parcial.
- **Garantía sobre lo implementado:** no se reabre ninguna tarea marcada [x] de `tasks-01.md`. `should_log()` no cambia (`invalidCredentials` y `validationFailed` siguen sin registrarse). `errors.rs`, `validators.rs`, `require_session`, `setup_test_db` y el patrón `xxx_inner` solo se reutilizan o se **amplían**. No se editan `auth.css` ni `dashboard.css`. Los formatos de fecha de la spec 1 no se tocan.
- **Orden dentro de cada tarea de código:** Test → Implementación → Verificación. Cada test debe fallar antes de implementar el código que lo satisface (`Constitution.md` §2.III), y ninguna implementación hace trampa para pasarlo.
- **Registro de comandos:** cada comando se añade a `generate_handler!` **en el mismo commit** en que se implementa. Son **26 comandos nuevos** (15 existentes + 26 = **41** al final): 6 de `categories.rs`, 5 de `units.rs`, 5 de `conversions.rs`, 9 de `products.rs` y 1 de `dashboard.rs`.
- **Matriz de autorización (`Agents.md` §3):** cada comando trae su fila en la misma tarea que lo implementa: sin sesión → `sessionExpired`; con Administrador y con Contador → permitido. La verificación de que ninguno quedó fuera está en la §9.
- **Nota general:** ninguna tarea se marca completa sin verificar que el código funciona de verdad, no solo que compila o que el test pasa (`Agents.md` §6). El ítem "Actualizar documentación" no se repite en cada sección: se consolida en la §9.
- **Nota de contexto (estado de la interfaz):** hoy Categorías, Unidades, Conversiones, Productos y la tarjeta de agotados del dashboard trabajan con datos simulados en `dashboard.js`. Cada sección del módulo elimina **su** mock en el mismo bloque en que conecta la vista real, para que nunca estén activas a la vez la lógica simulada y la real compitiendo por los mismos elementos del DOM. El resto de `dashboard.js` (ventas, clientes, notificaciones, configuración) queda fuera de alcance; la sección de facturas del dashboard se **oculta** hasta su spec (§6.10).

---

## 0. Verificaciones previas y preparación

*(Nada de esto crea funcionalidad; evita arrancar sobre supuestos sin comprobar.)*

- [x] Decisiones de `plan-2.md` §10 confirmadas por el dueño (las 18: las 8 originales, la cantidad inicial, la sección de facturas oculta sin adaptador, PRAGMA, M-06/M-07 y las 6 de la revisión QA). Pendiente solo mover `plan-2.md` a `plan.md` y `spec-2.md` a `spec.md` dentro de `specs/inventario-productos/`.
- [x] Línea base: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check` y `npm test` pasan **antes** de tocar nada (referencia de `tasks-01.md`: 97 tests Rust y 53 Vitest). Números reales de partida verificados: `cargo test` = 97 pruebas Rust OK en 3.11s; `npm test` = 53 tests Vitest OK en 39.97s; `cargo clippy` y `cargo fmt -- --check` no emitieron warnings ni fallos.
- [x] Verificar en `Cargo.toml` (no vino en los documentos revisados) que `chrono` conserva `default-features` o la feature `clock`, para que `chrono::Local` compile (C-04). **No se añade ninguna dependencia.** Resultado verificado: `chrono = { version = "0.4", features = ["serde"] }` conserva `default-features`, incluido `clock`; `tauri = { version = "2", features = [] }` no requiere cambios.
- [x] Verificar que el SQLite incluido por `bundled` admite índices parciales y sobre expresiones (`CREATE UNIQUE INDEX … ON t (min(a,b), max(a,b))` y `… WHERE status = 'active'`) con un script de prueba descartable.
- [x] Verificar el texto exacto del error de SQLite al violar el índice sobre expresiones (`UNIQUE constraint failed: index 'unitConversionsPair'`) y al violar el índice parcial de productos (`products.nameNormalized`). **El formato entre paréntesis es un supuesto**: el spike lo confirma; si el formato real difiere (por ejemplo, sin comillas alrededor del nombre del índice), el test de la §1.4 se ajusta sin cambiar su intención. Se fija en un test en la §1.4; aquí solo se anota.
- [x] **Spike de `tauri::test` (A-03, `plan.md` §1 verificación previa 4):** prueba descartable con `tauri::test::mock_builder()` y un comando de ejemplo con `#[serde(deny_unknown_fields)]`, enviándole un campo extra y un argumento faltante con `tauri::test::get_ipc_response`. **Anotado en `plan-02.md` §1** el texto exacto que recibe el frontend en cada caso y si el comando llegó a ejecutarse; ese patrón alimenta la clasificación de `invoke.js` (§1 bis) y el test de contrato. Los nombres de la API se confirman contra la versión instalada de `tauri`.
- [x] Verificar los helpers de test de la spec 1: `assert_requires_session` y `assert_requires_admin`. Anotar su firma. Si solo comprueban "sin sesión" y "sesión vencida", ver la tarea de `assert_allows_both_roles` de la §1.6.
- [x] Verificar que `init_schema` puede añadir `createdAt` y `updatedAt` a las tablas nuevas (`categories`, `units`, `products`, `unitConversions`) sin conflicto (son tablas nuevas, no hay migración; si la BD de desarrollo ya las creó sin esas columnas, borrarla). Anotar el formato exacto del `GLOB` que valida el timestamp.
- [x] Verificar el `data-page` real del enlace de facturas en el sidebar: `grep -n 'data-page' dashboard-unificado.html`. El valor esperado es `facturas`, pero puede ser `facturacion` u otro. **Valor real verificado**: no hay un enlace con `data-page="facturas"` en el sidebar del proyecto; el valor que existe en el HTML actual es el del menú principal de productos, y la ocultación de la sección de facturas queda documentada como `hidden` sin adaptador. Se ha anotado este hallazgo en `plan-02.md` §5/§7.7.
- [x] **Inventario de `dashboard.js` y `dashboard-unificado.html`** — levantado contra los archivos reales. **Sin números de línea** (se desplazan con cualquier edición; el `grep` es la fuente de verdad). Resultado:
  - **Globals del mock a eliminar:** `categoriasData`, `nextCategoriaId`, `editingCategoriaId`; `unidadesData`, `nextUnidadId`, `editingUnidadId`; `conversionesData`, `nextConversionId`, `editingConversionId`; `productosData`, `nextProductoId`, `editingProductoId`; `INVENTARIO_EJEMPLO` (dentro de un IIFE del dashboard); `productosDisponibles` (arreglo de **nombres**). **Todos se eliminan**, incluidos `productosData` y `productosDisponibles`: la sección de facturas se oculta hasta su spec. Orden: cada sección borra solo sus funciones; los arreglos `categoriasData`, `unidadesData` y `conversionesData` se borran juntos en la última tarea de §6.10 (A-02).
  - **Funciones por módulo a eliminar:**
    - Categorías: `showCategoriaTab`, `verCategoria`, `editarCategoriaDesdeVer`, `eliminarCategoriaDesdeVer`, `cancelarCategoriaForm`, `renderCategorias`, `renderProductPicker`, `agregarProductoPicker`, `quitarProductoPicker`, `editarCategoria`, `eliminarCategoria`, `guardarCategoria`.
    - Unidades y conversiones: `buscarUnidad`, `nombresUnidad`, `opcionesUnidadHTML`, `obtenerFactorConversion`, `formatearNumeroUnidad`, `renderUnidades`, `renderConversiones`, `abrirModalUnidad`, `cerrarModalUnidad`, `guardarUnidad`, `eliminarUnidad`, `poblarSelectoresModalConversion`, `actualizarPreviaConversion`, `abrirModalConversion`, `cerrarModalConversion`, `guardarConversion`, `eliminarConversion`, `poblarSelectoresConversor`, `actualizarConversorRapido`, `invertirConversorRapido`.
    - Productos: `actualizarProductosDisponibles`, `poblarSelectCategoriasProducto`, `poblarSelectUnidadesProducto`, `agregarUnidadRapida`, `irCrearCategoria`, `irCrearProducto`, `toggleCaducidadProducto`, `showProductoTab`, `prepararNuevoProducto`, `cancelarProductoForm`, `editarProducto`, `actualizarGananciaProducto`, `eliminarProducto`, `guardarProducto`, `formatearIdProducto`, `renderProductos`, `verProducto`, `editarProductoDesdeVer`, `historialProductoDesdeVer`, `eliminarProductoDesdeVer`.
    - Facturas (consumidoras del mock): `poblarSelectHistorialProducto`, `obtenerMovimientosProducto`, `agregarLineaVenta`, `actualizarProductosDisponibles` y la actualización de línea de venta. **La sección de facturas se oculta (`hidden`) hasta su spec.**
  - **Llamadas a retirar de `showPage()`:** `renderProductos()`, `renderCategorias()` y, en `unidades`, `renderUnidades()` + `renderConversiones()` + `actualizarConversorRapido()`. Las demás secciones (clientes, proveedores, notificaciones, configuración) no se tocan. `poblarDatalists()` y `actualizarProductosDisponibles()` se revisan al migrar (ubicarlas con `grep`).
  - **Paginador global:** `PAGINABLES` ya incluye `productos-body`, `categorias-body`, `unidades-body` y `conversiones-body`. Se engancha con un **`MutationObserver` sobre los hijos del `<tbody>`** (`iniciarPaginacion`): al repintar las filas se repagina solo y vuelve a la página 1. **No existe función de refresco**: las vistas nuevas solo deben escribir sus filas en esos mismos `<tbody>`. La tabla de la pestaña P-13 se añade a `PAGINABLES` (decisión confirmada, tarea de §6.10).
  - **Dashboard:** `#dash-agotados-total`, `#dash-agotados-categorias` y `#dash-agotados-sin-categoria` existen. `renderInventario(inv)` espera `categorias` como **arreglo de nombres** y la nueva tarjeta devuelve `{name, count}`: hay que adaptar ese render. `obtenerDatosDashboard` y `obtenerInventarioDashboard` están dentro de un IIFE y devuelven `DATOS_EJEMPLO` / `INVENTARIO_EJEMPLO` (KPIs, Mejor Vendidos y Lento Movimiento dependen de ventas: estado vacío, no datos reales). Los botones "Ver Agotados" y "Ver Descontinuados" son **dos `onclick` inline idénticos** que solo llaman a `showPage('productos', …)`, sin filtro.
  - **Pestañas de Productos:** `showProductoTab(tab, btn)` activa `#producto-<tab>` (`list`, `create`, `historial`) y muestra `#producto-search-bar` solo en `list`; los botones de pestaña son inline. La pestaña **Agotados y Descontinuados no existe**: hay que crear `#tab-producto-agotados` y la vista `#producto-agotados` (sección `.config-subview`) y su rama `'agotados'`.
  - **IDs verificados en el HTML (existen):** `#producto-search`, `#producto-precio-venta`, `#producto-ganancia`, `#producto-precio-entrada`, `#producto-stock`, `#producto-categoria`, `#producto-unidad`, `#producto-caducidad-tipo`, `#producto-caducidad-fecha`, `#categoria-ver`, `#categoria-descripcion`, `#categoria-productos-picker`, `#modal-unidad`, `#modal-unidad-abreviacion`, `#modal-conversion`, `#modal-conversion-factor`, `#modal-conversion-preview`, `.conversor-card`, `#productos-body`, `#categorias-body`, `#unidades-body`, `#conversiones-body`, `#reserved-data-warning`. **No existen y se crean:** `#tab-producto-agotados`, `#producto-agotados`.
  - **Elementos de la spec 1 en el HTML:** `usuarios.js` y `perfil.js` como `<script type="module">` y `dashboard.js` como script clásico.
  - **Manejadores inline:** el HTML tiene unos 226 atributos `onclick`/`oninput`/`onchange`/`onsubmit` en total; el subconjunto de Categorías, Unidades, Conversiones, Productos y los dos botones del dashboard se lista y reemplaza por listeners en la tarea de interfaz de cada sección (con `grep` sobre el bloque de `#page-productos`, `#page-categorias` y `#page-unidades`).
  - **Cadenas con codificación dañada (T-08):** se corrigen al migrar cada función (por ejemplo "Categorñas", "Lí­neas" con guion blando); no hace falta un barrido previo.
  - **Verificación con `grep`:** antes de tocar cada sección, ejecutar `grep -n` de los identificadores listados para localizarlos. El `grep` es la fuente de verdad, no los números de línea.
  - Sin estilos nuevos: se reutilizan las clases existentes; si hiciera falta una regla CSS, se documenta en `plan.md` §7 **antes** de implementar.
- [x] Revisar `services/invoke.js` y su test actual: anotar el comportamiento vigente ("devuelve intactas respuestas/errores de otros códigos") que debe seguir en verde.
- [x] Revisar el test de contrato `invoke` ↔ `generate_handler!` (`environment.test.js`): confirmar que el recuento esperado se calcula a partir de los archivos y no está fijo en 15; si está fijo, se actualiza en la §8.

## 1. Infraestructura compartida (backend)

*(Se crea antes del primer comando: todos los módulos las usan. Orden: Test → Implementación por pieza.)*

### 1.1 `text.rs` — texto, normalización y orden (7.1, 7.3, C-02)

- [x] Test (Rust, sobre las funciones puras):
  - `clean_name`: recorta extremos y reduce **cualquier** secuencia de espacios Unicode (incluidos saltos de línea y tabulaciones) a un solo espacio.
  - `clean_description`: conserva los saltos de línea, reduce espacios y tabulaciones repetidos dentro de cada línea, recorta la línea y el conjunto; vacío tras el recorte → `None`.
  - `normalize_text`: `Camión`, `CAMIÓN` y `camion` dan lo mismo; `año` ≠ `ano`; `Ñ→ñ`; `á é í ó ú ü → a e i o u u`; otros acentos o signos se conservan tras pasar a minúsculas; espacios internos colapsados; **sin** NFC/NFD (una letra descompuesta no se equipara con la compuesta, limitación aceptada).
  - `spanish_sort_key` / `sort_spanish`: `nuez` < `ñame` < `oso`; `n` < `ñ`; `Ángel` queda con la A; a igualdad de clave, por ID.
  - Largos en puntos de código (`chars().count()`), no en bytes: un nombre con `ñ` o emoji cuenta como caracteres.
- [x] Implementación de `text.rs` (`clean_name`, `clean_description`, `normalize_text`, `spanish_sort_key`, `sort_spanish` con `sort_by_cached_key`); declarar el módulo en `lib.rs`. **Ninguna consulta ordena por nombre en SQL.**

### 1.2 `timestamps.rs` — formato fijo de fechas (C-10)

- [x] Test (Rust): `format_utc_timestamp` produce `YYYY-MM-DDTHH:MM:SSZ` (segundos, sufijo `Z`, sin fracción, truncando); `parse_utc_timestamp` es su inversa (ida y vuelta); rechaza el formato con espacio y fracción que escribe el `ToSql` de `rusqlite` y las fechas imposibles (`2026-13-01T00:00:00Z`, `2026-02-30T00:00:00Z`) devolviendo `None`; quien lo lee y recibe `None` (fila corrupta, inalcanzable con el `CHECK` del esquema) responde `internal`, nunca un valor inventado (Q-05).
- [x] Implementación de `timestamps.rs`. Las columnas nuevas (`discontinuedAt`, `notifications.createdAt` y los `createdAt`/`updatedAt` de las tablas nuevas) se escriben **siempre como `String` con este helper**; no se usa el `ToSql`/`FromSql` de `DateTime` en ellas.

### 1.3 `validators.rs` — nombres, números y fechas (7.3, 7.4, 7.5)

- [x] Test (Rust, `validate_name` / `validate_description`): devuelven el valor **ya limpio y su versión normalizada**; rangos por entidad (categoría 1–50 / descripción hasta 200; unidad 1–30 / abreviación 1–10; producto 2–100 / descripción hasta 500); vacío tras el recorte rechazado; el texto se guarda tal como se escribió (mayúsculas y acentos) tras el recorte; los mensajes son los del catálogo de `plan.md` §4.
- [x] Test (Rust, `parse_scaled` y los `parse_*`), una sub-casilla por grupo de reglas:
  - [x] Formato: coma y punto como decimal; `007` aceptado; `0.123` aceptado; `1.234` rechazado por ambigüedad (con el texto "usa coma"); `,5`, `5,`, `1e3`, `+5`, `-5`, espacios y vacío rechazados por formato.
  - [x] Exceso de decimales por campo (costo 2 / porcentaje 2 / cantidad inicial 3 / factor 6 / cantidad del Conversor 6).
  - [x] **Orden de evaluación de 7.4** (formato → ambigüedad → decimales → rango: se informa el primero que falle, con un caso por cada par de reglas que se pisan).
  - [x] **El máximo de dígitos de la parte entera lo define cada campo** y `parse_scaled` lo recibe como parámetro (`max_entero_digitos`: costo 8, cantidad inicial 9, factor 15, cantidad del Conversor 15, porcentaje 6); una parte entera con más dígitos que el máximo del campo = fuera de rango sin desbordar.
  - [x] Topes con su frontera: costo (0 rechazado, 0,01 y 999.999,99 aceptados, 1.000.000 rechazado); porcentaje (0 y 1000,00 aceptados, 1000,01 rechazado); cantidad inicial (0 rechazado, 0,001 y 999.999,999 aceptados, 1.000.000 rechazado, 0,0001 por decimales); factor (0,000001 y 999.999.999,999999 aceptados, 0,0000001 por decimales, 1.000.000.000 rechazado); cantidad del Conversor (0 rechazado, tope aceptado).
  - [x] Los valores devueltos son enteros escalados (centavos, centésimas, millonésimas), nunca `f64`.
- [x] Test (Rust, `validate_expiry_date(texto, today, actual)`) con `today` inyectado: hoy válido; ayer rechazado; hoy + 29 y hoy + 30 aceptados (la frontera de *filtro* se prueba en la §6); `2026-02-30` y `2026-2-3` (formato) rechazados; año 2101 rechazado y 2100-12-31 aceptado; una caducidad **vencida que no se modifica** se acepta (`actual` igual al texto).
- [x] Implementación en `validators.rs` (se **amplía**, sin tocar lo existente): `validate_name`, `validate_description`, `parse_scaled`, `parse_cost_cents`, `parse_percent_hundredths`, `parse_quantity_milli` (cantidad inicial), `parse_factor_micro`, `parse_quantity_micro`, `validate_expiry_date`. `parse_scaled` es el único lector de números de la feature y recibe `max_entero_digitos` como parámetro (sin número mágico).

### 1.4 `errors.rs` — catálogo y traducción de errores (C-08)

- [x] Test (Rust): se añade el código `invalidState` serializado como `{ code, message }` sin `cause`; **los códigos existentes se serializan igual que antes** (test de regresión); `should_log()` sigue siendo `false` para `invalidCredentials` y `validationFailed` y `true` para `duplicateField`, `notFound`, `invalidState` e `internal`.
- [x] Test (Rust, `map_db_error(error, operación, duplicados)`): una violación `UNIQUE` real de SQLite se traduce a `duplicateField` con el mensaje correspondiente, **un test por columna/índice** (`categories.nameNormalized`, `units.nameNormalized`, `units.abbreviationNormalized`, `products.nameNormalized` con el índice parcial e `index 'unitConversionsPair'`) — esto protege contra un cambio de formato del mensaje de SQLite; una violación de `CHECK` o de clave foránea inesperada → `internal`; el `cause` indica módulo, operación y nombre de restricción y **nunca** valores ingresados (se fuerza un error con un nombre de producto reconocible y se comprueba el texto del log).
- [x] Test (Rust): el campo `expiredCleanupWarning` de los DTOs de respuesta es `null` cuando la purga funciona y contiene el texto "No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación" (con el N real) cuando falla; se serializa en `camelCase`.
- [x] Implementación: `invalidState` en el catálogo, `map_db_error` con tabla `(columna → mensaje)` que cada comando pasa, y los textos exactos de `plan.md` §4 como constantes en un solo lugar. Los textos de la spec 1 no se tocan.

### 1.5 Esquema (`db/schema.rs`, `plan.md` §2)

- [x] Test (Rust, SQL puro sobre el esquema recién creado, **un test por cada `CHECK`, `UNIQUE` y clave foránea, cada uno con un valor que fallaría por `NULL`** donde aplique):
  - [x] `categories`: `nameNormalized` repetido falla (`Camión` / `camion`); longitudes de `name` y `description`; `description` vacía falla (se guarda `NULL`).
  - [x] `units`: `nameNormalized` y `abbreviationNormalized` repetidos fallan por separado; longitudes.
  - [x] `unitConversions`: `fromUnitId = toUnitId` falla; `factorMicro` 0 y 10^15 fallan, 1 y 999999999999999 pasan; el par repetido falla **en ambos sentidos** (A→B y luego B→A); `ON DELETE CASCADE` al borrar cualquiera de las dos unidades.
  - [x] `products`: `costCents` 0 y 100000000 fallan; `profitPercentHundredths` −1 y 100001 fallan, 0 y 100000 pasan; `quantityMilli` −1 y 1000000000 fallan, 0 y 999999999 pasan, **sin `DEFAULT`** (un `INSERT` sin cantidad falla); `expiresOn` con formato malo, `2026-02-30` y `2101-01-01` fallan, `NULL` y una fecha válida pasan; `status` fuera de `active`/`discontinued` falla; `CHECK` de estado/fecha: activo con `discontinuedAt` falla, descontinuado **sin** fecha falla, descontinuado con fecha de formato incorrecto falla, descontinuado con el patrón exacto pasa; **índice parcial**: dos activos con el mismo `nameNormalized` fallan, un descontinuado con el mismo nombre que un activo pasa; `ON DELETE SET NULL` en `categoryId` y en `unitId`.
  - [x] **Auditoría** — `categories`, `units`, `unitConversions` y `products`: `createdAt` y `updatedAt` con formato incorrecto fallan (cada uno por separado, incluido uno que fallaría por `NULL`) y con el formato correcto pasan; **las fechas imposibles que sí cumplen el patrón (mes 13, hora 25, `02-30`) también fallan** (`strftime` ida y vuelta, Q-05), igual que en `discontinuedAt` y `notifications.createdAt`; `updatedAt` se actualiza al editar y `createdAt` no (test de comando en cada sección: categoría, unidad, factor de conversión, producto, descontinuar y activar).
  - [x] `notifications`: `createdAt` con formato incorrecto falla; `isRead` fuera de 0/1 falla; `type` y `priority` **no** tienen `CHECK` de lista (insertar un tipo arbitrario pasa, a propósito).
  - [x] `AUTOINCREMENT`: tras eliminar la fila con el ID más alto de cada tabla, el siguiente ID **no** reutiliza el anterior.
  - [x] `init_schema` ejecutado dos veces no falla, **no inserta datos semilla** en estas tablas (7.8) y no altera las tablas de la spec 1.
  - [x] **Arranque sin purga:** con un descontinuado vencido insertado como fixture, ejecutar la secuencia de arranque (`init_schema` + limpieza de sesiones + purga de `errorLogs`) lo deja intacto (P-08).
- [x] Implementación: ampliar `init_schema` con las tablas, índices y `CHECK` de `plan.md` §2 (todas con `CREATE TABLE IF NOT EXISTS` / `CREATE … INDEX IF NOT EXISTS`). Incluye `createdAt` y `updatedAt` (formato de §3.3, con `GLOB` más `strftime` ida y vuelta, Q-05) en las cuatro tablas que `plan.md` §2 señala. Como son tablas nuevas, **no hace falta borrar la BD de desarrollo** (salvo que ya se hubieran creado sin las columnas de auditoría). `PRAGMA foreign_keys = ON` ya está activo. Sin índices adicionales (C-06).

### 1.6 Fixtures y helpers de test

- [x] Crear `insert_category_fixture`, `insert_unit_fixture`, `insert_conversion_fixture(conn, fromUnitId, toUnitId, factorMicro) -> i64` (`INSERT` directo con `createdAt`/`updatedAt`; los tests de `listUnits` (§3.2) y `deleteUnit` (§3.4) la necesitan antes de que exista `createConversion` en §4.1, Q-01) e `insert_product_fixture(conn, name, categoryId, unitId, costCents, profitHundredths, quantityMilli, status, discontinuedAt, expiresOn) -> i64` (insertan filas válidas por `INSERT` directo, calculando `nameNormalized` con `normalize_text`). Son el **único** camino para crear productos con cantidad 0 o con una cantidad exacta en los tests (`createProduct` solo crea con la cantidad inicial indicada). Incluyen `createdAt` y `updatedAt` con `now` fijo.
- [x] Crear `assert_allows_both_roles(inner)` (o ampliar el helper de la spec 1 **sin cambiar su comportamiento actual**): con sesión de Administrador y de Contador el comando no devuelve `sessionExpired` ni `forbidden`. Cada comando lo usa junto con `assert_requires_session`.
- [x] Helper de trigger de prueba (`RAISE(ABORT)`) para forzar fallos de `DELETE` sobre `products` y de `INSERT` sobre `notifications`; vive solo bajo `#[cfg(test)]`.

### 1.7 Notificaciones mínimas (`db/notifications.rs`, NT-01)

- [x] Test (Rust): `insert_notification(conn, type, priority, message, now)` guarda la fila con `createdAt` en el formato de C-10 e `isRead = 0`; el armado de la lista de nombres respeta NT-01: orden alfabético español **antes** de truncar, separados por coma, 1 / 20 / 21 nombres (21 → "…, y 1 más", con 25 → "y 5 más"), el total real se informa aunque se trunque; ninguna notificación si no hay afectados.
- [x] Implementación de `notifications.rs` (`insert_notification` y la función que arma el mensaje de N-08/N-09 con los textos "Los siguientes productos quedaron sin categoría: …" / "…sin unidad de medida: …", prioridad `ADVERTENCIA`).

### 1.8 Purga de productos vencidos (`purge_expired_products`, C-06, P-08)

- [x] Test (Rust, con fixtures de fecha y `now` inyectado): un descontinuado de **59 d 23:59:59** no se purga; de **60 d exactos** sí; de **60 d + 1 s** sí; un activo nunca se purga; devuelve cuántos eliminó.
- [x] Test (Rust, **purga best-effort**): si la purga falla (trigger `RAISE(ABORT)` en `DELETE` sobre `products`), el comando **no aborta**: devuelve el listado **sin los vencidos** (excluidos en la consulta por la consulta), incluye `expiredCleanupWarning` con el texto "No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación" y registra el error en `errorLogs` (`cause` sin nombres de productos); tras el fallo la transacción del comando sigue utilizable (la purga corre en un `SAVEPOINT` con `ROLLBACK TO`); con N = 0 vencidos el aviso es `null`, y `expiredCleanupWarning` es `null` cuando la purga funciona. Se verifica en cada comando que purga con la tabla de §6.6.
- [x] Test (Rust, **conteo fallido**, Q-10): si falla la purga **y también** la consulta que cuenta los vencidos (por ejemplo, una vista de prueba que rompa el conteo), el comando no aborta, el aviso llega **sin cifra** ("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación") y se registran ambos errores sin valores ingresados.
- [x] Test (Rust, **`SAVEPOINT` fallido**, si es forzable de forma determinista): omitido; no se pudo forzar este fallo de manera determinista en la BD de test sin simular una transacción ya rota. Se cubre con la verificación manual de abajo.
- [x] Verificación manual (**`SAVEPOINT` fallido**): por inspección de `purge_best_effort`, un fallo al abrir el `SAVEPOINT` se transforma en `internal`, se registra, se propaga antes de cualquier lectura/listado y deja que la transacción exterior se revierta al abandonar su guard. Caso no soportado porque implica una transacción irrecuperable.
- [x] Implementación en `commands/products.rs`: `purge_expired_products(conn, now) -> Result<usize, AppError>` con `DELETE FROM products WHERE status = 'discontinued' AND discontinuedAt <= ?` y umbral `format_utc_timestamp(now − 60 días)`, y el helper `purge_best_effort(conn, now) -> Option<String>` que la ejecuta en un `SAVEPOINT`, captura y registra el fallo, cuenta los vencidos (N) y devuelve el aviso si N > 0. Es la **única** función de purga. Si falla, las consultas del comando añaden `AND (status <> 'discontinued' OR discontinuedAt > ?)` con el umbral de 60 días. Convención de uso (plan §3.6): se llama tras `require_session` y **antes** de contar, listar o actuar, dentro de una **única transacción** con lo que viene después; `require_session` se resuelve antes de abrirla. `previewSellingPrice` no la llama.

### 1.9 Modelos / DTOs

- [x] Crear `models/category.rs`, `unit.rs`, `conversion.rs` y `product.rs` con los DTOs de `plan.md` §4 (todos con `#[serde(rename_all = "camelCase")]`; los structs de entrada con `#[serde(deny_unknown_fields, rename_all = "camelCase")]`). Test (Rust): cada DTO serializa a las claves `camelCase` esperadas y los decimales viajan como texto con coma sin separador de miles ni ceros finales (porcentaje siempre con 2 decimales). Incluye los structs con aviso de purga (`CategoryList`, `UnitList`, `ProductConditionList`, `UpdateProductResult`, `ProductActionResult` y el campo `expiredCleanupWarning` de `CategoryDetail`, `DeleteCategoryResult`, `DeleteUnitResult` y `ProductDetail`): serializa a `null` cuando la purga funcionó y al texto del aviso cuando falló. Los comandos de §2, §3 y §6 devuelven estos structs (`plan.md` §4).

## 1 bis. Infraestructura compartida (frontend)

**Nota de dependencia:** no implementar `invoke.js` (el primer ítem de esta sección) hasta que el **spike de `tauri::test`** de la §0 esté hecho y el **patrón exacto del rechazo de argumentos** esté anotado en `plan-02.md` §1 (verificación previa 4). Sin ese patrón, la clasificación de errores sin forma `AppError` se implementaría sobre un supuesto y podría no distinguir el rechazo de Tauri de cualquier otro error.

- [x] Test (Vitest) de `services/invoke.js` ampliado (C-01, T-03): un texto con la forma del **rechazo de argumentos de Tauri** (patrón anotado en el spike de `tauri::test`) se convierte en "Los datos enviados no son válidos"; **cualquier otro** error sin forma `AppError` (texto plano distinto, excepción de IPC, comando no registrado) se convierte en "Ocurrió un error inesperado" (Q-09); ambos se escriben en `console.error` con el nombre del comando; los errores con forma `AppError` se devuelven **intactos**; **el test existente ("devuelve intactas respuestas/errores de otros códigos") sigue en verde sin modificarse**.
- [x] Implementación: ampliar `services/invoke.js` solo con esa clasificación (`isArgumentRejection` con el patrón del spike, Q-09). Sigue siendo el único punto con `window.__TAURI__.core.invoke`.
- [x] Test (Vitest) de `utils/formatters.js`: dinero (centavos → USD con dos decimales, `es-ES`), ID con relleno de ceros (`#00001`), fecha `dd/mm/aaaa`; la **caducidad se formatea por partes de la cadena, sin `Date`** (`2026-03-01` nunca se desplaza por zona horaria); el timestamp de descontinuación sí se convierte a hora local; días restantes. Los decimales llegados como texto no pasan por `Number` ni `parseFloat`.
- [x] Implementación de `utils/formatters.js`.
- [x] Test (Vitest + jsdom) de `components/confirmDialog.js`: usa `.modal-overlay` y `.modal-card`, acepta texto multilínea con conteos, devuelve una promesa que resuelve `true` al confirmar y `false` al cancelar o con Escape, y no deja nodos huérfanos; **foco (Q-06):** `role="dialog"` y `aria-modal="true"`, foco inicial en **Cancelar**, Tab y Shift+Tab ciclan dentro del diálogo y al cerrar el foco vuelve al elemento que lo abrió.
- [x] Implementación de `confirmDialog.js` (reemplaza cada `confirm()` de esta feature).

---

## 2. Categorías — CAT-01 a CAT-06

*(Orden: crear → listar → editar → detalle → eliminar, porque cada paso produce los datos del siguiente. CAT-05 —crear sobre la marcha— vive en la §6 con el combobox.)*

### 2.1 CAT-01 — Crear categoría y aviso de duplicado

- [ ] Test (Rust, sobre `create_category_inner` y `search_categories_inner`): creación con nombre único y descripción opcional (vacía → `NULL`); texto guardado tal como se escribió, `nameNormalized` rellenado por el backend; duplicado sin distinguir mayúsculas ni acentos (`Camión` / `camion` / `CAMIÓN`) → `duplicateField` con "Ya existe una categoría con ese nombre", decidido por el `INSERT`; `ñ` distinta de `n`; nombre vacío o de 51 caracteres → `validationFailed` con su mensaje; descripción de 201 caracteres rechazada; `searchCategories` busca con `instr()` sobre la columna normalizada (`%` y `_` literales, acentos ignorados, `ñ` ≠ `n`, texto vacío = sin filtro) y marca `isExactMatch` solo con igualdad exacta normalizada; el ID no se reutiliza tras eliminar.
- [ ] Test (Rust): **filas de la matriz de autorización** para `createCategory` y `searchCategories` (`assert_requires_session` + `assert_allows_both_roles`).
- [ ] Implementación: comandos `createCategory(name, description?)` y `searchCategories(text)` en `commands/categories.rs`; registrarlos en `generate_handler!`; `create_category` devuelve `CategoryListItem`. `searchCategories` no purga (no cuenta).

### 2.2 CAT-02 — Ver todas las categorías

- [ ] Test (Rust, sobre `list_categories_inner`): devuelve todas con `productCount` que **incluye descontinuados**; **purga antes de contar** (un descontinuado vencido no se cuenta y deja de existir); orden alfabético español (`Ángel` con la A, `ñame` entre `nuez` y `oso`, desempate por ID); búsqueda por nombre con la misma regla que `searchCategories`; con la purga fallida (trigger) **no aborta**: devuelve el listado sin vencidos con `expiredCleanupWarning` y registra el error (§1.8); estado vacío (lista vacía, sin datos semilla).
- [ ] Test (Rust): fila de la matriz de autorización de `listCategories`.
- [ ] Implementación: `listCategories(search?)`; la consulta ordena solo por `id` y el orden español se aplica en Rust con `sort_spanish`.

### 2.3 CAT-03 — Editar categoría

- [ ] Test (Rust, sobre `update_category_inner`): guardar sin cambios o cambiando solo la descripción funciona (la unicidad excluye la propia fila); nombre que coincide con otra categoría → `duplicateField` con el mismo mensaje de CAT-01; categoría inexistente → `notFound` "La categoría ya no existe"; mismas validaciones de texto que al crear.
- [ ] Test (Rust): fila de la matriz de autorización de `updateCategory`.
- [ ] Implementación: `updateCategory(categoryId, name, description?)`; la unicidad la decide el `UPDATE`.

### 2.4 CAT-06 — Detalle de categoría

- [ ] Test (Rust, sobre `get_category_inner`): devuelve nombre, descripción, `productCount` y `products: ProductBrief[]` (id, nombre, `isOutOfStock`, `isDiscontinued`) con **activos y descontinuados**, en orden español; purga antes de contar y listar; no incluye ingresos; categoría inexistente → "La categoría ya no existe".
- [ ] Test (Rust): fila de la matriz de autorización de `getCategory`.
- [ ] Implementación: `getCategory(categoryId)`.

### 2.5 CAT-04 + NT-01 (N-09) — Eliminar categoría

- [ ] Test (Rust, sobre `delete_category_inner`, productos creados con `insert_product_fixture`): sin productos → se elimina y **no** hay notificación; con productos (activos y descontinuados) → el campo `categoryId` de cada uno queda `NULL` (por la FK, no por lógica del comando), se inserta **una** notificación `N-09` / `ADVERTENCIA` con todos los afectados (formato y límite de 20 de NT-01) y se devuelve `affectedProducts` con el total real; **`updatedAt` de cada afectado queda en `now`** (lo escribe un `UPDATE` explícito, no la FK) y el de las demás filas no cambia (Q-02); los afectados se leen **antes** del `DELETE`; la purga se ejecuta primero (un descontinuado vencido no cuenta ni se lista); categoría inexistente → "La categoría ya no existe"; **atomicidad**: con un trigger de prueba que haga fallar el `INSERT` en `notifications`, la categoría y los vínculos de los productos quedan intactos; el `cause` de un log nunca incluye nombres de productos.
- [ ] Test (Rust): fila de la matriz de autorización de `deleteCategory`.
- [ ] Implementación: `deleteCategory(categoryId)` en la transacción única de `plan.md` §3.7 (purga → existe → leer afectados → insertar notificación si hay → `UPDATE` de `updatedAt` de los afectados → `DELETE` → `commit`).

### 2.6 Interfaz de Categorías (FE-C-01 a FE-C-07)

- [ ] Test (Vitest + jsdom, **antes** de la implementación) de `categoriesService.js`: cada wrapper `camelCase` llama a su comando `snake_case` con los argumentos correctos (el nombre `snake_case` solo aparece aquí).
- [ ] Test (Vitest + jsdom) de `categoriasView.js` con el servicio sustituido por un stub **solo en el test**: pinta las filas devueltas por `listCategories` y ninguna otra (sin datos de ejemplo); estados de carga, vacío ("Aún no hay categorías") y error; **no** muestra la columna Ingresos (atributo `hidden`); mientras se escribe el nombre en el alta muestra las coincidencias como aviso de duplicado e indica "ya existe" si hay coincidencia exacta; no hay botón "agregar" adicional; el mensaje de duplicado del backend aparece en pantalla (no `alert()`); eliminar abre `confirmDialog` con el conteo de productos del último listado y, al terminar, muestra el resultado con el número **real** devuelto; el detalle muestra nombre, descripción, cantidad y lista de productos por id.
- [ ] Implementación: `services/categoriesService.js` y `views/categoriasView.js` (dueño único de la sección Categorías, incluido el detalle); `maxlength` nombre 50 y descripción 200 solo como ayuda; descripción **opcional**; eliminar el bloque "Listado de Productos \*" (`#categoria-productos-picker`, FE-C-02) y su dependencia de `productosDisponibles`; ocultar con `hidden` la columna y el KPI de **Ingresos** (D-02, PEN-10); orden y búsqueda del backend; corregir cadenas dañadas (T-08).
- [ ] Eliminar de `dashboard.js` el mock de Categorías (sus funciones: `renderCategorias`, `guardarCategoria`, `eliminarCategoria`, `eliminarCategoriaDesdeVer`, `verCategoria`, picker de productos y todas sus llamadas, incluidas las de `showPage()`), y los `onclick` inline correspondientes del HTML. **No se borra aquí el arreglo `categoriasData`** (ni sus contadores): el formulario de producto aún lo lee; se borra con los demás arreglos en la última tarea de §6.10 (A-02). Verificación: `grep` de esas funciones en `dashboard.js` no devuelve nada; el dashboard carga sin errores en consola; la sección Categorías muestra el estado vacío con la BD vacía.
- [ ] Verificación manual/UI: crear, editar y eliminar una categoría real desde la app; el duplicado muestra el mensaje del backend; eliminar con productos asignados (cuando existan, §6) informa el conteo.

---

## 3. Unidades de medida — UM-01 a UM-04

### 3.1 UM-01 — Crear unidad y aviso de duplicado

- [ ] Test (Rust, sobre `create_unit_inner` y `search_units_inner`): creación válida (nombre 1–30, abreviación 1–10); duplicado de nombre y duplicado de abreviación (sin distinguir mayúsculas ni acentos) → `duplicateField` con el mensaje que indica **cuál** de los dos ("Ya existe una unidad con ese nombre" / "…con esa abreviación"), deducido de la columna que reporta SQLite; campos vacíos o fuera de largo → `validationFailed`; `searchUnits` busca por **nombre y por abreviación** y `isExactMatch` es verdadero si coincide exactamente por cualquiera de los dos; **doble coincidencia (Q-07)**: con una unidad llamada `kg` y otra con abreviación `kg`, `searchUnits("kg")` devuelve **ambas**, las dos con `isExactMatch = true`.
- [ ] Test (Rust): filas de la matriz de autorización de `createUnit` y `searchUnits`.
- [ ] Implementación: `createUnit(name, abbreviation)` y `searchUnits(text)` en `commands/units.rs`, registrados en `generate_handler!`. `searchUnits` no purga.

### 3.2 UM-02 — Ver todas las unidades

- [ ] Test (Rust, sobre `list_units_inner`): `productCount` incluye descontinuados; **`conversionCount` cuenta las conversiones en que participa la unidad como origen o como destino** (conversiones insertadas por `INSERT` directo en el test); purga antes de contar; orden español; búsqueda por nombre y abreviación; estado vacío.
- [ ] Test (Rust): fila de la matriz de autorización de `listUnits`.
- [ ] Implementación: `listUnits(search?)`.

### 3.3 UM-03 — Editar unidad

- [ ] Test (Rust, sobre `update_unit_inner`): como CAT-03 con nombre y abreviación; la unicidad excluye la propia fila; mensaje que indica cuál dato ya está registrado; "La unidad ya no existe".
- [ ] Test (Rust): fila de la matriz de autorización de `updateUnit`.
- [ ] Implementación: `updateUnit(unitId, name, abbreviation)`.

### 3.4 UM-04 + NT-01 (N-08) — Eliminar unidad

- [ ] Test (Rust, sobre `delete_unit_inner`): sin productos ni conversiones → se elimina directamente; con conversiones → se borran en cascada **dentro de la misma transacción** y se devuelve `deletedConversions` (contadas antes del `DELETE`); con productos → `unitId` queda `NULL`, su `updatedAt` pasa a `now` (Q-02), **una** notificación `N-08` / `ADVERTENCIA` (formato de NT-01, límite de 20, total real) y `affectedProducts`; purga primero; "La unidad ya no existe"; **atomicidad**: con un trigger que falle el `INSERT` en `notifications`, la unidad **y sus conversiones** quedan intactas; el log no incluye nombres de productos.
- [ ] Test (Rust): fila de la matriz de autorización de `deleteUnit`.
- [ ] Implementación: `deleteUnit(unitId)` con la transacción única de `plan.md` §3.7.

### 3.5 Interfaz de Unidades (FE-U-01 a FE-U-05, FE-U-07)

- [ ] Test (Vitest + jsdom): `unitsService.js` (wrappers de unidades) y `unidadesView.js` con stub: pinta solo lo que devuelve `listUnits`; estados de carga/vacío/error; el modal de alta muestra coincidencias como aviso de duplicado (FE-U-07); el mensaje del backend indica cuál dato (nombre o abreviación) está duplicado (FE-U-02); eliminar abre `confirmDialog` con **dos consecuencias separadas** (productos que quedarán sin unidad; conversiones que se borrarán) tomadas del último listado y el resultado muestra ambos números **reales**; `conversionCount` se conserva en la caché de la vista pero **no** es columna; la búsqueda de unidades por nombre y abreviación la hace el backend.
- [ ] Implementación: `services/unitsService.js` (parte de unidades) y `views/unidadesView.js` (dueño único de unidades, modal de unidad, conversiones y Conversor; las conversiones y el Conversor se conectan en la §4); `#modal-unidad-abreviacion` con `maxlength=10` y nombre `30`; eliminar el renombrado de productos por texto (`guardarUnidad`); corregir cadenas dañadas (T-08).
- [ ] Eliminar de `dashboard.js` el mock de unidades (funciones de render, `guardarUnidad`, `eliminarUnidad`, validaciones y llamadas en `showPage()`) y sus manejadores inline. **No se borra aquí el arreglo `unidadesData`** (ni su contador): el Conversor y el formulario de producto aún lo leen; se borra en la última tarea de §6.10 (A-02). **La eliminación del mock de conversiones y del Conversor se hace en la §4**, no aquí, para no dejar el Conversor sin datos entre ambas. Verificación: `grep` de los identificadores de unidades.
- [ ] Verificación manual/UI: crear, editar y eliminar una unidad real; duplicar nombre y abreviación muestra el mensaje correcto.

---

## 4. Conversiones y Conversor Rápido — UM-06, UM-07

### 4.1 UM-06 — Gestionar conversiones

- [ ] Test (Rust, sobre `create_conversion_inner`, `update_conversion_factor_inner`, `delete_conversion_inner`, `list_conversions_inner`), una sub-casilla por grupo:
  - [ ] Alta válida (factor con hasta 6 decimales, texto → `factorMicro`).
  - [ ] **Orden de validación de `plan.md` §3.5** con un test por regla incumplida: misma unidad ("Elige dos unidades distintas") → factor (7.4) → unidades existentes ("La unidad ya no existe") → conversión directa ya existente en **cualquier sentido** (mensaje "Ya existe una conversión…Edítala") → **segundo camino** (A↔B y B↔C impiden A↔C; mensaje "Esas unidades ya están relacionadas…") → `INSERT`.
  - [ ] El índice `unitConversionsPair` como última defensa (test que lo viola saltándose la comprobación en código).
  - [ ] Editar solo el factor (el grafo no cambia; "La conversión ya no existe" si no existe).
  - [ ] Eliminar quita la arista y permite luego crear la que antes era segundo camino.
  - [ ] `listConversions`: forma "1 [origen] = [factor] [destino]", factor como texto con coma sin ceros finales, orden por unidad de origen y luego destino con `sort_spanish`, búsqueda por nombre o abreviación de cualquiera de las dos unidades.
  - [ ] **Auditoría:** editar el factor con `updateConversionFactor` actualiza `updatedAt` de la conversión y **no** cambia `createdAt`; el valor de `updatedAt` coincide con `format_utc_timestamp(now)`.
- [ ] Test (Rust): filas de la matriz de autorización de `listConversions`, `createConversion`, `updateConversionFactor` y `deleteConversion`.
- [ ] Implementación en `commands/conversions.rs`: los cuatro comandos, registrados en `generate_handler!`. El camino único se valida cargando las conversiones y recorriendo el grafo en memoria dentro de la misma transacción.

### 4.2 UM-07 — Conversor Rápido

- [ ] Test (Rust, sobre `convert_quantity_inner`): ambos sentidos (multiplica y divide); **varios tramos** (incluida tonelada→kilo→gramo→miligramo, que sin reducir por MCD desbordaría `i128`); misma unidad → devuelve la cantidad sin ceros finales; sin camino → `validationFailed` "No hay una conversión registrada entre esas unidades" (no `notFound`); unidad inexistente → "La unidad ya no existe"; redondeo a 6 decimales mitad hacia arriba **una sola vez al final**; resultado sin ceros finales; resultado `0` si el redondeo lo anula; resultado > 999.999.999,999999 y desborde en un camino largo (`checked_*`) → `validationFailed` "El resultado es demasiado grande para mostrarse" y **nunca** desborde silencioso; cantidad validada con `parse_quantity_micro`; el resultado viaja como **texto decimal**; solo lee (no crea ni modifica nada).
- [ ] Test (Rust): fila de la matriz de autorización de `convertQuantity`.
- [ ] Implementación: `convertQuantity(fromUnitId, toUnitId, quantity)` con fracción `N/D` en `i128`, reducción por MCD tras cada tramo y recorrido simple del grafo sin ciclos; registrado en `generate_handler!`.

### 4.3 Interfaz de Conversiones y Conversor (FE-V-01 a FE-V-06, FE-U-06)

- [ ] Test (Vitest + jsdom): el listado pinta lo que devuelve `listConversions` con el factor tal cual (hasta 6 decimales sin ceros finales, FE-V-06); en edición, origen y destino están **deshabilitados** y solo se envía el factor (FE-V-01); el factor es `type="text"` con `inputmode="decimal"` y se envía **tal cual** (FE-V-02, T-07); el texto de ayuda explica el camino único (FE-V-03); el mensaje del backend (misma unidad, duplicado, segundo camino) aparece en pantalla (FE-V-04); eliminar usa `confirmDialog` (FE-V-05); el Conversor llama a `convertQuantity`, **muestra el texto del backend sin pasarlo por `Number`/`parseFloat`** y muestra el mensaje del backend cuando no hay camino o el resultado es demasiado grande (FE-U-06).
- [ ] Implementación: completar `unitsService.js` (conversiones y Conversor) y `unidadesView.js`; eliminar el cálculo y el recorrido BFS del cliente (`obtenerFactorConversion`, `actualizarConversorRapido`, validaciones de duplicado en el cliente); la vista previa `#modal-conversion-preview` se conserva solo como comodidad.
- [ ] Eliminar de `dashboard.js` el mock de conversiones y del Conversor (`formatearNumeroUnidad`, `guardarConversion`, `eliminarConversion`, BFS) y sus manejadores inline. **El arreglo `conversionesData` se borra con los demás en la última tarea de §6.10** (A-02). Verificación con `grep` de las funciones; la sección carga sin errores.
- [ ] Verificación manual/UI: crear A↔B y B↔C, comprobar que A↔C se rechaza con el mensaje de camino único; convertir en ambos sentidos con 6 decimales; probar "sin camino".

---

## 5. Precios — P-10 y P-11 (funciones puras)

*(Se separa de Productos porque `selling_price_cents` lo usan la vista previa, el listado y el detalle, y `weighted_average_cost_cents` no tiene consumidor.)*

- [ ] Test (Rust, sobre `pricing.rs`): `selling_price_cents` — 12,50 USD y 20 % → 1500 centavos; 0 % (vende al costo) y 1000 % (≈ $11.000.000 sin desbordar); redondeo mitad hacia arriba; `weighted_average_cost_cents` — ejemplo del requisito 10 a $1,00 y 5 a $2,00 → **133** (133,33); con cantidad actual 0 devuelve exactamente el costo de la compra (S-9); rechaza cantidad comprada ≤ 0 y costo de compra ≤ 0; desborde verificado (`checked_*`) → error, nunca resultado silencioso.
- [ ] Test (Rust, `previewSellingPrice` al editar): recibe solo el `%` (texto) y el `productId`; el costo lo lee el backend de la BD. Si el costo en la BD cambió entre la carga del formulario y el guardado, el precio devuelto usa el costo **actual**, no el que el usuario vio. El precio devuelto por la vista previa es exactamente el que luego guarda `updateProduct` (misma función `selling_price_cents`). *(La implementación del comando está en §6.1; este test fija aquí el contrato del cálculo.)*
- [ ] Implementación: `pricing.rs` (`selling_price_cents`, `weighted_average_cost_cents`) con `i128`, sin `f64`, declarado `pub` en `lib.rs`. `weighted_average_cost_cents` **no tiene consumidor** hasta PEN-04: sin warnings de `clippy` (módulo de librería con tests) y registrado en Complexity Tracking (`plan.md` §9).

---

## 6. Productos — P-01 a P-14 (y CAT-05, UM-05)

### 6.1 P-01 + P-11 — Agregar producto y vista previa del precio

- [ ] Test (Rust, sobre `create_product_inner` y `preview_selling_price_inner`, con `today` inyectado), una sub-casilla por grupo:
  - [ ] Creación válida → producto **activo, con la cantidad inicial indicada (> 0)**, guardada en `quantityMilli`; **ningún comando de esta spec la modifica después**; el ID no se reutiliza tras eliminar; el precio de venta devuelto es el calculado por `selling_price_cents`.
  - [ ] Cantidad inicial con `parse_quantity_milli` (> 0, hasta 999.999,999 y 3 decimales): omitida, 0, negativa, no numérica, con más de 3 decimales o mayor que el tope → rechazada con el mensaje de 7.4.
  - [ ] Costo 0, negativo, no numérico, > 2 decimales o > 999.999,99 rechazados con el mensaje correcto de 7.4; porcentaje negativo, > 1000,00 o > 2 decimales rechazado.
  - [ ] Texto: nombre de 1 carácter ("al menos 2 caracteres"), vacío y de 101 caracteres; descripción > 500; descripción vacía → `NULL`; texto guardado tal como se escribió.
  - [ ] Unicidad: nombre duplicado entre **activos** → `duplicateField` "Ya existe un producto con ese nombre" (decidido por el índice parcial); mismo nombre que un descontinuado → permitido.
  - [ ] Caducidad: pasada rechazada, hoy aceptada, `2026-02-30` y año > 2100 rechazados.
  - [ ] Referencias: `categoryId`/`unitId` inexistentes → "La categoría ya no existe" / "La unidad ya no existe".
  - [ ] **Orden de validación** (texto → números —precio de entrada, cantidad inicial, porcentaje— → caducidad → existencia de categoría/unidad → unicidad).
- [ ] Test (Rust, **contrato C-01**): el struct de entrada de `createProduct` **falla al deserializarse** con `sellingPrice` (sí declara `quantity` y `cost`, ambos como texto); `create_product_inner` recibe la cantidad inicial como texto (> 0), la valida y la inserta.
- [ ] Test (Rust, sobre `preview_selling_price_inner`, **costo vigente**): al **editar** recibe `productId` y el `%` (texto) y **lee el costo de la BD**; si el costo en la BD cambió entre la carga del formulario y la vista previa, el precio devuelto usa el costo **actual**; al **crear** recibe el `cost` escrito (texto, validado con `parse_cost_cents`) y el `%`; `productId` y `cost` a la vez, o ninguno, → `validationFailed` "Los datos enviados no son válidos"; aplica las mismas reglas del `%` que el guardado y devuelve **exactamente** el precio que luego se guarda con ese costo; **no escribe ni purga** (la BD queda igual); producto inexistente → "El producto ya no existe"; descontinuado → `invalidState`.
- [ ] Test (Rust, sobre `preview_selling_price_inner`, **producto que cambió de estado entre la carga del formulario y la vista previa**): con un `productId` cuyo producto **venció** desde que se abrió el formulario (fixture con `discontinuedAt` anterior al umbral de 60 días), la vista previa responde `notFound` "El producto ya no existe" (queda excluido en la consulta, igual que en §3.6); con un `productId` cuyo producto **se descontinuó** desde que se abrió el formulario (pero aún no venció), responde `invalidState` "Un producto descontinuado no se puede editar. Actívalo primero". En ambos casos la BD no cambia y no se registra ningún `validationFailed`.
- [ ] Test (Rust): filas de la matriz de autorización de `createProduct` y `previewSellingPrice`.
- [ ] Implementación en `commands/products.rs`: `createProduct(input)` (un único argumento `input`, struct `deny_unknown_fields` que declara `cost` y `quantity` como texto) y `previewSellingPrice(input)` (`profitPercent` y `productId` —el costo lo lee el backend de la BD— o `cost` al crear; struct `deny_unknown_fields`); registrados en `generate_handler!`; `create_product` devuelve `ProductListItem`.

### 6.2 P-02 / P-06 — Detalle de producto

- [ ] Test (Rust, sobre `get_product_inner`): activo → todos los campos de P-02 (precios en centavos, `profitPercent` con 2 decimales como texto, `quantity` como texto sin ceros finales, `expiresOn` como `YYYY-MM-DD`); descontinuado → `isDiscontinued`, `discontinuedAt` en el formato de C-10 y `remainingDays` (60 días menos lo transcurrido, **hacia arriba**, **acotado a 0–60**, con una fixture de reloj atrasado que daría > 60 y otra vencida que daría < 0); **purga primero**: un descontinuado vencido responde "El producto ya no existe"; inexistente → "El producto ya no existe"; categoría/unidad `NULL` llegan como `null`.
- [ ] Test (Rust): fila de la matriz de autorización de `getProduct`.
- [ ] Implementación: `getProduct(productId)`.

### 6.3 P-03 — Editar producto

- [ ] Test (Rust, sobre `update_product_inner`): reemplazo completo de los campos editables (lo ausente se guarda como vacío); guardar sin cambios funciona (la unicidad excluye la propia fila); cambiar el % recalcula el precio con el costo vigente; **el costo y la cantidad no cambian**; una caducidad **ya vencida que no se modifica** se acepta, mientras que asignar una fecha pasada se rechaza; categoría/unidad en `null` ("Sin categoría" / "Sin unidad") funcionan; descontinuado → `invalidState` "Un producto descontinuado no se puede editar. Actívalo primero"; producto, categoría o unidad inexistentes → su mensaje de `notFound`; nombre duplicado entre activos → `duplicateField`; purga primero.
- [ ] Test (Rust, **contrato C-01**): el struct de entrada de `updateProduct` falla al deserializarse con `quantity`, `sellingPrice` **y `cost`**, y pasa con los campos válidos.
- [ ] Test (Rust): fila de la matriz de autorización de `updateProduct`.
- [ ] Implementación: `updateProduct(input)` (struct `deny_unknown_fields` que no declara cantidad, precio de venta ni costo).

### 6.4 P-04, P-07 — Descontinuar y activar

- [ ] Test (Rust, sobre `discontinue_product_inner` y `activate_product_inner`, con `now` inyectado): descontinuar un activo → `status = 'discontinued'` y `discontinuedAt = format_utc_timestamp(now)`; descontinuar uno ya descontinuado → `invalidState` "El producto ya está descontinuado"; activar un descontinuado → vuelve a activo, `discontinuedAt = NULL` y **conserva todos sus datos, incluida la cantidad** (fixture con cantidad > 0); activar uno ya activo → `invalidState` "El producto ya está activo"; activar con **otro activo del mismo nombre** → `duplicateField` "No se puede activar: ya existe otro producto activo con ese nombre" (lo decide el índice parcial, con mensaje propio de activación); purga primero (un vencido responde "El producto ya no existe"); inexistente → "El producto ya no existe".
- [ ] Test (Rust): filas de la matriz de autorización de `discontinueProduct` y `activateProduct`.
- [ ] Implementación: `discontinueProduct(productId)` y `activateProduct(productId)`.

### 6.5 P-14 — Eliminar producto manualmente

- [ ] Test (Rust, sobre `delete_product_inner`): elimina físicamente activos y descontinuados (incluido uno con cantidad > 0, fixture); **no genera notificación**; un descontinuado ya vencido responde "El producto ya no existe" (la purga previa lo borró); inexistente → "El producto ya no existe"; las categorías/unidades relacionadas no cambian.
- [ ] Test (Rust): fila de la matriz de autorización de `deleteProduct`.
- [ ] Implementación: `deleteProduct(productId)`.

### 6.6 P-08 — Cobertura transversal de la purga

- [ ] Test (Rust, una tabla de casos recorriendo **todos** los comandos de `plan.md` §3.6): `listCategories`, `getCategory`, `deleteCategory`, `listUnits`, `deleteUnit`, `listOutOfStockAndDiscontinued`, `getProduct`, `updateProduct`, `discontinueProduct`, `activateProduct` y `deleteProduct` purgan antes de actuar; los conteos y las listas de N-08/N-09 no incluyen vencidos; los comandos que reciben `productId` responden "El producto ya no existe" para uno vencido; con un fallo de la purga (trigger) **ninguno aborta**: cada uno devuelve su resultado sin los vencidos (excluidos en la consulta), con `expiredCleanupWarning` no nulo, y registra el error en `errorLogs`; `listProducts`, `getDashboardSummary` y `previewSellingPrice` **no** purgan (solo ven activos o no escriben).
- [ ] Verificación: añadir esta tabla de casos al cierre de la §6 sin duplicar los tests de las secciones anteriores; si alguno de los comandos listados no purga, se corrige el comando, no el test.

### 6.7 P-09 + P-12 — Listado de productos activos con búsqueda y filtros

- [ ] Test (Rust, sobre `list_products_inner`, con `today` inyectado y fixtures con cantidades exactas), una sub-casilla por grupo:
  - [ ] Solo activos (incluye agotados, marcados `isOutOfStock`); orden español; búsqueda solo por nombre (7.2).
  - [ ] Filtro de categoría concreta y "Sin categoría"; `categoryId` y `withoutCategory` a la vez → `validationFailed` "Los datos enviados no son válidos".
  - [ ] Filtro de stock — Disponibles (> 0), Agotados (= 0), **Stock bajo (0 < cantidad < 5)** con su frontera (4,999 incluido, 5 excluido, 0 excluido).
  - [ ] Filtro de caducidad — Con caducidad (incluye vencidos, S-11), Sin caducidad, Vencidos (anterior a hoy), **Próximos a vencer (hoy a hoy + 29, ambos inclusive)** con hoy incluido, hoy + 29 incluido y hoy + 30 excluido.
  - [ ] Los filtros se combinan entre sí y con la búsqueda.
  - [ ] Con productos creados por `createProduct` (cantidad inicial > 0), "Disponibles" los devuelve a todos y "Agotados" devuelve vacío (§14 de la spec); "Stock bajo" devuelve los creados con menos de 5.
  - [ ] El struct `filters` rechaza claves desconocidas y toma los valores por defecto de los campos omitidos (B-05).
- [ ] Test (Rust): fila de la matriz de autorización de `listProducts`.
- [ ] Implementación: `listProducts(filters)` con `today` inyectado desde el envoltorio (`chrono::Local::now().date_naive()`); `isOutOfStock` = cantidad 0 e `isLowStock` = 0 < cantidad < 5; sin `Valor Total`.

### 6.8 P-05 + P-13 — Pestaña "Agotados y Descontinuados"

- [ ] Test (Rust, sobre `list_out_of_stock_and_discontinued_inner`): devuelve productos descontinuados **o** agotados, **un mismo producto una sola vez** (agotado y descontinuado a la vez aparece una vez con ambas condiciones); con datos reales (cantidad inicial > 0) ningún activo aparece como agotado: solo los descontinuados; los agotados se prueban con fixtures; filtro `condition` (`all` / `outOfStock` / `discontinued`); búsqueda por nombre; `remainingDays` solo para descontinuados (0–60, hacia arriba); orden español con desempate por ID; **purga primero**, sin vencidos en el resultado; con la purga fallida devuelve el listado sin vencidos y con `expiredCleanupWarning` (§1.8).
- [ ] Test (Rust): fila de la matriz de autorización de `listOutOfStockAndDiscontinued`.
- [ ] Implementación: `listOutOfStockAndDiscontinued(search?, condition?)`.

### 6.9 Interfaz de Productos — servicios, combobox y formulario (FE-P-10 a FE-P-20, CAT-05, UM-05)

- [ ] Test (Vitest + jsdom) de `components/combobox.js` (T-06): muestra las coincidencias que entrega el backend (`searchCategories`/`searchUnits`; la unidad también por abreviación); la opción "Agregar nueva categoría «texto»" / "Agregar nueva unidad «texto»" aparece **solo** si ninguna coincidencia tiene `isExactMatch`; botón "Sin categoría" / "Sin unidad" **solo en edición** y solo si ya hay valor; **texto escrito sin elegir opción bloquea el guardado** con el aviso "Elige una opción de la lista o vacía el campo"; nunca envía ni crea nada a partir de texto sin elegir; trabaja con `{id, nombre}` (T-02); **teclado (Q-06)**: `role="combobox"`/`listbox`/`option` con `aria-expanded` y `aria-activedescendant`, ↑/↓ cambian la opción activa, Enter elige, Escape cierra sin elegir, Tab cierra y sigue; **doble coincidencia exacta (Q-07)**: dos unidades que coinciden (una por nombre, otra por abreviación) se muestran ambas y no aparece "Agregar nueva"; cada opción de unidad se muestra con el formato **"nombre (abreviación)"** (`kg (k)` y `Kilogramo (kg)`), y ese mismo formato se usa cuando hay una sola coincidencia; las de categoría muestran solo el nombre.
- [ ] Implementación de `components/combobox.js`.
- [ ] Test (Vitest + jsdom) de `productosView.js` — formulario, una sub-casilla por grupo:
  - [ ] La categoría elegida con "Agregar nueva" se crea en ese momento, queda seleccionada y **existe aunque el producto no se guarde** (S-7).
  - [ ] "Agregar nueva unidad" abre `#modal-unidad` con el nombre **precargado y editable** y la abreviación **vacía, sin sugerencia automática**; **cancelar ese paso no crea nada** y deja el campo sin cambios.
  - [ ] El precio de venta es solo lectura y **no se envía**; la vista previa llama a `previewSellingPrice` con el texto tal cual (al editar, solo `productId` y `%`; al crear, el `cost` escrito y el `%`), queda en blanco **sin error** mientras el valor esté incompleto o inválido y muestra el mensaje al salir del campo o al guardar.
  - [ ] `#producto-ganancia` es editable y obligatoria; `#producto-precio-entrada` es obligatorio en creación y de solo lectura en edición; `#producto-stock` es la **cantidad inicial obligatoria** en creación (texto con `inputmode="decimal"`, > 0, hasta 3 decimales, FE-P-13) y texto de solo lectura en edición, que no se envía; categoría y unidad son **opcionales**.
  - [ ] Los campos numéricos son `type="text"` con `inputmode="decimal"`, envían el texto sin `parseFloat` y solo muestran ayudas de cliente ("usa coma para los decimales", aviso de `1.234`; la regla autoritativa es la del backend, T-07).
  - [ ] `maxlength` de nombre 100 y descripción 500 solo como ayuda (FE-P-17); la caducidad usa `min` = hoy y `max` = `2100-12-31` solo como ayuda; los mensajes de duplicado y de "ya no existe" del backend se muestran en pantalla.
- [ ] Implementación: `services/productsService.js` (wrappers de los 9 comandos de productos) y la parte de formulario de `views/productosView.js`; eliminar `irCrearCategoria()` y `agregarUnidadRapida()` (el primero navegaba a otra pantalla y el segundo usaba `prompt()`); ocultar con `hidden` la sección **Facturas Asociadas** del formulario (D-03).

### 6.10 Interfaz de Productos — listados, filtros, pestaña y detalle (FE-P-01 a FE-P-07, FE-P-30 a FE-P-34)

- [ ] Test (Vitest + jsdom) del listado, la pestaña y el detalle de productos, una sub-casilla por grupo:
  - [ ] El listado pinta solo lo que devuelve `listProducts` (sin datos de ejemplo), con ID `#00001`, "Sin categoría" / "—", badges "Agotado" y "Stock Bajo" con el umbral `0 < cantidad < 5` (FE-P-03), columna **Acciones** (ver, editar, descontinuar, eliminar) y **sin** columna Valor Total (`hidden`); búsqueda placeholder "Buscar producto…" y filtros de categoría, stock y caducidad que llaman al backend.
  - [ ] La pestaña **Agotados y Descontinuados** (`#tab-producto-agotados` / `#producto-agotados`, rama `'agotados'` de `showProductoTab`) muestra Nombre, Categoría, Unidad, Cantidad, Condición, Días restantes y Acciones, con filtro de condición y búsqueda; descontinuado → acciones **Activar** y **Eliminar**.
  - [ ] El detalle de un activo ofrece **Editar, Descontinuar y Eliminar**; el de un descontinuado muestra etiqueta, fecha de descontinuación (hora local, `dd/mm/aaaa`), días restantes y **Activar / Eliminar** (sin Editar ni Descontinuar); el detalle muestra ganancia con 2 decimales y **sin el tono naranja por ganancia negativa** (FE-P-30).
  - [ ] La confirmación de descontinuar y de eliminar usa `confirmDialog` y **advierte la pérdida de stock** si la cantidad > 0.
  - [ ] Ante `invalidState` ("ya está descontinuado" / "ya está activo") o "ya no existe" se muestra el mensaje del backend y se recarga la vista; un descontinuado no ofrece "Editar" y, si el backend aun así rechaza (datos desactualizados), se muestra su mensaje (FE-P-20).
  - [ ] El paginador global sigue funcionando con las filas del backend (T-09).
- [ ] Implementación: resto de `productosView.js` (listado, filtros, pestaña P-13, detalle y acciones) y orden de pestañas Productos, Agotados y Descontinuados, Crear Nuevo Producto, Historial; **ocultar con `hidden`** la pestaña **Historial**, el botón **Ver Historial** y el `<details>` **Facturas Asociadas** del detalle (D-01, D-03, PEN-09); `formatters.js` para dinero, fecha e ID; exportar la función que abre la pestaña P-13 con filtro aplicado (la usa el dashboard en la §7); **añadir el `<tbody>` de la pestaña P-13 a `PAGINABLES`** en `dashboard.js` (único cambio al paginador; test jsdom: con más filas que el tamaño de página, solo se ven las de la página actual).
- [ ] **Código simulado de facturas:** eliminar `productosData`, `productosDisponibles` y las funciones de facturas que los consumen (`poblarSelectHistorialProducto`, `obtenerMovimientosProducto`, `agregarLineaVenta`, la actualización de línea de venta y `poblarDatalists` en su parte de productos). **Ocultar la sección de facturas del dashboard** hasta su spec: localizar el `<li>` del sidebar con `a[data-page="facturas"]` (verificar el valor real del `data-page` contra `dashboard-unificado.html` con `grep` antes de implementar; el atributo vive en el `<a>`, no en el `<li>`) y ocultarlo con `.closest('li').hidden = true`; ocultar también la página (`#page-facturas` o el contenedor equivalente) con `hidden`, igual que Historial y Facturas Asociadas. **Sin adaptador, sin caché y sin conversión a dólares en el frontend.** Verificación con `grep`: esos identificadores no aparecen en `dashboard.js`; la navegación no da acceso a la sección oculta. Cuando se implemente la spec de facturas, se conectará directamente a `productsService`.
- [ ] Eliminar de `dashboard.js` el mock de productos (`productosData`, `next…Id`, `renderProductos`, `guardarProducto`, `editarProducto`, `actualizarGananciaProducto`, `verProducto`, `eliminarProducto`, `eliminarProductoDesdeVer`, `crearProductoRapido` si solo sirve al mock) y todos sus manejadores inline, incluidas las llamadas dentro de `showPage()`. Verificación: `grep` del resto de los identificadores de la lista de la §0 no devuelve nada en `dashboard.js`; el dashboard carga sin errores en consola.
- [ ] **Borrado final de los arreglos globales (A-02) — última tarea de código de §6.10:** con todas las secciones ya migradas, eliminar **juntos** `categoriasData`, `unidadesData` y `conversionesData` con sus contadores `next…Id` y `editing…Id`, y la lectura transitoria que el formulario de producto viejo hacía de ellos. Verificación: `grep` de esos identificadores en `dashboard.js` no devuelve nada; el dashboard carga sin errores en consola y el Conversor, los productos y el resto de secciones siguen funcionando.
- [ ] Verificación manual/UI: crear un producto real con categoría y unidad creadas sobre la marcha; ver el cálculo del precio al escribir el %; editar, descontinuar (ver el contador de días), activar y eliminar; comprobar con dos productos del mismo nombre (uno descontinuado) el rechazo al activar.

---

## 7. Dashboard — DASH-01 y protección de la spec 1

### 7.1 DASH-01 — Tarjeta de productos agotados

- [ ] Test (Rust, sobre `get_dashboard_summary_inner`, con fixtures): total de **agotados activos** (cantidad 0); un activo con cantidad > 0 y un descontinuado agotado **no** cuentan; hasta **3 categorías** con más agotados en orden descendente por cantidad y, a igualdad, **alfabético español**; con 4 o más categorías solo salen 3; `withoutCategoryCount` cuenta los agotados activos sin categoría; estado vacío → total 0, lista vacía y 0 sin categoría (sin datos de ejemplo); no purga (solo activos).
- [ ] Test (Rust): fila de la matriz de autorización de `getDashboardSummary`.
- [ ] Implementación: `getDashboardSummary()` en `commands/dashboard.rs`, un **único comando** de resumen; registrado en `generate_handler!`.
- [ ] Test (Vitest + jsdom) de `dashboardView.js` con stub de `dashboardService.js`: pinta total, hasta 3 categorías (el render actual `renderInventario` espera nombres en un arreglo; se adapta a `{name, count}`) y "sin categoría" (`#dash-agotados-total`, `#dash-agotados-categorias`, `#dash-agotados-sin-categoria`); estado vacío con total 0; **"Ver Agotados" y "Ver Descontinuados"** abren la pestaña P-13 con el filtro de condición ya aplicado (Agotados / Descontinuados), usando `showPage` y `showProductoTab` a través de `window` (hoy son dos `onclick` inline idénticos en el HTML que solo llaman a `showPage('productos', …)`: se reemplazan por listeners de `dashboardView.js`); **KPIs `inventario` y `agotados`, "Mejor Vendidos" y "Lento Movimiento" en estado vacío** (`hidden` o vacío, sin cifras ni listas inventadas, D-08, PEN-10).
- [ ] Implementación: `services/dashboardService.js` y `views/dashboardView.js`; en el IIFE de `dashboard.js` que contiene `DATOS_EJEMPLO`/`INVENTARIO_EJEMPLO` (ubicarlo con `grep`), retirar la parte de agotados de `obtenerInventarioDashboard`/`renderInventario` (la carga de la tarjeta pasa a `dashboardView.js`); los KPIs y listados que dependen de ventas pasan a estado vacío (sin datos inventados). Los botones "Ver …" se conectan **sin tocar `perfil.js`, `usuarios.js` ni `#reserved-data-warning`** (FE-D-04).
- [ ] Verificación manual/UI: con la BD vacía, la tarjeta muestra ceros; con productos creados por la app (cantidad inicial > 0) la tarjeta sigue en cero; con fixtures de cantidad 0 el total y las categorías coinciden con la pestaña P-13. Como ningún comando de la app produce un agotado, para esta comprobación se detiene la aplicación y, **sobre la BD de desarrollo**, se ejecuta `UPDATE products SET quantityMilli = 0 WHERE id = ?` con un cliente SQLite; es un paso manual de prueba, nunca código ni comando de la aplicación (Q-04).

### 7.2 C-09 / FE-D-04 — Protección de los elementos de la spec 1

- [ ] Test (Vitest + jsdom, **escribirlo antes de tocar el HTML por última vez en esta feature**): carga `dashboard-unificado.html` y comprueba que existe `#reserved-data-warning` y que siguen referenciados los módulos `usuarios.js` y `perfil.js` y el `import('./session.js')` de `dashboard.js`. El test existente de `perfil.js` (advertencia con `hasReservedContactData: true`) sigue en verde sin modificarse.

---

## 8. Pruebas transversales de interfaz y contrato (Vitest + jsdom)

*(Con `window.__TAURI__` sustituido por un stub **solo dentro de los tests**, nunca en código de producción.)*

- [ ] Test de contrato `invoke` ↔ `generate_handler!`: con los servicios nuevos, los nombres `snake_case` extraídos de `lib.rs` y de los wrappers coinciden exactamente (**41 comandos**: 7 `auth.rs` + 8 `users.rs` + 6 `categories.rs` + 5 `units.rs` + 5 `conversions.rs` + 9 `products.rs` + 1 `dashboard.rs`). Límite conocido, ya documentado: no detecta typos en los argumentos. **El test calcula el recuento a partir de los archivos** (nombres `snake_case` en `generate_handler!` y en los wrappers `services/*.js`), no lo compara contra un literal fijo. Si el test existente usaba un literal (por ejemplo, `15`), hay que reescribirlo para que cuente dinámicamente; así no se rompe con cada comando nuevo y no hay que actualizarlo en cada módulo.
- [ ] Test de limpieza (ampliar `environment.test.js` sin quitar lo existente): recorrer las fuentes de producción y comprobar que `dashboard.js` ya no contiene `categoriasData`, `unidadesData`, `conversionesData`, `INVENTARIO_EJEMPLO`, `productosData`, `productosDisponibles`, el literal de agotados de ejemplo ni los `next…Id` de esta feature, que ningún archivo de producción de esta feature usa `localStorage`/`sessionStorage` para datos de negocio, que ningún `onclick`/`oninput`/`onchange` inline de las secciones migradas sigue en el HTML, **y que el `<li>` que contiene `a[data-page="facturas"]` y la página `#page-facturas` tienen el atributo `hidden`**.
- [ ] Test (Vitest + jsdom): ante un `expiredCleanupWarning` no nulo, cada vista que lo recibe (categorías, unidades, pestaña P-13, detalle y acciones de producto) muestra el aviso "No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación" de forma discreta (`textContent`, `role="status"`), sin romper el listado; con `null` lo limpia.
- [ ] Test: todas las vistas nuevas (`categoriasView`, `unidadesView`, `productosView`, `dashboardView`) muestran estado de carga, vacío y error, y ninguna contiene texto técnico (mensajes de Rust/SQLite) ante un error interno: solo el mensaje genérico.
- [ ] Verificar que toda la batería de `npm test` pasa junto con los tests de la spec 1 (anotar el recuento final).

## 9. Cierre del módulo

- [ ] **Revisión final contra `spec.md`:** recorrer CAT-01 a CAT-06, UM-01 a UM-07, P-01 a P-14, DASH-01 y NT-01 y confirmar que cada requisito tiene su comando implementado y su test pasando. Recorrer también la §14 (criterios de prueba) de la spec, punto por punto.
- [ ] **Verificación de la matriz de autorización (`Agents.md` §3):** la tabla `(comando, sin sesión → rechazado, ambos roles → permitido)` tiene una fila por cada uno de los **26** comandos nuevos y cada fila tiene su test pasando. Ningún comando de esta feature queda fuera. Ninguno actualiza `lastActivityAt`.
- [ ] **Verificación de registros:** `validationFailed` e `invalidCredentials` no generan fila en `errorLogs`; `duplicateField`, `notFound`, `invalidState` e `internal` sí; ningún registro contiene valores ingresados por el usuario (nombres, descripciones, números, nombres de productos).
- [ ] **Verificación de la purga best-effort:** con un trigger de prueba que haga fallar el `DELETE` sobre `products`, cada comando de §3.6 devuelve su resultado sin vencidos, incluye `expiredCleanupWarning` y registra el error; la app sigue usable.
- [ ] **Verificación de columnas de auditoría:** `createdAt` y `updatedAt` están en `categories`, `units`, `products` y `unitConversions`, con el formato correcto, y `updatedAt` se actualiza al editar, y también en los productos afectados al eliminar su categoría o unidad (Q-02).
- [ ] **Verificación de la sección de facturas oculta:** el contenedor de facturas del dashboard tiene el atributo `hidden` y no se accede desde la navegación.
- [ ] **Verificación de garantías:** ninguna tarea [x] de `tasks-01.md` fue reabierta; `should_log()` no cambió; no se editaron `auth.css` ni `dashboard.css`; el test existente de `invoke.js` y el de `perfil.js` siguen en verde sin modificaciones.
- [ ] **Complexity Tracking:** confirmar que `plan.md` §9 refleja las desviaciones reales (P-10 sin consumidor, rechazo de contrato por Tauri, textos compuestos en el frontend, lectura que purga, purga best-effort). Si apareció alguna otra durante la implementación, documentarla ahí.
- [ ] **Documentación:** actualizar `Agents.md` §1 ("Estado actual": Autenticación y Usuarios e Inventario-Productos implementados) y, si cambiaron, §7 con los comandos reales; actualizar `specs/pendientes.md` (verificar que PEN-01 a PEN-14 siguen vigentes y que ninguna se resolvió sin querer; el estado de PEN-03 sigue como resuelto; PEN-11 a PEN-14 ya están registrados desde antes de empezar la implementación); `Constitution.md` **no** cambia salvo que surja una nueva desviación decidida por el dueño.
- [ ] Formato y lint: `npx prettier --write` (frontend y tests, sin los bundles vendorizados), `cargo fmt -- --check` y `cargo clippy --all-targets -- -D warnings` sin warnings; no hay `#[allow(non_snake_case)]`.
- [ ] `cargo test` y `npm test` pasan; anotar los recuentos finales.
- [ ] Verificación manual de extremo a extremo en la app real, como Administrador y como Contador: categorías, unidades, conversiones y Conversor, productos (alta, edición, descontinuar, activar, eliminar), pestaña Agotados y Descontinuados, tarjeta del dashboard y eliminación de una categoría/unidad con productos. Comprobar que Historial, Facturas Asociadas, la sección de facturas, Valor Total e Ingresos permanecen ocultos. **Pendiente si el entorno no permite operar la ventana nativa de Tauri**; en ese caso se anota explícitamente, como en `tasks-01.md`.
- [ ] **Pasada en la app compilada (Q-12):** repetir la verificación anterior con el resultado de `npm run tauri:build` (no solo `tauri:dev`): carga de los `<script type="module">`, listeners que reemplazaron a los `onclick`, `[hidden]`, diálogos con foco, buscadores con teclado y estados vacíos. Si el entorno no permite operar la ventana nativa, se anota como pendiente igual que arriba.
- [ ] **Verificación de teclado y foco (Q-06):** en la app real, recorrer el buscador de categoría y de unidad solo con teclado (↑/↓, Enter, Escape, Tab) y abrir/cerrar un diálogo de confirmación con teclado comprobando que el foco no se escapa y vuelve al botón que lo abrió.
- [ ] **Verificación de la BD de desarrollo:** si durante la implementación se ajustó un `CHECK`, una columna o un índice de una tabla que ya existía en la BD local, borrarla y reabrir la aplicación para regenerarla con el esquema final. Comprobar que `init_schema` crea las 5 tablas nuevas con las columnas de auditoría (`createdAt`/`updatedAt`) y que la app arranca sin errores en consola. Los tests de esquema usan una BD en memoria nueva y no se ven afectados por esto.
- [ ] Commit(s) siguiendo Conventional Commits, idealmente uno por tarea completa de este documento (`Agents.md` §8).
