# Plan: Categorías, Unidades de Medida y Productos

- **Feature:** `inventario-productos`
- **Basado en:** `spec.md` (esta misma carpeta; hoy `spec-2.md`), `Constitution.md` v1.2.0, `Agents.md`, la revisión de la interfaz (§7, antes `frontend-cambios.md`) y las decisiones de auditoría C-01 a C-10 (trazadas en §10, todas confirmadas por el dueño) y los hallazgos A-01 a B-05 de `conversacion.md` (también en §10).
- **Estado:** **Confirmado por el dueño.** Las decisiones de §10 (incluidas las nuevas de la revisión QA) están confirmadas. Se puede implementar siguiendo `tasks.md`. **Segunda revisión QA (Q-01 a Q-12):** aplicada por delegación del dueño, que dejó las decisiones a criterio del auditor (§10, decisiones 19 a 26).
- **Nota:** es el **segundo módulo con backend Rust y SQLite**. Reutiliza sin cambios la infraestructura de `autenticacion-y-usuarios` (conexión compartida, `AppError`, `require_session`, `setup_test_db`, patrón `xxx_inner`, envoltorio delgado) y solo la **amplía** con elementos nuevos. Nada de lo implementado en la spec 1 se reabre (ver la garantía en el encabezado de `tasks.md`).

---

## 1. Dependencias y configuración

**Dependencias nuevas: ninguna** (`Constitution.md` §2.II). Se reutilizan `rusqlite` 0.32.1 (`bundled`, `chrono`), `chrono` 0.4.45 y, en desarrollo, Vitest + jsdom. `bcrypt` y `tauri-plugin-dialog` no intervienen. La única novedad de `Cargo.toml` es activar la feature `test` de `tauri` **solo en `[dev-dependencies]`** (`tauri = { version = "2", features = ["test"] }`) para probar con `tauri::test` el rechazo de argumentos que hace Tauri antes de llamar al comando (A-03, §8): no añade ningún crate ni entra en el binario de producción; se registra en §9.

| Necesidad | Solución | Por qué no una dependencia |
|---|---|---|
| Normalizar texto sin acentos (7.1) | Función propia de ~15 líneas (`text.rs`) | `unicode-normalization` aplicaría NFC/NFD, que la spec descarta conscientemente (7.1) por ser un proyecto local, offline y de un solo usuario; el riesgo de mezclar formas es bajo en la práctica y se documenta como limitación aceptada. La tabla de plegado es de 6 vocales |
| Aritmética decimal exacta (7.4) | Enteros escalados (`i64` en SQLite, `i128` en cálculo) | `rust_decimal`/`bigdecimal` son innecesarios: el rango máximo escalado es 10^15 |
| Orden alfabético español | Clave de orden en Rust (C-02) | Una collation con `create_collation` habría que registrarla en cada conexión, incluida la de tests |
| Búsqueda por subcadena | `instr()` de SQLite sobre columnas normalizadas | `LIKE` trata `%` y `_` como comodines; `regex` sobra |
| Rechazar caracteres de control y de formato Unicode (7.3, M-06) | `char::is_control()` para Cc y una tabla propia de rangos Cf en `text.rs` | `unicode-general-category` resolvería ambas, pero sería una dependencia para una sola comprobación; la tabla se fija con un test y se revisa solo si cambia el estándar |

**Verificaciones previas (van a `tasks.md` §0):**
1. **Verificado** en `Cargo.toml`: `chrono = { version = "0.4", features = ["serde"] }` conserva las `default-features` (incluyen `clock`), así que `chrono::Local` compila (C-04). `tauri = { version = "2", features = [] }`: la feature `test` se añade solo en `[dev-dependencies]` (arriba).
2. Confirmar que el SQLite incluido por `bundled` admite índices parciales y sobre expresiones (disponibles desde SQLite 3.9, muy anterior a la versión que empaqueta `rusqlite` 0.32).
3. Confirmar el formato exacto del mensaje de SQLite para el índice sobre expresiones (`index 'unitConversionsPair'`), que la deducción de campo duplicado de §4 usa; un test lo fija (como ya hace la spec 1 con las columnas).
4. **Spike de `tauri::test` (A-03):** una prueba descartable con `tauri::test::mock_builder()` y un comando de ejemplo con `#[serde(deny_unknown_fields)]`, enviándole un campo extra con `tauri::test::get_ipc_response`; **anotar el texto exacto** del error que recibe el frontend y si el comando llegó a ejecutarse. Los nombres de la API (`mock_builder`, `mock_context`, `noop_assets`, `get_ipc_response`, `InvokeRequest`) se confirman contra la versión instalada de `tauri`; si difieren, el test de §8 se adapta sin cambiar su intención.
5. **Columnas de auditoría:** confirmar que `init_schema` puede añadir `createdAt` y `updatedAt` a `categories`, `units`, `products` y `unitConversions` sin conflicto con las tablas de la spec 1 (son tablas nuevas, no hay migración) y anotar el formato exacto del `GLOB` que valida el timestamp.

**Concurrencia y transacciones:** igual que `plan-01` §1: una conexión compartida (`Mutex<Connection>`, `lock_conn()`), todas las funciones internas `xxx_inner` reciben `conn: &mut Connection`, y la transacción solo garantiza el rollback (el `Mutex` ya serializa los comandos).

**Testing:** `#[cfg(test)]` nativo en Rust y Vitest + jsdom en el frontend, como en la spec 1.

---

## 2. Esquema de base de datos

`init_schema(conn, now)` (en `db/schema.rs`) se **amplía** con las tablas nuevas; las de la spec 1 no se tocan. Todas usan `CREATE TABLE IF NOT EXISTS`, por lo que `init_schema` sigue siendo idempotente. Como son tablas nuevas, **no hace falta borrar la base de desarrollo** (a diferencia de la spec 1, `IF NOT EXISTS` sí las crea). **Mientras se desarrolla esta feature:** si se ajusta un `CHECK`, una columna o un índice de una tabla que ya se creó en la base local, `CREATE … IF NOT EXISTS` conserva la definición vieja y hay que **borrar la base de desarrollo** para recrearla (lo permite `Constitution.md` §4 mientras no haya datos reales). Los tests de esquema usan una base en memoria nueva y no se ven afectados (B-02). Nombres de tablas y columnas en `camelCase` (`Constitution.md` §3). `PRAGMA foreign_keys = ON` ya se activa al abrir la conexión: `ON DELETE SET NULL` y `ON DELETE CASCADE` dependen de él.

```sql
CREATE TABLE IF NOT EXISTS categories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 50),             -- tal como se escribió (7.3)
  nameNormalized TEXT NOT NULL UNIQUE CHECK (length(nameNormalized) >= 1), -- clave de unicidad y búsqueda (7.1)
  description TEXT CHECK (description IS NULL OR length(description) BETWEEN 1 AND 200),
  createdAt TEXT NOT NULL CHECK (createdAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt),   -- formato fijo de §3.3
  updatedAt TEXT NOT NULL CHECK (updatedAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt)
);

CREATE TABLE IF NOT EXISTS units (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 30),
  nameNormalized TEXT NOT NULL UNIQUE CHECK (length(nameNormalized) >= 1),
  abbreviation TEXT NOT NULL CHECK (length(abbreviation) BETWEEN 1 AND 10),
  abbreviationNormalized TEXT NOT NULL UNIQUE CHECK (length(abbreviationNormalized) >= 1),
  createdAt TEXT NOT NULL CHECK (createdAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt),   -- formato fijo de §3.3
  updatedAt TEXT NOT NULL CHECK (updatedAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt)
);

CREATE TABLE IF NOT EXISTS unitConversions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  fromUnitId INTEGER NOT NULL REFERENCES units(id) ON DELETE CASCADE,
  toUnitId INTEGER NOT NULL REFERENCES units(id) ON DELETE CASCADE,
  factorMicro INTEGER NOT NULL CHECK (factorMicro BETWEEN 1 AND 999999999999999), -- 1 from = factorMicro / 10^6 to
  createdAt TEXT NOT NULL CHECK (createdAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt),   -- formato fijo de §3.3
  updatedAt TEXT NOT NULL CHECK (updatedAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt),
  CHECK (fromUnitId <> toUnitId)
);
-- A lo sumo una conversión por par de unidades, en cualquier sentido (UM-06). El camino único se valida en código (§3.5).
CREATE UNIQUE INDEX IF NOT EXISTS unitConversionsPair
  ON unitConversions (min(fromUnitId, toUnitId), max(fromUnitId, toUnitId));

CREATE TABLE IF NOT EXISTS products (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 2 AND 100),
  nameNormalized TEXT NOT NULL CHECK (length(nameNormalized) >= 1),
  description TEXT CHECK (description IS NULL OR length(description) BETWEEN 1 AND 500),
  categoryId INTEGER REFERENCES categories(id) ON DELETE SET NULL,
  unitId INTEGER REFERENCES units(id) ON DELETE SET NULL,
  costCents INTEGER NOT NULL CHECK (costCents BETWEEN 1 AND 99999999),                 -- hasta 999.999,99 USD
  profitPercentHundredths INTEGER NOT NULL CHECK (profitPercentHundredths BETWEEN 0 AND 100000), -- 0 a 1000,00 %
  expiresOn TEXT CHECK (expiresOn IS NULL OR (
    expiresOn GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
    AND expiresOn <= '2100-12-31'
    AND date(expiresOn) IS expiresOn)),                                                -- fecha de calendario válida
  quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 0 AND 999999999),          -- hasta 999.999,999; sin DEFAULT: createProduct la recibe (> 0)
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'discontinued')),
  discontinuedAt TEXT,
  createdAt TEXT NOT NULL CHECK (createdAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt),   -- formato fijo de §3.3
  updatedAt TEXT NOT NULL CHECK (updatedAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt),
  CHECK (
    (status = 'active' AND discontinuedAt IS NULL) OR
    (status = 'discontinued' AND discontinuedAt IS NOT NULL AND discontinuedAt GLOB
      '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
      AND strftime('%Y-%m-%dT%H:%M:%SZ', discontinuedAt) IS discontinuedAt)
  )
);
-- Único solo entre productos activos: un descontinuado no bloquea el nombre (7.1, P-07).
CREATE UNIQUE INDEX IF NOT EXISTS productsActiveName ON products (nameNormalized) WHERE status = 'active';

CREATE TABLE IF NOT EXISTS notifications (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL,        -- sin CHECK de lista, a propósito: la validez la garantiza el código (spec §5)
  priority TEXT NOT NULL,    -- ídem
  message TEXT NOT NULL,
  createdAt TEXT NOT NULL CHECK (createdAt GLOB
    '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt),
  isRead INTEGER NOT NULL DEFAULT 0 CHECK (isRead IN (0, 1))
);
```

Notas de diseño:

- **Unicidad sin acentos, dos capas (`Constitution.md` §4):** las columnas `*Normalized` tienen `UNIQUE` (o el índice parcial `productsActiveName`) y las rellena el código con la **misma** función `normalize_text()` (§3.1). `COLLATE NOCASE` no se usa: solo pliega ASCII y no detectaría `Café` frente a `cafe`. El texto visible se guarda tal como se escribió una vez recortado.
- **`CHECK` que un `NULL` dejaría pasar:** en SQLite una expresión `CHECK` que evalúa a `NULL` **no falla**. Por eso la rama `discontinued` exige `discontinuedAt IS NOT NULL` antes del `GLOB`, y la caducidad usa `date(expiresOn) IS expiresOn` (con `IS`, un `date()` nulo por fecha inválida sí falla). Cada `CHECK` tiene su test con un valor que fallaría por `NULL` (§8). **Timestamps (Q-05):** el `GLOB` solo comprueba la forma, así que `2026-13-01T00:00:00Z` o `2026-01-01T25:00:00Z` lo pasarían; por eso los `CHECK` de `createdAt`, `updatedAt`, `discontinuedAt` y `notifications.createdAt` añaden `strftime('%Y-%m-%dT%H:%M:%SZ', x) IS x`: `strftime` devuelve `NULL` para una fecha imposible y con `IS` el `CHECK` falla.
- **Longitudes:** `length()` de SQLite cuenta caracteres (puntos de código), igual que 7.3. Que el texto esté recortado y con espacios colapsados no se expresa en el esquema: lo garantiza `clean_name()`/`clean_description()`. `description` vacía se guarda como `NULL` (el `CHECK` exige al menos 1 carácter).
- **Rangos escalados (C-05):** costo en centavos, porcentaje en centésimas, cantidad en milésimas y factor en millonésimas, todos como `INTEGER` con `CHECK` de rango. El mayor valor escalado es 10^15, menor que 2^53, así que cabe en `i64` y nunca pierde precisión en JS.
- **Precio de venta: derivado, no se guarda.** Se calcula al leer con `selling_price_cents(costCents, profitPercentHundredths)` (§3.4). Así no hay nada que sincronizar cuando las facturas de entrada cambien el costo (PEN-04) y todos los módulos ven un valor coherente (spec P-11). Como es derivado, no lleva `CHECK`: el máximo es ~$11.000.000 (§10).
- **Estado y fecha, consistentes en el esquema:** el `CHECK` de `status`/`discontinuedAt` impide un activo con fecha o un descontinuado sin ella. `discontinuedAt`, `notifications.createdAt` y las columnas de auditoría usan el formato fijo propio de §3.3; las columnas de la spec 1 conservan el suyo.
- **Auditoría (`createdAt`/`updatedAt`):** todas las tablas nuevas salvo `notifications` (que ya tiene `createdAt`) llevan `createdAt` y `updatedAt` con el formato fijo de §3.3. Ambas se escriben con `now` inyectado y el helper `format_utc_timestamp`. `updatedAt` se actualiza en toda escritura posterior de la fila: editar categoría, unidad o producto, editar el factor de una conversión y también descontinuar y activar un producto (cambian la fila) y la eliminación de su categoría o unidad (§3.7, Q-02: la clave foránea deja el campo en `NULL` pero no puede tocar `updatedAt`, así que el comando lo actualiza de forma explícita). **El esquema solo valida el formato del timestamp; la regla "actualizar `updatedAt` en cada escritura" vive en el código de cada comando** (ver §3.5 para el caso de `unitConversions`), y los tests de cada sección la verifican. Un comando futuro que modifique estas tablas debe respetar la misma regla. No se exponen al frontend todavía (no hay caso de uso); cuando una spec futura los necesite, se añaden al DTO sin migración.
- **Purga best-effort:** `purge_expired_products` se ejecuta en los comandos de §3.6, pero si falla **no aborta el comando**: se registra el error, se excluyen en la consulta los vencidos y se devuelve un aviso al frontend (`expiredCleanupWarning`, §3.6 y §4).
- **Cantidad:** `quantityMilli` no tiene `DEFAULT`: `createProduct` la **recibe** (texto → milésimas, mayor que 0, §3.2) y la inserta; ningún comando de esta feature la modifica después. El esquema permite 0 (un producto agotado por ventas futuras), por eso el `CHECK` es `BETWEEN 0 AND …` aunque `createProduct` exija > 0. Los tests que necesitan un producto agotado (cantidad 0) o una cantidad exacta lo insertan directamente con fixtures (§8).
- **Sin índices adicionales** sobre `categoryId`, `unitId` ni `status`/`discontinuedAt` (C-06): un solo usuario y tablas pequeñas (§2.II). Se reevalúa solo si hay evidencia de lentitud.
- **Identificadores:** todas las tablas son `AUTOINCREMENT`, el ID nunca se reutiliza. El relleno de ceros (`#00001`) es solo presentación.
- **Base inicial vacía:** `init_schema` no inserta datos semilla de estas tablas (7.8). El arranque **no** purga productos (P-08): se añade un test.
- **Conexión y apagones (M-03):** al abrir la conexión (`db/connection.rs`; ampliación, sin reabrir tareas de la spec 1) se fijan explícitamente `PRAGMA journal_mode = DELETE` y `PRAGMA synchronous = FULL`: un solo archivo de base de datos (sin `-wal`/`-shm`, fácil de copiar) y cada transacción confirmada llega al disco antes de responder, que es lo que importa con cortes de energía frecuentes. WAL se descarta: su ventaja es la concurrencia, que una aplicación de una sola sesión no necesita. Un test abre una base **en un archivo temporal** y comprueba ambos valores (en memoria `journal_mode` no aplica). La copia de seguridad periódica queda como PEN-11.

---

## 3. Reglas transversales en código

Todo vive en módulos pequeños reutilizados por los comandos, para que una regla no quede divergente entre comandos (`Constitution.md` §2.II), igual que `validators.rs` en la spec 1.

### 3.1 Texto, normalización y orden (`text.rs`, `validators.rs`)

- `clean_name(raw) -> String`: recorta los extremos y reduce **cualquier** secuencia de espacios Unicode (`char::is_whitespace`, incluidos saltos de línea y tabulaciones) a un único espacio.
- `clean_description(raw) -> Option<String>`: conserva los saltos de línea (`\r\n` y `\r` pasan primero a `\n`); dentro de cada línea reduce las secuencias de espacios y tabulaciones a un espacio y recorta la línea; recorta el conjunto; vacío → `None`.
- `normalize_text(clean) -> String`: minúsculas (`to_lowercase`) y plegado de `á é í ó ú ü` → `a e i o u u`. La `ñ` se conserva; cualquier otro carácter queda tal cual. Sin NFC/NFD (limitación aceptada conscientemente por el dueño, spec 7.1).
- `spanish_sort_key(normalized) -> String` (C-02): sustituye cada `ñ` por `n` seguida de `char::MAX`. Con la comparación por puntos de código de `String`, `nuez` < `ñame` < `oso`, y `n` < `ñ`.
- `sort_spanish(items, nombre_normalizado, id)`: `sort_by_cached_key` con `(spanish_sort_key(nombre), id)`; a igualdad de clave, por ID. **Ninguna consulta ordena por nombre en SQL** (el orden binario pondría la `ñ` después de la `z`): solo `ORDER BY id` para determinismo, y el orden se aplica en Rust sobre la lista completa (el paginador global es DOM, así que el backend devuelve todo).
- **Dónde se aplica:** categorías, unidades, conversiones (clave de la unidad de origen y luego la de destino), productos, pestaña P-13, desempate de DASH-01 y la lista de nombres de NT-01 (**antes** de truncar a 20).
- `validate_name(raw, min, max, mensajes)` y `validate_description(raw, max)` devuelven el valor ya limpio **y** su versión normalizada (como `validate_username()` en la spec 1), de modo que un comando no pueda validar sin recibir lo que debe guardar. El largo se cuenta en puntos de código (`chars().count()`).
- **Caracteres no permitidos (M-06, 7.3):** `clean_name` y `clean_description` no los eliminan; `validate_name`, `validate_description` y la validación de la abreviación **rechazan** —tras el recorte y **antes** de comprobar si el texto está vacío o de contar el largo— todo texto con un carácter de control (`char::is_control()`, categoría Cc: incluye NUL, que además truncaría `length()` de SQLite) o de formato (Cf: `is_format_char`, con la tabla de rangos de `text.rs`; por ejemplo U+00AD, U+200B–200F, U+202A–202E, U+2060–2064, U+2066–206F y U+FEFF; la lista completa se toma de la categoría Cf de Unicode al implementar y se fija con un test). Los espacios, tabulaciones y saltos de línea ya se resolvieron en el recorte. Mensajes en §4. **No se expresa en el esquema** (SQLite no distingue categorías Unicode): es una regla de código única y por eso el valor limpio solo se obtiene de `validate_*`. Consecuencia aceptada por el dueño: un emoji compuesto con U+200D (por ejemplo, una familia) se rechaza.
- **Búsqueda (C-03, 7.2):** la consulta del usuario pasa por `normalize_text(clean_name(..))` y se compara con `instr(columnaNormalizada, ?) > 0` (no `LIKE`: `%` y `_` son texto literal). Texto vacío = sin filtro. Reutiliza las columnas normalizadas que ya exige la unicidad (no se añaden columnas): nombre de producto, categoría y unidad, y abreviación de unidad (OR). El indicador `isExactMatch` (para decidir "Agregar nueva…") es la igualdad exacta de la consulta normalizada con el nombre (o la abreviación) normalizado.

### 3.2 Números y dinero (`validators.rs`)

`parse_scaled(texto, reglas) -> Result<i128, DecimalError>` es el **único** lector de números de la feature (precio, %, factor, cantidad del Conversor, vista previa). Recibe en `reglas` el **`max_entero_digitos`** del campo (8, 9, 15, 15 o 6 según el campo, abajo), que sustituye a cualquier límite fijo. Evalúa en el orden de 7.4 y devuelve la **primera** falla:

1. **Formato:** vacío, o algo distinto de dígitos con a lo sumo **un** separador (`,` o `.`) con al menos un dígito a cada lado (se rechazan `,5`, `5,`, `1e3`, `+5`, `-5` y espacios). Solo cuentan los dígitos ASCII `0-9`: cifras de otros alfabetos o de ancho completo (`١٢`, `１２`) se rechazan por formato.
2. **Ambigüedad del punto:** un solo punto, sin coma, exactamente 3 dígitos después y parte entera numéricamente distinta de 0 (`1.234` se rechaza; `0.123` y `007` se aceptan).
3. **Exceso de decimales:** más de los permitidos, contados tal como se escribieron.
4. **Rango:** mínimo y máximo del valor ya escalado. **El número máximo de dígitos de la parte entera lo define cada campo** y `parse_scaled` lo recibe como parámetro (`max_entero_digitos`): costo 8, cantidad inicial 9, factor de conversión 15, cantidad del Conversor 15 y porcentaje 6. Una parte entera con más dígitos que el máximo del campo cuenta como fuera de rango, sin desbordar.

Cada campo envuelve el error con su texto (catálogo, §4): `parse_cost_cents` (>0 y ≤ 999.999,99; 2 decimales; 8 dígitos enteros), `parse_percent_hundredths` (0 a 1000,00; 2; 6 dígitos enteros), `parse_quantity_milli` (**cantidad inicial del producto**: >0 y ≤ 999.999,999; 3 decimales; 9 dígitos enteros; devuelve milésimas), `parse_factor_micro` (0,000001 a 999.999.999,999999; 6; 15 dígitos enteros) y `parse_quantity_micro` (cantidad del **Conversor**: >0 y ≤ 999.999.999,999999; 6; 15 dígitos enteros; devuelve millonésimas). Toda la aritmética es entera, sin `f64`. Los números viajan al frontend así:

- **Precios:** `costCents` y `sellingPriceCents` como enteros (centavos); el frontend divide entre 100 solo para mostrar (T-07).
- **Decimales (cantidad, factor, porcentaje, resultado del Conversor):** como **texto decimal con coma**, sin separador de miles y sin ceros finales (el porcentaje, siempre con 2 decimales). Con coma, un valor devuelto vuelve a pasar la regla de 7.4 si el formulario lo precarga al editar (un punto como `1.234` sería rechazado por ambigüedad). El frontend no los pasa por `Number` ni `parseFloat`.

### 3.3 Fechas y formato de timestamps (`timestamps.rs`, C-04 y C-10)

- **Hoy local (C-04):** las funciones internas que necesitan "hoy" (caducidad de 7.5 y filtros de P-12) reciben `today: NaiveDate` además de `now: DateTime<Utc>`; el envoltorio lo obtiene con `chrono::Local::now().date_naive()`. Las demás siguen recibiendo solo `now` (convención de `plan-01` §3).
- **Formato fijo (C-10):** `YYYY-MM-DDTHH:MM:SSZ` (segundos, sufijo `Z`, sin fracción), producido por un único helper `format_utc_timestamp(DateTime<Utc>) -> String` (trunca a segundos) y leído por `parse_utc_timestamp(&str) -> Option<DateTime<Utc>>`. `discontinuedAt`, `notifications.createdAt` y los `createdAt`/`updatedAt` de las tablas nuevas se escriben **siempre como `String` con ese helper**: no se usa el `ToSql`/`FromSql` de `DateTime` de `rusqlite` en esas columnas, porque escribe con espacio y fracción variable y mezclarlo con otro formato rompería la comparación de texto. Los formatos de la spec 1 (`createdAt`, `lastActivityAt`, `errorLogs`) no cambian.
- **Lectura inválida (Q-05):** con los `CHECK` de §2, `parse_utc_timestamp` solo devuelve `None` si la fila está corrupta (por ejemplo, editada fuera de la aplicación). Quien lo lee y recibe `None` (cálculo de `remainingDays`) responde `internal` —se registra, sin valores— en lugar de inventar un número; la purga no depende de esto porque compara texto.
- **Caducidad (7.5):** `validate_expiry_date(texto, today, actual)` exige exactamente `YYYY-MM-DD` (10 caracteres; `NaiveDate::parse_from_str` por sí solo acepta `2026-2-3`), fecha de calendario real (`2026-02-30` se rechaza) y año ≤ 2100. Solo rechaza una fecha anterior a `today` si es **nueva o cambió** respecto de la guardada: una caducidad ya vencida que no se modifica se acepta al editar. Se compara y muestra sin zona horaria (la fecha es un día de calendario; el frontend no la pasa por `Date`, que la desplazaría por UTC).
- **Días restantes de un descontinuado:** `60 días − (now − discontinuedAt)`, en segundos y redondeado hacia arriba a días, acotado a 0–60 (un cambio de reloj nunca da valores negativos ni mayores que 60).

### 3.4 Precios (`pricing.rs`, P-10 y P-11)

- `selling_price_cents(cost_cents, percent_hundredths) -> i64`: `round_half_up(cost × (10000 + pct) / 10000)` con `i128`. Ejemplo: 1250 centavos y 2000 centésimas → 1500.
- `weighted_average_cost_cents(qty_actual_milli, cost_actual, qty_comprada_milli, cost_compra) -> Result<i64, AppError>`: los cuatro argumentos se reciben como `i128` para verificar cada operación; los valores de SQLite (`i64`) se amplían en el punto de llamada. Calcula `round_half_up((qa·ca + qb·cb) / (qa + qb))`. Rechaza cantidad comprada o costo de la compra ≤ 0. Con cantidad actual 0 el resultado es exactamente el costo de la compra. Ejemplo: 10 a $1,00 y 5 a $2,00 → 133,33 → `133`; es justo el caso de un producto creado con 10 unidades a $1,00 (cantidad y costo iniciales, S-9) que recibe una compra de 5 a $2,00.
- **Vista previa (P-11), costo vigente:** `preview_selling_price_inner(conn, now, input)` recibe el `%` como texto y **una** de dos fuentes de costo: al **editar**, el `productId` (el costo lo lee el backend **de la base de datos en ese momento**, nunca lo envía el frontend, para que el precio mostrado no diverja del guardado aunque el costo haya cambiado desde que se abrió el formulario); al **crear**, aún no existe el producto y no hay costo en la base, así que el formulario envía el `cost` escrito, como texto, que se valida con `parse_cost_cents`. Enviar ambos o ninguno es un error de contrato (`validationFailed`, "Los datos enviados no son válidos"). Usa la misma `selling_price_cents` que el guardado. **No escribe ni purga** (spec P-11): si el producto no existe (o es un descontinuado ya vencido, que se excluye en la consulta como en §3.6) responde `notFound` "El producto ya no existe"; si está descontinuado, `invalidState` con el mismo texto que editar.
- **P-10 no tiene consumidor** hasta las facturas de entrada (PEN-04): es un módulo `pub` del crate (declarado en `lib.rs`), con sus tests y sin warnings de `clippy`. Registrado en §9.

### 3.5 Camino único y Conversor (`commands/conversions.rs`)

- **Regla:** que entre dos unidades haya a lo sumo un camino equivale a que el grafo de conversiones (no dirigido) **no tenga ciclos**. Registrar A↔B solo crea un segundo camino si A y B **ya están conectadas**.
- **Alta (`createConversion`)**, en una transacción, en este orden (la primera regla incumplida es la que se informa): misma unidad → factor (7.4) → unidades existentes → conversión directa ya existente (en cualquier sentido, consulta por `min/max` del par) → unidades ya conectadas a través de otras (se cargan todas las conversiones y se recorre el grafo desde A en memoria) → `INSERT`. El índice único `unitConversionsPair` queda como última defensa.
- **Editar factor** no altera el grafo; **eliminar** una conversión o una unidad (cascada) solo quita aristas. Al editar el factor, `updateConversionFactor` actualiza `updatedAt` con `format_utc_timestamp(now)` y **no** toca `createdAt` (coherente con la regla general de auditoría de §2).
- **Conversor (`convertQuantity`, UM-07):** sigue el camino único. Como grafo sin ciclos, un recorrido simple desde el origen encuentra el único camino. Valor exacto como fracción `N/D` (empieza en `cantidadMicro / 10^6`); por tramo "1 A = f B": de A a B `N·=f, D·=10^6`; de B a A `N·=10^6, D·=f`. Tras cada tramo se **reduce por el máximo común divisor** (sin esto, una cadena realista como tonelada→kilo→gramo→miligramo desbordaría `i128` sin necesidad). Al final, `resultadoMicro = round_half_up(N·10^6 / D)`. Cada multiplicación es `checked_*`: un **desborde del cálculo** se rechaza con "No se pudo calcular esta conversión: la cadena de unidades es demasiado larga" y un **resultado mayor que 999.999.999,999999** con "El resultado es demasiado grande para mostrarse" (ambos `validationFailed`, M-07), de modo que una cadena larga de factores con resultado pequeño no se reporte como "resultado grande". Misma unidad de origen y destino: devuelve la cantidad (formateada sin ceros finales). Sin camino: `validationFailed` con su mensaje (no `notFound`, para no llenar el log con un resultado esperado). El resultado puede ser `0` si el redondeo a 6 decimales lo anula. Solo lee: no crea ni modifica nada.

### 3.6 Purga de productos vencidos (C-06, P-08)

- Una única función `purge_expired_products(conn, now) -> Result<usize, AppError>`: `DELETE FROM products WHERE status = 'discontinued' AND discontinuedAt <= ?` con el umbral `format_utc_timestamp(now − 60 días)`. Ambos lados truncan a segundos, así que "60 × 24 horas exactos" se purga y 59 días 23:59:59 no.
- **Quién la ejecuta:** todo comando que **liste o cuente** descontinuados (`listCategories`, `getCategory`, `deleteCategory`, `listUnits`, `deleteUnit`, `listOutOfStockAndDiscontinued`) y todo comando que **reciba un `productId`** (`getProduct`, `updateProduct`, `discontinueProduct`, `activateProduct`, `deleteProduct`). Regla uniforme y simple: se ejecuta tras `require_session`, **antes** de contar, listar o actuar, y un producto vencido responde "El producto ya no existe". No se purga al arrancar ni en segundo plano (P-08).
- **Transacción:** `require_session` se resuelve **antes** de abrir la transacción (como `validate_session` en `plan-01` §3: su borrado de sesiones vencidas debe confirmarse aunque el comando falle). Después, purga y lectura/escritura posterior van en **una sola transacción**, para que lo contado o listado sea coherente con lo purgado.
- **Purga best-effort:** si la purga falla (por ejemplo, un `DELETE` bloqueado, un índice corrupto o un trigger), **el comando no se aborta**. La purga corre dentro de un `SAVEPOINT` propio: si falla se hace `ROLLBACK TO` y la transacción del comando sigue utilizable. Se registra el error (`internal`, con su `cause`, sin nombres de productos) y se continúa con la lectura, **excluyendo en la consulta los productos vencidos**: toda consulta de esos comandos que lista, cuenta o busca descontinuados añade `AND (status <> 'discontinued' OR discontinuedAt > ?)` con el mismo umbral de 60 días, de modo que un vencido no se cuenta ni se muestra y, si se pidió por `productId`, responde "El producto ya no existe". **Ejemplos del filtro por tipo de consulta** (`umbral` = `format_utc_timestamp(now − 60 días)`):
  - **Lista** (categorías, unidades, pestaña P-13, detalle): `SELECT … FROM products WHERE … AND (status <> 'discontinued' OR discontinuedAt > ?)` (en `listOutOfStockAndDiscontinued`, además `status = 'discontinued'` sigue siendo válido como filtro, pero la condición de vencido se aplica siempre).
  - **Conteo** (productos por categoría o unidad, `productCount` de los listados, `affectedProducts` de las eliminaciones): `SELECT COUNT(*) FROM products WHERE categoryId = ? AND (status <> 'discontinued' OR discontinuedAt > ?)` (o `unitId = ?`).
  - **Por `productId`** (`getProduct`, `updateProduct`, `discontinueProduct`, `activateProduct`, `deleteProduct`): `SELECT … FROM products WHERE id = ? AND (status <> 'discontinued' OR discontinuedAt > ?)`; si no devuelve fila, `notFound` "El producto ya no existe".

  Un helper `purge_best_effort(conn, now) -> Option<String>` devuelve el aviso: tras el fallo cuenta los vencidos (`status = 'discontinued' AND discontinuedAt <= ?`) y, si son N > 0, devuelve "No se pudieron limpiar N productos vencidos; se reintentará en la próxima operación"; si N = 0 o la purga funcionó devuelve `None`. **Si el propio conteo falla (Q-10)**, el helper registra ese segundo error y devuelve el aviso **sin cifra** ("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación"); nunca propaga el fallo al comando. **Si el propio `SAVEPOINT` falla** (caso no soportado: implicaría que la transacción del comando ya está rota por un error grave de SQLite), el comando **aborta con `internal`** genérico, registra el error y no devuelve el listado: no tiene sentido continuar sobre una transacción que no se puede recuperar con `ROLLBACK TO`. **La transacción abierta se abandona y `rusqlite` hace rollback al dropear el guard**, así que no queda una transacción colgada: no se intenta "recuperar" la transacción a mano. El aviso viaja en el campo opcional `expiredCleanupWarning: string | null` de la respuesta (§4); en un error de la propia operación no hay respuesta que lo lleve, pero el fallo ya quedó registrado. La purga se reintenta en la próxima operación.
- **Aclaración de redacción (Q-11):** "excluir en la consulta" es un filtro SQL (`AND (status <> 'discontinued' OR discontinuedAt > ?)`): no borra nada ni mantiene listas en memoria.
- **Motivo:** la purga es un `DELETE` simple y su fallo no debería bloquear toda la aplicación. El usuario nunca ve un producto vencido (se filtran en la consulta), la app sigue siendo usable y el fallo queda registrado para diagnóstico.
- **`previewSellingPrice` no purga:** es de solo lectura (spec P-11) y solo se llama al editar un producto activo.
- Los comandos de solo lectura que purgan son una excepción consciente a "las lecturas no escriben"; el mismo patrón ya existe en `validate_session`.

### 3.7 Eliminación de categoría y unidad con notificación (C-07)

Una única transacción, en este orden: **(1)** purga; **(2)** la entidad existe (si no, `notFound`); **(3)** leer los nombres de los productos afectados **antes** del `DELETE` (con `ON DELETE SET NULL` el vínculo se pierde al borrar), ordenados con `sort_spanish`, y su total real; **(4)** si hay afectados, insertar la notificación (`N-09` para categoría, `N-08` para unidad; prioridad `ADVERTENCIA`; mensaje "Los siguientes productos quedaron sin categoría: …" / "…sin unidad de medida: …"; hasta 20 nombres separados por coma y, si hay más, " y N más"); **(5)** `UPDATE products SET updatedAt = ? WHERE categoryId = ?` (o `unitId = ?`) con `format_utc_timestamp(now)` (Q-02: la FK `ON DELETE SET NULL` cambia esas filas pero no puede tocar `updatedAt`); **(6)** `DELETE` (en unidades, la cascada de conversiones queda dentro de la misma transacción; antes se cuentan para el resultado); **(7)** `commit`. El comando devuelve los conteos reales (`affectedProducts`, y `deletedConversions` en unidades). Si falla cualquier paso, nada se aplica. El `cause` de un log **nunca** incluye nombres de productos.

---

## 4. Comandos Tauri

Convenciones de `plan-01` §4: nombre lógico `camelCase` (el del wrapper de `services/*.js`), función Rust `snake_case` (`createProduct` → `create_product`) y es el nombre que recibe `invoke()`, escrito solo dentro de los servicios. Los **argumentos** los convierte Tauri; las **respuestas** llevan `#[serde(rename_all = "camelCase")]`. **`today` nunca lo envía el frontend (Q-11):** donde una fila de la tabla dice que el comando "recibe `today`", se refiere a su función interna; el envoltorio lo calcula con `chrono::Local::now().date_naive()` y los argumentos de `invoke()` no lo incluyen. Todo comando devuelve `Result<T, AppError>`, con envoltorio delgado sobre `xxx_inner(conn: &mut Connection, now, [today,] …)` y registro desde el envoltorio. Los módulos nuevos se declaran en `lib.rs` y los comandos se registran en `generate_handler!`.

**Autorización:** los 26 comandos exigen sesión. Cada `xxx_inner` empieza con `let _actor = require_session(conn, now)?;` antes de tocar ningún dato; ambos roles pueden ejecutarlos todos (spec §4). Ninguno actualiza `lastActivityAt` (eso es solo de `touchSession()`). La mutación de `users`/`sessions` sigue regida por `Agents.md` §3 y esta feature no las toca.

**Contrato de entrada (C-01):** con argumentos individuales Tauri **ignora en silencio** las claves extra, así que `createProduct` y `updateProduct` reciben **un único argumento `input`**: un struct `#[serde(deny_unknown_fields, rename_all = "camelCase")]` que no declara los campos prohibidos (**precio de venta** al crear; **cantidad, precio de venta y costo** al editar). `createProduct` sí declara `cost` y `quantity` (ambos como texto: la cantidad inicial es obligatoria). El rechazo lo hace Tauri antes de llamar al comando: llega al frontend como texto (no `AppError`) y no pasa por `errorLogs`; `invoke.js` lo convierte en "Los datos enviados no son válidos" y lo escribe en `console.error` con el nombre del comando (A-03: un argumento mal escrito en un wrapper no queda mudo; nada se guarda en `errorLogs`, decisión del dueño). Por simetría, los demás structs de entrada (filtros) usan también `deny_unknown_fields`, y sus campos opcionales llevan `#[serde(default)]` (B-05): un `filters` que omite `stock`, `expiry` o `withoutCategory` toma `all`, `all` y `false`; sin eso, un campo omitido se rechazaría como error de contrato. `updateProduct` es de **reemplazo completo** de los campos editables: lo ausente se guarda como vacío (la interfaz siempre envía todos).

**Orden de validación** (se informa la primera regla incumplida): campos de texto (recorte → caracteres no permitidos → vacío → largo) → números (por campo, en el orden del formulario —precio de entrada, cantidad inicial, porcentaje— y, dentro de cada uno, el orden de 7.4) → caducidad → existencia de `categoryId`/`unitId` ("ya no existe") → unicidad (la decide el `INSERT`/`UPDATE`, como en `plan-01` §2). Los errores de `rusqlite` pasan por `map_db_error(error, operación, duplicados)`: una violación `UNIQUE` se traduce a `duplicateField` con el mensaje de la columna que reporta SQLite según la tabla `(columna → mensaje)` que cada comando pasa (`categories.nameNormalized`, `units.nameNormalized`, `units.abbreviationNormalized`, `products.nameNormalized` e `index 'unitConversionsPair'`); una violación de `CHECK` o de clave foránea inesperada es `internal`. Un test por columna protege contra cambios de formato del mensaje de SQLite.

### Comandos

| Comando | Módulo | Cubre |
|---|---|---|
| `listCategories(search?)` | `commands/categories.rs` | CAT-02. Purga. Devuelve `CategoryList` (`items: CategoryListItem[]` con `productCount`, incluye descontinuados, y `expiredCleanupWarning`). Búsqueda por nombre (§3.1). Orden español. |
| `searchCategories(text)` | `categories.rs` | CAT-01/CAT-05: coincidencias para el aviso de duplicado y el combobox, con `isExactMatch`. Sin purga (no cuenta). |
| `getCategory(categoryId)` | `categories.rs` | CAT-06. Purga. Detalle con la lista de productos (activos y descontinuados) y `expiredCleanupWarning`. |
| `createCategory(name, description?)` | `categories.rs` | CAT-01, CAT-05. Devuelve la categoría creada. |
| `updateCategory(categoryId, name, description?)` | `categories.rs` | CAT-03. La unicidad la decide el `UPDATE` (excluye la propia fila). |
| `deleteCategory(categoryId)` | `categories.rs` | CAT-04, NT-01 (N-09). Transacción de §3.7. |
| `listUnits(search?)` | `commands/units.rs` | UM-02. Purga. Devuelve `UnitList` (`items` con `productCount` y `conversionCount`, y `expiredCleanupWarning`). Búsqueda por nombre y abreviación. |
| `searchUnits(text)` | `units.rs` | UM-01/UM-05, con `isExactMatch` (por nombre o por abreviación). |
| `createUnit(name, abbreviation)` | `units.rs` | UM-01, UM-05. |
| `updateUnit(unitId, name, abbreviation)` | `units.rs` | UM-03. |
| `deleteUnit(unitId)` | `units.rs` | UM-04, NT-01 (N-08), cascada de conversiones (§3.7). |
| `listConversions(search?)` | `commands/conversions.rs` | UM-06. Búsqueda por nombre o abreviación de cualquiera de sus dos unidades. Orden por unidad de origen y destino. |
| `createConversion(fromUnitId, toUnitId, factor)` | `conversions.rs` | UM-06 (§3.5). `factor` viaja como texto. |
| `updateConversionFactor(conversionId, factor)` | `conversions.rs` | UM-06 (solo el factor). |
| `deleteConversion(conversionId)` | `conversions.rs` | UM-06. |
| `convertQuantity(fromUnitId, toUnitId, quantity)` | `conversions.rs` | UM-07. Solo lectura; resultado como texto decimal. |
| `listProducts(filters)` | `commands/products.rs` | P-09, P-12. Solo activos (incluye agotados). `filters`: `search?`, `categoryId?`, `withoutCategory`, `stock` (`all`/`available`/`outOfStock`/`low`), `expiry` (`all`/`withExpiry`/`withoutExpiry`/`expired`/`expiringSoon`). `categoryId` y `withoutCategory` a la vez es un error de contrato. Los campos omitidos toman su valor por defecto (B-05). `today` lo calcula el envoltorio (no es argumento del frontend). No purga (no lista descontinuados). |
| `listOutOfStockAndDiscontinued(search?, condition?)` | `products.rs` | P-05, P-13. Purga. `condition`: `all`/`outOfStock`/`discontinued`. Devuelve `ProductConditionList` (`items` y `expiredCleanupWarning`). |
| `getProduct(productId)` | `products.rs` | P-02, P-06. Purga. Incluye estado, fecha de descontinuación y días restantes. |
| `createProduct(input)` | `products.rs` | P-01. Recibe la **cantidad inicial** (> 0) como texto. `today` lo calcula el envoltorio (no es argumento del frontend). |
| `updateProduct(input)` | `products.rs` | P-03. Solo activos (`invalidState` si está descontinuado). `today` lo calcula el envoltorio (no es argumento del frontend). Devuelve `UpdateProductResult` (producto y `expiredCleanupWarning`). |
| `previewSellingPrice(input)` | `products.rs` | P-11. `input`: `profitPercent` (texto) y **`productId`** (al editar; el costo lo lee el backend de la BD) o **`cost`** (al crear, aún sin producto); ambos o ninguno es error de contrato (§3.4). Mismas reglas y misma función que el guardado; no escribe ni purga. Devuelve `sellingPriceCents`. |
| `discontinueProduct(productId)` | `products.rs` | P-04. Registra `format_utc_timestamp(now)`. Devuelve `ProductActionResult`. |
| `activateProduct(productId)` | `products.rs` | P-07. Bloquea si existe otro activo con el mismo nombre (lo decide el índice parcial; mensaje propio de activación). Devuelve `ProductActionResult`. |
| `deleteProduct(productId)` | `products.rs` | P-14. Activos y descontinuados. Sin notificación. Devuelve `ProductActionResult`. |
| `getDashboardSummary()` | `commands/dashboard.rs` | DASH-01: total de agotados activos, hasta 3 categorías con más agotados (desempate alfabético) y agotados sin categoría. Solo activos, por eso no purga. |

### DTOs (contrato de datos Rust ↔ JS)

Todo struct devuelto lleva `#[serde(rename_all = "camelCase")]`. Los textos decimales son los de §3.2.

| DTO | Lo devuelven | Campos (como los ve JS) |
|---|---|---|
| `CategoryListItem` | `createCategory`, `updateCategory` y cada elemento de `CategoryList` | `id`, `name`, `description: string \| null`, `productCount` |
| `CategoryList` | `listCategories` | `items: CategoryListItem[]`, `expiredCleanupWarning: string \| null` |
| `CategoryMatch` | `searchCategories` | `id`, `name`, `isExactMatch` |
| `CategoryDetail` | `getCategory` | `id`, `name`, `description`, `productCount`, `products: ProductBrief[]`, `expiredCleanupWarning` |
| `ProductBrief` | `getCategory` | `id`, `name`, `isOutOfStock`, `isDiscontinued` |
| `DeleteCategoryResult` | `deleteCategory` | `affectedProducts`, `expiredCleanupWarning` |
| `UnitListItem` | `createUnit`, `updateUnit` y cada elemento de `UnitList` | `id`, `name`, `abbreviation`, `productCount`, `conversionCount` |
| `UnitList` | `listUnits` | `items: UnitListItem[]`, `expiredCleanupWarning` |
| `UnitMatch` | `searchUnits` | `id`, `name`, `abbreviation`, `isExactMatch` |
| `DeleteUnitResult` | `deleteUnit` | `affectedProducts`, `deletedConversions`, `expiredCleanupWarning` |
| `ConversionItem` | `listConversions`, `createConversion`, `updateConversionFactor` | `id`, `fromUnit: {id, name, abbreviation}`, `toUnit: {…}`, `factor: string` |
| `ConversionResult` | `convertQuantity` | `result: string` |
| `ProductListItem` | `listProducts`, `createProduct` y el campo `product` de `UpdateProductResult` | `id`, `name`, `categoryId`, `categoryName`, `unitId`, `unitName`, `unitAbbreviation` (los cinco pueden ser `null`), `quantity: string`, `costCents`, `sellingPriceCents`, `profitPercent: string`, `expiresOn: string \| null`, `isOutOfStock`, `isLowStock` |
| `ProductConditionItem` | cada elemento de `ProductConditionList` | `id`, `name`, `categoryName`, `unitName`, `unitAbbreviation`, `quantity`, `isOutOfStock`, `isDiscontinued`, `remainingDays: number \| null` |
| `ProductConditionList` | `listOutOfStockAndDiscontinued` | `items: ProductConditionItem[]`, `expiredCleanupWarning` |
| `UpdateProductResult` | `updateProduct` | `product: ProductListItem`, `expiredCleanupWarning` |
| `ProductActionResult` | `discontinueProduct`, `activateProduct`, `deleteProduct` | `expiredCleanupWarning` |
| `ProductDetail` | `getProduct` | los de `ProductListItem` más `description`, `isDiscontinued`, `discontinuedAt: string \| null` (ISO 8601 UTC), `remainingDays` y `expiredCleanupWarning` |
| `PriceQuote` | `previewSellingPrice` | `sellingPriceCents` |
| `DashboardSummary` | `getDashboardSummary` | `outOfStockTotal`, `topCategories: [{name, count}]` (hasta 3), `withoutCategoryCount` |
| *(vacío)* | `deleteConversion` | — |

`isOutOfStock` es `cantidad = 0`; `isLowStock` es `0 < cantidad < 5` (P-12); ninguno incluye `Valor Total` ni `Ingresos` (PEN-10).

**Aviso de purga (`expiredCleanupWarning`):** campo `string | null` de las respuestas de los 11 comandos que purgan (`listCategories`, `getCategory`, `deleteCategory`, `listUnits`, `deleteUnit`, `listOutOfStockAndDiscontinued`, `getProduct`, `updateProduct`, `discontinueProduct`, `activateProduct`, `deleteProduct`). Es `null` cuando la purga funcionó (o no había nada que limpiar) y contiene el texto del aviso (§3.6) cuando falló. Los comandos que devolvían un arreglo o nada pasan a devolver un struct con ese campo (`CategoryList`, `UnitList`, `ProductConditionList`, `UpdateProductResult`, `ProductActionResult`); `listProducts`, `createProduct` y `previewSellingPrice` no purgan y no cambian.

### Catálogo de errores (código → texto exacto)

`errors.rs` **añade** el código `invalidState` y no cambia los existentes ni `AppError::should_log()`: `validationFailed` e `invalidCredentials` no se registran (`Constitution.md` v1.2.0 §2.VI); `duplicateField`, `notFound`, `invalidState` e `internal` sí. "Ya no existe" se mantiene en los textos de esta feature; los de la spec 1 ("El usuario no existe") no se tocan. Los textos que la spec solo describe quedan fijados aquí (**confirmados por el dueño**, §10).

| Código | Condición | Texto |
|---|---|---|
| `validationFailed` | nombre vacío (categoría, unidad o producto) | El nombre es obligatorio |
| `validationFailed` | nombre de categoría > 50 / de unidad > 30 / de producto > 100 | El nombre no puede tener más de {50 \| 30 \| 100} caracteres |
| `validationFailed` | nombre de producto de 1 carácter | El nombre debe tener al menos 2 caracteres |
| `validationFailed` | descripción > 200 (categoría) / > 500 (producto) | La descripción no puede tener más de {200 \| 500} caracteres |
| `validationFailed` | abreviación vacía / > 10 | La abreviación es obligatoria / La abreviación no puede tener más de 10 caracteres |
| `validationFailed` | nombre, abreviación o descripción con caracteres de control o de formato (M-06) | El nombre contiene caracteres no permitidos / La abreviación contiene caracteres no permitidos / La descripción contiene caracteres no permitidos |
| `validationFailed` | número: formato (campo = El precio de entrada, La cantidad inicial, El porcentaje de ganancia, El factor de conversión, La cantidad del Conversor) | {Campo} no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales |
| `validationFailed` | número: ambigüedad del punto (`1.234`) | {Campo} no usa separador de miles. Escribe los decimales con coma (por ejemplo, 1,234) |
| `validationFailed` | número: exceso de decimales | {Campo} admite como máximo {2 \| 3 \| 6} decimales |
| `validationFailed` | precio de entrada fuera de rango | El precio de entrada debe ser mayor que 0 y no superar 999.999,99 |
| `validationFailed` | cantidad inicial fuera de rango | La cantidad inicial debe ser mayor que 0 y no superar 999.999,999 |
| `validationFailed` | porcentaje fuera de rango | El porcentaje de ganancia debe estar entre 0 y 1.000,00 |
| `validationFailed` | factor fuera de rango | El factor de conversión debe estar entre 0,000001 y 999.999.999,999999 |
| `validationFailed` | cantidad del Conversor fuera de rango | La cantidad debe ser mayor que 0 y no superar 999.999.999,999999 |
| `validationFailed` | caducidad no es una fecha válida | La fecha de caducidad no es válida |
| `validationFailed` | caducidad con año > 2100 | La fecha de caducidad no puede ser posterior al año 2100 |
| `validationFailed` | caducidad anterior a hoy (nueva o modificada) | La fecha de caducidad no puede ser anterior a hoy |
| `validationFailed` | conversión con la misma unidad de origen y destino | Elige dos unidades distintas |
| `validationFailed` | conversor sin camino entre las unidades | No hay una conversión registrada entre esas unidades |
| `validationFailed` | conversor: resultado mayor que el tope | El resultado es demasiado grande para mostrarse |
| `validationFailed` | conversor: desborde del cálculo (cadena larga o factores grandes) | No se pudo calcular esta conversión: la cadena de unidades es demasiado larga |
| `validationFailed` | `categoryId` y `withoutCategory` a la vez | Los datos enviados no son válidos |
| *(texto de `invoke.js`)* | rechazo de Tauri por campo no admitido o argumento faltante (patrón fijado por el spike de §1) | Los datos enviados no son válidos |
| *(texto de `invoke.js`)* | cualquier otro error sin forma `AppError` (fallo de IPC, comando no registrado, error de serialización) (Q-09) | Ocurrió un error inesperado |
| `duplicateField` | nombre de categoría repetido | Ya existe una categoría con ese nombre |
| `duplicateField` | nombre de unidad repetido | Ya existe una unidad con ese nombre |
| `duplicateField` | abreviación repetida | Ya existe una unidad con esa abreviación |
| `duplicateField` | nombre de producto activo repetido (crear o editar) | Ya existe un producto con ese nombre |
| `duplicateField` | activar con otro activo del mismo nombre | No se puede activar: ya existe otro producto activo con ese nombre |
| `duplicateField` | conversión directa ya existente (cualquier sentido) | Ya existe una conversión entre esas dos unidades. Edítala para cambiar el factor |
| `duplicateField` | segundo camino entre dos unidades ya relacionadas | Esas unidades ya están relacionadas a través de otras conversiones. Elimina alguna para crear esta |
| `notFound` | entidad inexistente o purgada | El producto ya no existe / La categoría ya no existe / La unidad ya no existe / La conversión ya no existe |
| `invalidState` | editar un descontinuado | Un producto descontinuado no se puede editar. Actívalo primero |
| `invalidState` | descontinuar uno ya descontinuado | El producto ya está descontinuado |
| `invalidState` | activar uno ya activo | El producto ya está activo |
| `internal` | cualquier error interno (incluido un fallo de la purga, una violación de `CHECK` o de clave foránea inesperada) | Ocurrió un error inesperado |

El `cause` que va a `errorLogs` indica módulo, operación y el código/nombre de restricción de SQLite, y **nunca** valores ingresados por el usuario (nombres, descripciones, números); se prueba.

---

## 5. Frontend: servicios, vistas y componentes

Se mantiene la separación de `plan-01` §6: las **vistas** manejan DOM y eventos; los **servicios** son la única capa que llama `invoke()` (siempre a través de `invoke.js`, que sigue siendo el único punto con `window.__TAURI__.core.invoke`). Sin estado propio fuera de la última respuesta como caché de vista; tras cada mutación se recarga (T-01). Toda vista tiene estados de carga, vacío ("Aún no hay categorías") y error (T-01, 7.8): la base empieza vacía y no hay datos semilla.

- **`services/invoke.js` (se amplía, C-01):** clasifica los errores **sin forma `AppError`** (Q-09): si el texto es un rechazo de argumentos de Tauri (patrón anotado en el spike de §1, p. ej. `invalid args …` o `… missing required key …`) lo convierte en "Los datos enviados no son válidos"; **cualquier otro** (fallo de IPC, comando no registrado, error de serialización) lo convierte en el genérico "Ocurrió un error inesperado", para no atribuir al usuario un fallo que no es suyo. **Además escribe ese error en `console.error` con el nombre del comando y el texto que dio Tauri** (A-03), solo en la consola de desarrolladores y nunca en `errorLogs` (decisión del dueño): un argumento mal escrito en un wrapper deja así un rastro en desarrollo. Los errores con forma `AppError` se siguen devolviendo intactos, como hoy: el test existente debe seguir en verde. Es una excepción acotada a "Rust es dueño de los textos" (§9).
- **Servicios nuevos:** `categoriesService.js`, `unitsService.js` (incluye conversiones y Conversor), `productsService.js` y `dashboardService.js`, con wrappers `camelCase`; el nombre `snake_case` solo aparece ahí.
- **Vistas nuevas (módulos ES):** `categoriasView.js` (dueño único de la sección Categorías, incluido el detalle), `unidadesView.js` (unidades, modal de unidad, conversiones y Conversor Rápido), `productosView.js` (listado, pestaña Agotados y Descontinuados, formulario, detalle) y `dashboardView.js` (tarjeta de agotados y sus botones).
- **Componentes nuevos:** `components/combobox.js` (T-06): input con lista de coincidencias que entrega el backend (`searchCategories`/`searchUnits`), opción "Agregar nueva …" solo si ninguna coincidencia tiene `isExactMatch`, botón "Sin categoría"/"Sin unidad" en edición y estado "texto escrito sin elegir opción" para que el formulario rechace guardar. **Teclado y accesibilidad (Q-06):** `role="combobox"` con lista `role="listbox"` y opciones `role="option"`, `aria-expanded` y `aria-activedescendant`; ↑/↓ recorren las opciones, Enter elige la resaltada, Escape cierra la lista sin elegir y Tab la cierra y sigue. **Doble coincidencia exacta (Q-07):** si una unidad coincide por nombre y otra por abreviación, se muestran ambas y no se ofrece "Agregar nueva". **Formato de cada opción (Q-07):** cada opción de **unidad** muestra siempre **"nombre (abreviación)"** —por ejemplo, `kg (k)` y `Kilogramo (kg)` para una unidad llamada `kg` con abreviación `k` y otra llamada `Kilogramo` con abreviación `kg`—, de modo que el usuario distingue dos coincidencias exactas a simple vista; el formato es el mismo con una sola coincidencia y con varias. Las opciones de categoría muestran solo el nombre (no tienen abreviación). `components/confirmDialog.js` (T-04): diálogo con `.modal-overlay` y `.modal-card`, texto multilínea con conteos, que devuelve una promesa; reemplaza cada `confirm()`. **Foco (Q-06):** `role="dialog"` y `aria-modal="true"`; el foco inicial va al botón **Cancelar** (acción segura en confirmaciones destructivas), Tab y Shift+Tab ciclan dentro del diálogo, Escape cancela y al cerrar el foco vuelve al elemento que lo abrió. El modal de alta de unidad sigue las mismas reglas de foco.
- **`utils/formatters.js`:** dinero (centavos → USD con dos decimales, `es-ES`), fecha `dd/mm/aaaa` (la caducidad se formatea **por partes de la cadena**, sin `Date`; el timestamp de descontinuación sí se convierte a hora local), ID con relleno de ceros (`#00001`) y días restantes.
- **Normalización y búsqueda:** el frontend **no** tiene su propia copia de la normalización (T-05, D-04): toda búsqueda y la vista previa del precio las resuelve el backend; la vista previa deja el campo en blanco, sin error, mientras el valor esté incompleto o inválido, y muestra el mensaje al salir del campo o al guardar.
- **Campos numéricos:** `<input type="text" inputmode="decimal">` (T-07); el texto se envía tal cual y el cliente solo puede mostrar ayudas.
- **Salida segura (A-01):** todo dato que viene del usuario o de la base (nombres, descripciones, abreviaciones) se pinta con `textContent`, `createElement` y `replaceChildren`; **las vistas y componentes nuevos no usan `innerHTML`, `outerHTML` ni `insertAdjacentHTML` con datos** (ni siquiera con un escapado propio). Lo protegen un test estático en `environment.test.js` y, por vista, un test jsdom que guarda un nombre como `<img src=x onerror=…>` y comprueba que se ve como texto y no crea ningún elemento. **No se activa CSP** en esta feature (§10): el proyecto es local, offline y de un solo usuario, y activarla exigiría migrar los manejadores y estilos inline de todo el HTML heredado (incluidas secciones sin spec), lo que rompería la aplicación. La defensa de esta feature es la salida segura. Detalle técnico: el HTML actual tiene unos 168 `onclick` y 195 `style=` inline; Tauri añade hashes/nonces a la CSP de los scripts y estilos que procesa, con lo que `'unsafe-inline'` queda sin efecto y los manejadores inline dejarían de funcionar en la versión compilada (en `tauri:dev` no se nota); y con `'unsafe-inline'` una CSP no frenaría un `onerror` inyectado. El endurecimiento completo es PEN-12 (baja prioridad; se reevaluará si el proyecto cambia de contexto).
- **Pausa, orden de respuestas y doble envío (M-02):** `utils/requestGuard.js` ofrece tres funciones pequeñas: `debounce(fn, 300)` (búsquedas a medida que se escribe, vista previa del precio y Conversor), `latestOnly(fn)` (numera cada consulta y **descarta** la respuesta de una consulta ya superada por otra más reciente) y `withBusy(button, fn)` (deshabilita el botón mientras dura la llamada y lo rehabilita al terminar, también si falla). `withBusy` envuelve Guardar, "Agregar nueva …", el botón de confirmar de `confirmDialog` y las acciones Activar/Descontinuar/Eliminar. Justificado en §9.
- **Conexión con el dashboard:** los botones "Ver Agotados" y "Ver Descontinuados" los gestiona `dashboardView.js`, que importa de `productosView.js` una función que abre la pestaña P-13 con el filtro ya aplicado (D-07); la navegación usa las funciones globales de `dashboard.js` (`showPage`, `showProductoTab`) a través de `window`.
- **Código simulado de facturas:** `productosData`, `productosDisponibles` y las funciones de facturas que los consumen **se eliminan** junto con el resto del mock de esta feature. La sección de facturas del dashboard queda **oculta** hasta que se implemente su spec, igual que Historial y Facturas Asociadas. **Cómo se oculta:** el `<li>` del sidebar se localiza con `a[data-page="facturas"]` (el atributo vive en el `<a>`, no en el `<li>`; verificar el valor real del `data-page` contra `dashboard-unificado.html` antes de implementar) y se oculta con `.closest('li').hidden = true`; la página del dashboard (`#page-facturas` o el contenedor equivalente que se identifique con `grep`) también se oculta con `hidden`. Las funciones de facturas que leían `productosData`/`productosDisponibles` se eliminan, de modo que no queda código roto accesible. No hay adaptador, no hay caché de solo lectura y no hay conversión a dólares en el frontend. Cuando se implemente la spec de facturas, se conectará directamente al servicio de productos (`productsService.getActiveProducts()`, que esa spec definirá). Esto elimina el acoplamiento entre `dashboard.js` (script clásico) y los módulos ES nuevos, y reduce drásticamente la superficie de XSS pendiente. El paginador global no requiere llamada de refresco: se engancha con un `MutationObserver` sobre el `<tbody>` (verificado en `dashboard.js`).
- **Sin CSP:** no se activa `app.security.csp`. El proyecto es local, offline y de un solo usuario; activar CSP exigiría migrar los manejadores y estilos inline de todo el HTML heredado, incluidas secciones sin spec. La defensa de esta feature es la salida segura (`textContent` en código nuevo). PEN-12 queda como pendiente de baja prioridad.
- **Aviso de purga fallida:** los servicios exponen `expiredCleanupWarning` de las respuestas que lo traen (§4). Cada vista que lo recibe lo muestra como texto discreto (`textContent`, `role="status"`, en su zona de feedback existente, sin romper ni ocultar el listado) y lo limpia cuando llega `null`.
- **Paginación de P-13 — confirmado:** la tabla de la pestaña Agotados y Descontinuados se añade a `PAGINABLES` (con el id de su `<tbody>`), igual que Productos.

---

## 6. Estructura de carpetas (ampliación)

**Backend (`src-tauri/src/`)** — se añade a lo existente (`commands/auth.rs`, `commands/users.rs`, `db/schema.rs`, `db/connection.rs`, `models/user.rs`, `models/session.rs`, `validators.rs`, `errors.rs`, `main.rs`):

```
src-tauri/src/
  commands/
    categories.rs      // CAT-01..06 (+ eliminación con N-09)
    units.rs           // UM-01..05 (+ eliminación con N-08)
    conversions.rs     // UM-06, UM-07, camino único y Conversor (§3.5)
    products.rs        // P-01..P-09, P-12..P-14, vista previa de precio, purge_expired_products (§3.6)
    dashboard.rs       // DASH-01
  db/
    schema.rs          // se amplía: tablas, índices y CHECK de §2
    notifications.rs   // insert_notification(conn, type, priority, message, now) y el armado de la lista de nombres (NT-01)
  models/
    category.rs  unit.rs  conversion.rs  product.rs   // DTOs de §4
  text.rs              // clean_name, clean_description, normalize_text, spanish_sort_key, sort_spanish, is_format_char
  timestamps.rs        // format_utc_timestamp, parse_utc_timestamp
  pricing.rs           // selling_price_cents, weighted_average_cost_cents (P-10, sin consumidor)
  validators.rs        // se amplía: validate_name/description, parse_scaled y los parse_* de §3.2, validate_expiry_date
  errors.rs            // se amplía: código invalidState, map_db_error con tabla de duplicados
  lib.rs               // declara los módulos nuevos y registra los comandos en generate_handler!
```

**Frontend (`js/`)** — se añade a `session.js`, `utils/formValidation.js`, `views/loginView.js`, `recoveryView.js`, `perfil.js`, `usuarios.js`, `services/invoke.js`, `authService.js`, `usersService.js`:

```
js/
  views/      categoriasView.js  unidadesView.js  productosView.js  dashboardView.js
  components/ combobox.js  confirmDialog.js
  services/   categoriesService.js  unitsService.js  productsService.js  dashboardService.js
  utils/      formatters.js  requestGuard.js
  __tests__/  (un archivo por módulo nuevo, §8)
```

No hay `adaptadorFacturas.js` ni archivo equivalente: la sección de facturas queda oculta hasta su spec (§5).

---

## 7. Adaptación de interfaz existente

Los cambios de interfaz salen de la revisión completa de `dashboard-unificado.html` y `dashboard.js` (PEN-03, resuelta). **Criterio rector: la spec manda**; cuando el HTML/JS actual la contradice, se cambia el frontend. Cada fila indica la tarea de `tasks.md` donde se implementa (`T §n`).

### 7.1 Cambios transversales (T-01 a T-10)

| ID | Cambio | Spec | Tarea |
|---|---|---|---|
| T-01 | **Capa de acceso a datos.** Cada listado, alta, edición y baja llama a un comando Tauri; el frontend solo conserva la última respuesta como caché de vista y recarga tras cada mutación. Se eliminan los datos de ejemplo y los contadores `next…Id`. Toda vista tiene estados de carga, vacío y error; la base empieza vacía. | §2, §8, 7.8 | T §2.6, §3.5, §6.9, §6.10, §8 |
| T-02 | **Referencias por id.** Selectores y combobox manejan `{id, nombre}`; nunca se envía ni compara un nombre como clave. | 7.7 | T §6.9 |
| T-03 | **Errores.** Se muestra el mensaje específico del backend en pantalla (no `alert()`); los internos, el genérico. `invoke.js` convierte el rechazo de argumentos de Tauri en "Los datos enviados no son válidos" y cualquier otro error sin forma `AppError` en "Ocurrió un error inesperado"; los `AppError` se devuelven intactos. | §8 | T §1 bis |
| T-04 | **Diálogo de confirmación reutilizable** (`.modal-overlay` + `.modal-card`), con texto multilínea y conteos; reemplaza cada `confirm()` (eliminar categoría, unidad, conversión y producto; descontinuar). | CAT-04, UM-04, UM-06, P-04, P-14 | T §1 bis |
| T-05 | **Normalización de búsqueda única** (la de 7.1, ñ distinta de n), hecha por el backend; el cliente no tiene copia. | 7.1, 7.2 | T §1.1, §2.1 |
| T-06 | **Combobox con creación sobre la marcha** (Categoría y Unidad): coincidencias por subcadena (Unidad: nombre y abreviación), "Agregar nueva …" solo sin coincidencia exacta, error si se guarda texto sin elegir opción, botón "Sin categoría"/"Sin unidad" en edición. Reemplaza `<select>` + "+" en `.autocomplete-row`. | CAT-05, UM-05 | T §6.9 |
| T-07 | **Números y dinero.** Todo campo numérico es `<input type="text" inputmode="decimal">` (no `type="number"`); coma o punto, sin separador de miles; el texto se envía tal cual y el backend valida y convierte con enteros. El cliente solo muestra ayudas ("usa coma para los decimales", aviso de `1.234`); la regla autoritativa es la de 7.4. Mostrar: centavos ÷ 100 con formato `es-ES` y sufijo USD. | 7.4 | T §4.3, §6.9 |
| T-08 | **UTF-8.** Corregir las cadenas dañadas (p. ej. "Categorña", "Lí­neas" con guion blando) en cada función que se toque. | — | T §2.6, §3.5, §4.3, §6.x |
| T-09 | **Paginación.** Se mantiene el paginador global (DOM, `MutationObserver` sobre el `<tbody>`); la tabla de P-13 se añade a `PAGINABLES`. | P-09 | T §6.10 |
| T-10 | **Código simulado de facturas.** `productosData`, `productosDisponibles` y las funciones de facturas que los consumen se eliminan; la sección de facturas del dashboard queda oculta (`hidden`) hasta su spec. Sin adaptador y sin caché. | §3.2 | T §6.10 |
| T-11 | **Pausa, respuestas viejas y doble envío** (M-02): `debounce` de 300 ms en búsquedas, vista previa del precio y Conversor; `latestOnly` descarta respuestas superadas; `withBusy` deshabilita los botones de guardar/confirmar durante la llamada. | 9 | T §1 bis, §2.6, §3.5, §4.3, §6.9, §6.10 |
| T-12 | **Salida segura** (A-01): datos del usuario siempre como texto (`textContent`/DOM), sin `innerHTML` con datos en vistas y componentes nuevos. | 9 | T §1 bis, §2.6, §3.5, §4.3, §6.9, §6.10, §7.1, §8 |
| T-13 | **Regla global `[hidden]`** (M-01): `[hidden] { display: none !important; }` al inicio de `dashboard.css`, para que el atributo `hidden` oculte siempre aunque otra regla fije `display`. | `Constitution.md` §2.V (§7.7) | T §1 bis |

### 7.2 Productos

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-P-01 | Pestañas `.view-tabs` | Añadir **Agotados y Descontinuados** (`#tab-producto-agotados`, vista `#producto-agotados`, rama `'agotados'` de `showProductoTab`). Orden: Productos, Agotados y Descontinuados, Crear Nuevo Producto, Historial (oculto). | P-13, S-1 | T §6.10 |
| FE-P-02 | `renderProductos` | Solo **activos** (incluye agotados marcados), en el orden español del backend. | P-09 | T §6.10 |
| FE-P-03 | Umbral de stock | Stock bajo = `0 < cantidad < 5`; agotado = `0` con su propio badge; mismo umbral para badge y filtro (hoy `stock <= 5`). | P-09, P-12 | T §6.7, §6.10 |
| FE-P-04 | Columnas | Añadir **Acciones** (ver, editar, descontinuar, eliminar; patrón `.col-acciones`); **ocultar Valor Total** (D-02); "Sin categoría" / "—" si falta; ID con ceros (`#00001`, solo presentación). | P-09, P-14 | T §6.10 |
| FE-P-05 | `#producto-search` | Buscar solo por **nombre** (T-05); placeholder "Buscar producto…". | P-12, 7.2 | T §6.10 |
| FE-P-06 | Filtros (bajo el toolbar; `.historial-toolbar` y `.field`) | Categoría (una o "Sin categoría"), Stock (Todos / Disponibles / Agotados / Stock bajo), Caducidad (Todos / Con caducidad —incluye vencidos— / Sin caducidad / Vencidos / Próximos a vencer: hoy a hoy + 29). Se combinan entre sí y con la búsqueda. | P-12 | T §6.7, §6.10 |
| FE-P-07 | Vista Agotados y Descontinuados | Columnas Nombre, Categoría, Unidad, Cantidad, **Condición** (Agotado / Descontinuado / ambas; una sola fila por producto), **Días restantes** (solo descontinuados, hacia arriba, 0–60), Acciones. Orden español. Descontinuado: **Activar** y **Eliminar**; agotado activo: las acciones del listado. Filtro de condición y búsqueda por nombre. La purga la ejecuta el comando. | P-05, P-13, P-08 | T §6.8, §6.10 |
| FE-P-10 | `#producto-precio-venta` | **Solo lectura**, vista previa calculada por el backend (`previewSellingPrice`): al editar envía solo el % (texto) y el `productId` y el costo lo lee el backend de la BD; al crear, el % y el costo escrito (texto); nunca se envía el precio de venta. Mientras sea incompleto o inválido queda vacío/"—" **sin error**; el mensaje aparece al salir del campo o al guardar. | P-01, P-11 | T §6.1, §6.9 |
| FE-P-11 | `#producto-ganancia` | Campo de texto **editable y obligatorio** (`inputmode="decimal"`), 0 a 1000,00, hasta 2 decimales (hoy es solo lectura y admite negativos). | P-01, P-11, S-6 | T §6.9 |
| FE-P-12 | `#producto-precio-entrada` | Creación: obligatorio, > 0, hasta 999.999,99 y 2 decimales. Edición: solo lectura. | P-01, P-03, 7.4 | T §6.9 |
| FE-P-13 | `#producto-stock` | Creación: campo de **cantidad inicial obligatoria** (texto con `inputmode="decimal"`, > 0, hasta 3 decimales; hoy es `type="number"` con `placeholder="0"`). Edición: solo lectura como texto, hasta 3 decimales sin ceros finales, y no se envía. | P-01, P-03, 7.4, 7.6 | T §6.9 |
| FE-P-14 | `#producto-categoria`, `#producto-unidad` | **Opcionales** (quitar `required` y asterisco); reemplazar `<select>` por el combobox T-06. | 5, P-01, CAT-05, UM-05 | T §6.9 |
| FE-P-15 | `irCrearCategoria()`, `agregarUnidadRapida()` | Se eliminan. Creación dentro del combobox sin salir del formulario; unidad: `#modal-unidad` con nombre precargado **editable** y abreviación **vacía, sin sugerencia**; cancelar no crea nada; lo creado existe aunque se cancele el producto (S-7). | CAT-05, UM-05 | T §6.9 |
| FE-P-16 | Caducidad (`-tipo` + `-fecha`) | Se mantiene el patrón; fecha con `min` = hoy y `max` = `2100-12-31` solo como ayuda; el backend rechaza fecha pasada, `2026-02-30` y año > 2100; en edición, una fecha vencida no modificada se acepta; se muestra `dd/mm/aaaa`. | 7.5, P-03 | T §6.9 |
| FE-P-17 | Largos | `maxlength` nombre 2–100 y descripción hasta 500, solo como ayuda: decide el backend (puntos de código Unicode). | 7.3 | T §6.9 |
| FE-P-18 | Duplicado de nombre | Mostrar el mensaje del backend. | 7.1, P-01 | T §6.9 |
| FE-P-19 | Sección Facturas Asociadas del formulario | **Ocultar** (`hidden`) hasta la spec de Facturas. | §3.2, PEN-09 | T §6.9 |
| FE-P-20 | Edición de descontinuados | No se ofrece "Editar"; si el backend rechaza igualmente (datos desactualizados), mostrar su mensaje. | P-03 | T §6.10 |
| FE-P-30 | Detalle: precio, % de ganancia | Mostrar los valores del backend; ganancia con 2 decimales; **eliminar el tono naranja por ganancia negativa** (ya no existe). | P-02, P-11 | T §6.10 |
| FE-P-31 | Etiqueta de stock del detalle | "Stock Bajo" solo si `0 < cantidad < 5`; "Agotado" si `cantidad = 0`. | P-12 | T §6.10 |
| FE-P-32 | Eliminar / Descontinuar | Mantener "Eliminar" (P-14) con confirmación que advierte la **pérdida de stock** si cantidad > 0, vía comando; **añadir "Descontinuar"** (P-04) con su confirmación (también advierte). Ambos en el detalle de un activo. Si el backend responde "ya está descontinuado", mostrar el mensaje y recargar. | P-04, P-14 | T §6.10 |
| FE-P-33 | Producto descontinuado | Etiqueta, fecha de descontinuación, días restantes, **Activar** y **Eliminar**; ocultar Editar y Descontinuar; mostrar el rechazo por nombre activo duplicado, "El producto ya no existe" si venció y, ante "ya está activo", el mensaje y recarga. | P-06, P-07, P-14 | T §6.10 |
| FE-P-34 | `<details>` Facturas Asociadas y botón Ver Historial | **Ocultar** hasta las specs de Historial y Facturas. | §3.2, PEN-09 | T §6.10 |

La pestaña Historial de producto (`#producto-historial`) queda oculta por completo (D-01).

### 7.3 Categorías

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-C-01 | `#categoria-descripcion` | Opcional. Largos: nombre 1–50, descripción hasta 200. | CAT-01, 7.3 | T §2.6 |
| FE-C-02 | Bloque "Listado de Productos \*" (`#categoria-productos-picker`, `agregarProductoPicker`, `quitarProductoPicker`, `renderProductPicker`) | **Eliminar** (era obligatorio y su `onchange` no guardaba nada); la categoría se asigna desde el producto. | CAT-01, P-01 | T §2.6 |
| FE-C-03 | Nombre al crear | Mostrar coincidencias como **aviso de duplicado**; si hay exacta, indicar que ya existe. Sin botón "agregar": Guardar es la única acción. | CAT-01 | T §2.6 |
| FE-C-04 | Listado | **Ocultar Ingresos** (D-02); conteo de productos del backend (incluye descontinuados); orden español; búsqueda con T-05. | CAT-02 | T §2.6 |
| FE-C-05 | Eliminar | Confirmación con cuántos productos quedarán sin categoría (del último listado); el resultado usa el número real devuelto. | CAT-04, S-10 | T §2.5, §2.6 |
| FE-C-06 | Detalle `#categoria-ver` | **Ocultar Ingresos**; mostrar nombre, descripción, cantidad y lista de productos del backend, por id. | CAT-06 | T §2.4, §2.6 |
| FE-C-07 | Edición | Sin cambio funcional; mostrar el mensaje de duplicado del backend. | CAT-03 | T §2.6 |

### 7.4 Unidades y conversiones

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-U-01 | `#modal-unidad-abreviacion` | `maxlength=10`; nombre `30` (hoy 8). | 7.3 | T §3.5 |
| FE-U-02 | `guardarUnidad` | Mostrar el mensaje del backend que indica **cuál** dato (nombre o abreviación) está duplicado; eliminar el renombrado de productos por texto. | UM-01, UM-03 | T §3.5 |
| FE-U-03 | `eliminarUnidad` | Confirmación con **dos consecuencias separadas** (productos sin unidad; conversiones que se borrarán), del último listado; el resultado usa los números reales. | UM-04, S-10 | T §3.4, §3.5 |
| FE-U-04 | Columna "Productos con Unidad" | Conteo del backend (incluye descontinuados). `conversionCount` viene en el listado pero **no es columna**: solo alimenta la confirmación. | UM-02 | T §3.2, §3.5 |
| FE-U-05 | `#unidad-search` | Mantener, con T-05; busca unidades por nombre y abreviación. | 7.2 | T §3.5 |
| FE-U-06 | Conversor Rápido (`.conversor-card`) | Se **conserva** el componente (D-05); el cálculo lo hace `convertQuantity`; se eliminan el cálculo y el BFS del cliente. Cantidad como texto decimal (hasta 6 decimales); el resultado llega como texto y se muestra tal cual (sin `Number`/`parseFloat`). Sin camino o resultado demasiado grande: mensaje del backend. | UM-06, UM-07, PEN-06 | T §4.2, §4.3 |
| FE-U-07 | Nombre en el modal de alta | Mostrar unidades existentes que coincidan como aviso de duplicado. | UM-01 | T §3.5 |
| FE-V-01 | Edición de conversión | **Origen y destino deshabilitados**; solo el factor. | UM-06 | T §4.3 |
| FE-V-02 | `#modal-conversion-factor` | Texto con `inputmode="decimal"` (no `type="number"`); 0,000001 a 999.999.999,999999; hasta 6 decimales. | S-4, 7.4 | T §4.3 |
| FE-V-03 | Texto de ayuda del modal | Reemplazar "podrá encadenar…" por la explicación del camino único (no A↔C si ya existe A↔B↔C). | UM-06 | T §4.3 |
| FE-V-04 | `guardarConversion` | El backend decide (misma unidad, duplicado, segundo camino); se muestra su mensaje. `#modal-conversion-preview` se mantiene solo como comodidad. | UM-06 | T §4.3 |
| FE-V-05 | `eliminarConversion` | Confirmación T-04. | UM-06 | T §4.3 |
| FE-V-06 | `formatearNumeroUnidad` | Hasta 6 decimales, sin ceros finales (hoy 4). | S-4, UM-07 | T §4.3 |

La página de Unidades conserva dos tablas (unidades y conversiones) en la misma página, sin pestañas nuevas.

### 7.5 Dashboard y notificaciones

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-D-01 | Tarjeta Productos Agotados | Datos reales de un **único comando** (`getDashboardSummary`): total de agotados activos, hasta 3 categorías y sin categoría; vacío = 0. | DASH-01 | T §7.1 |
| FE-D-02 | "Ver Agotados" / "Ver Descontinuados" | Abrir la pestaña P-13 con el filtro de condición ya aplicado (hoy: dos `onclick` iguales sin filtro). | DASH-01, P-13 | T §7.1 |
| FE-D-03 | KPIs `inventario` y `agotados`, Mejor Vendidos, Lento Movimiento | **Estado vacío**, sin cifras ni listas inventadas, hasta Facturas de venta. | DASH-01, §2, PEN-10 | T §7.1 |
| FE-D-04 | Elementos de la spec 1 en el dashboard | **No se alteran**: `#reserved-data-warning`, los `<script type="module">` de `usuarios.js` y `perfil.js` y el `import('./session.js')`. Un test jsdom los protege. | AU-06 (spec 1) | T §7.2 |

**Notificaciones:** sin cambios en esta spec; `N-08` y `N-09` solo se guardan (NT-01) y `#page-notificaciones` no se toca (D-09).

### 7.6 Decisiones de interfaz (D-01 a D-09, todas resueltas)

| ID | Decisión | Resolución |
|---|---|---|
| D-01 | Pestaña Historial y botón "Ver Historial". | **Ocultar** hasta la spec de Historial (PEN-09). |
| D-02 | Columnas Valor Total e Ingresos. | **Ocultas y sin datos** hasta Facturas de venta; nunca valores inventados (PEN-10). |
| D-03 | Sección Facturas Asociadas. | **Ocultar** hasta las specs de facturas (PEN-09). |
| D-04 | Dónde se ejecuta la búsqueda y el cálculo del precio en vivo. | **Backend**: la vista previa es un comando y la búsqueda usa la normalización de 7.1. |
| D-05 | Conversor Rápido. | **Se conserva**, con comando de backend (UM-07). |
| D-06 | Quién dispara la purga. | **Todo comando** que liste, cuente o toque un descontinuado; no hay purga al iniciar (P-08). |
| D-07 | Botones del dashboard hacia P-13. | Abrir con el filtro aplicado. |
| D-08 | Alcance de datos reales del dashboard. | Solo la tarjeta de agotados; el resto, estado vacío. |
| D-09 | Página de Notificaciones. | No tocarla ni mostrar N-08/N-09 (PEN-01). |

### 7.7 Decisiones propias de este plan

- **Origen de los datos:** desaparecen `categoriasData`, `unidadesData`, `conversionesData`, `INVENTARIO_EJEMPLO`, `productosData`, `productosDisponibles`, los contadores `next…Id` y sus funciones de render, cálculo y validación del cliente; todo pasa a las vistas de §5 y a los comandos de §4. La sección de facturas del dashboard queda **oculta** (`hidden`) hasta su spec. El resto de `dashboard.js` queda como orquestador (navegación, sidebar, logout, utilidades): sigue cargado como script clásico, sin `type="module"`, y los módulos nuevos se cargan con `<script type="module">`. **Orden de borrado (A-02):** cada sección elimina **solo sus funciones** y llamadas; los arreglos globales `categoriasData`, `unidadesData` y `conversionesData` (con sus contadores `next…Id` y `editing…Id`) se borran **juntos en una única tarea al final de T §6.10**, porque se leen fuera de su sección (el Conversor lee `unidadesData`; el formulario de producto lee `categoriasData` y `unidadesData`; el `grep` es la fuente de verdad, sin números de línea que se desplazan). Hasta esa tarea los arreglos existen pero ningún código de las secciones ya migradas los usa; el formulario de producto viejo los sigue leyendo **de forma transitoria** hasta T §6.9.
- **Manejadores inline:** los `onclick`/`oninput`/`onchange` de las secciones migradas dejan de funcionar con módulos ES y se reemplazan por listeners registrados desde las vistas (misma regla que `plan-01` §6). El inventario exacto se levanta en `tasks.md` §0 contra `dashboard-unificado.html`.
- **Elementos ocultos, no eliminados** (D-01, D-02, D-03): pestaña Historial, botón "Ver Historial", sección Facturas Asociadas, **sección de facturas del dashboard** (el `<li>` del sidebar con `a[data-page="facturas"]` y la página `#page-facturas`, ambos con `hidden`), columnas Valor Total e Ingresos, y KPIs, "Mejor Vendidos" y "Lento Movimiento" en estado vacío. Se ocultan con el atributo `hidden` para que las specs futuras (PEN-04, PEN-05, PEN-09, PEN-10) los reactiven sin reconstruirlos.
- **Verificación real del sidebar:** se comprobó con `Select-String` sobre [src/dashboard-unificado.html](src/dashboard-unificado.html) y no existe ningún enlace con `data-page="facturas"`; el archivo real del proyecto no contiene esa ruta en el sidebar. La ocultación de la sección de facturas se mantiene como medida de seguridad, pero el valor exacto del selector a ocultar se confirma en el HTML real y no por suposición. El patrón de ocultación sigue siendo `a[data-page="<valor real>"]` + `.closest('li').hidden = true`, con la página `hidden` en el bloque correspondiente.
- **Estilos (`Constitution.md` §2.V):** **una única excepción** en `dashboard.css` y ninguna en `auth.css` (M-01): se añade al inicio de la hoja `[hidden] { display: none !important; }`. Justificación: el plan oculta con el atributo `hidden` la pestaña Historial, el botón Ver Historial, Facturas Asociadas, la sección de facturas, Valor Total, Ingresos y los KPIs en estado vacío, y cualquier regla CSS que fije `display` sobre ese elemento (`flex`, `table-cell`…) anula el atributo sin avisar; los tests jsdom comprueban el atributo, no lo que se dibuja. Es un componente compartido, por eso se documenta aquí (`Constitution.md` §2.V) y en §9. Las reglas existentes `.pager[hidden]` y `.dash-error[hidden]` quedan como están (redundantes, no se tocan). Se reutilizan las clases existentes (`.view-tabs`, `.historial-toolbar`, `.field`, `.col-acciones`, `.modal-overlay`, `.modal-card`, `.autocomplete-row`, `.feedback-inline`). Si el combobox o la pestaña nueva exigieran una regla que no exista, se documenta aquí **antes** de implementar.
- **Elementos de la spec 1 que no se tocan (C-09, FE-D-04):** `#reserved-data-warning`, los módulos `usuarios.js` y `perfil.js` y el `import('./session.js')` de `dashboard.js`. Un test de jsdom los protege (§8).
- **Notificaciones:** la página `#page-notificaciones` no se toca (D-09): `N-08` y `N-09` solo se guardan.

---

## 8. Estrategia de testing (test-first, `Constitution.md` §2.III)

**Backend (Rust):** tests sobre las funciones `xxx_inner` y los módulos puros, con `setup_test_db()` (en memoria, `foreign_keys = ON`) y fechas inyectadas (`now`, `today`). **Los productos agotados (cantidad 0) y los que necesitan una cantidad exacta se crean insertando filas directamente** en la base de pruebas con un helper `insert_product_fixture(...)`: ningún comando ni código de producción produce cantidad 0 ni altera la cantidad después de crear. Los fallos forzados (purga y notificación) usan **triggers de prueba** (`RAISE(ABORT)`) creados solo en el test.

| Área | Qué se prueba |
|---|---|
| Autorización | Matriz de 26 filas (una por comando de §4): sin sesión → `sessionExpired`; con Administrador y con Contador → permitido. La verificación final confirma que ningún comando de esta feature quedó fuera. |
| Esquema | Un test por cada `CHECK`, `UNIQUE` y clave foránea: nombres normalizados repetidos (`Camión`/`camion`), índice parcial (activo repetido falla, descontinuado con el mismo nombre no), par de conversiones en ambos sentidos, `ON DELETE SET NULL` y cascada de conversiones, `CHECK` de rangos, estado/fecha y caducidad, **cada uno con un valor que fallaría por `NULL`**, y formato de `discontinuedAt`. |
| Texto y orden | Tabla de 7.1 (`Camión`/`CAMIÓN`/`camion` iguales, `año` ≠ `ano`, espacios colapsados), nombres frente a descripciones (7.3), `ñame` entre `nuez` y `oso`, `Ángel` con la A, desempate por ID y **un test por cada listado** que use `sort_spanish` (incluidos conversiones, DASH-01 y la lista de NT-01). |
| Números | Coma y punto, rechazo de `1e3`, `+5`, espacios, `,5`, `5,`, exceso de decimales, `0.123`, `1.234`, `007`, el orden de mensajes de 7.4 y los topes de cada rango. |
| Fechas | Caducidad: hoy válido, ayer rechazado, hoy + 29 incluido, hoy + 30 excluido, `2026-02-30`, `2026-2-3` y año > 2100 rechazados; vencida no modificada aceptada al editar. Formato de timestamp: ida y vuelta de `format`/`parse`. |
| Precios | `selling_price_cents` (ejemplo 12,50 y 20 % → 15,00; 0 % y 1000 %); `weighted_average_cost_cents` (ejemplo 133,33 → 1,33; con cantidad 0; rechazos) y que la vista previa devuelve lo mismo que se guarda. **Costo vigente:** `previewSellingPrice` con `productId` lee el costo de la BD (si el costo cambió entre la carga y la vista previa, usa el **actual**) y con `cost` (al crear) lo valida con `parse_cost_cents`; `productId` y `cost` a la vez, o ninguno, es error de contrato; no purga ni escribe. |
| Auditoría | `createdAt` y `updatedAt` con formato incorrecto fallan (`GLOB`) en `categories`, `units`, `products` y `unitConversions`, y con el correcto pasan; `updatedAt` cambia al editar (categoría, unidad, factor, producto, descontinuar y activar) y `createdAt` no; **eliminar una categoría o unidad actualiza `updatedAt` de los productos afectados** (Q-02). |
| Timestamps (Q-05) | `2026-13-01T00:00:00Z`, `2026-01-01T25:00:00Z` y `2026-02-30T00:00:00Z` fallan en `createdAt`, `updatedAt`, `discontinuedAt` y `notifications.createdAt` aunque cumplan el `GLOB`; `parse_utc_timestamp` los devuelve como `None`; una fila corrupta hace que `remainingDays` responda `internal` y se registre. |
| Conversiones | Misma unidad, duplicado en ambos sentidos y segundo camino (A↔B, B↔C, A↔C); Conversor en ambos sentidos, de varios tramos (incluida tonelada→kilo→gramo→miligramo), sin camino, con redondeo a 6 decimales y sin ceros finales, resultado `0`, resultado > 999.999.999,999999 y desborde del cálculo **con mensajes distintos** (M-07), y misma unidad. |
| Purga | Con fixtures: 59 d 23:59:59 no se purga, 60 d exactos y 60 d + 1 s sí; los conteos de categorías/unidades y las listas de N-08/N-09 no incluyen vencidos; "El producto ya no existe" para uno vencido en cada comando de §3.6; **purga best-effort:** con un trigger `RAISE(ABORT)` en `DELETE` sobre `products`, cada comando de §3.6 **no aborta**: devuelve el listado sin los vencidos (excluidos en la consulta), incluye `expiredCleanupWarning` con el texto del aviso y registra el error en `errorLogs` (`cause` sin nombres de productos); sin vencidos (N = 0) el aviso es `null`; `expiredCleanupWarning` es `null` cuando la purga funciona; **si el `SAVEPOINT` de la purga falla**, el comando responde `internal` genérico y **no** devuelve el listado (caso no soportado, transacción irrecuperable); **arrancar la aplicación no purga productos**. |
| Eliminaciones | N-09/N-08 con fronteras de 20 y 21 nombres ("y N más"), orden español y total real; cero afectados → ninguna notificación; **atomicidad**: un trigger que falle el `INSERT` en `notifications` deja intactas la categoría o unidad y sus conversiones; el resultado informa los conteos reales; los productos afectados quedan con `updatedAt` = `now` y las filas no afectadas no cambian (Q-02). |
| Productos | Crear (con su cantidad inicial; omitida, 0, negativa, con más de 3 decimales o > 999.999,999 rechazadas), editar (unicidad excluye la propia fila, descontinuado → `invalidState`), descontinuar/activar/eliminar con sus `invalidState`, activar bloqueado por un activo homónimo, filtros de stock y caducidad combinados con la búsqueda (fronteras de 5 y de hoy + 29), `listOutOfStockAndDiscontinued` sin duplicar un producto agotado y descontinuado, y `conversionCount` de unidades (origen y destino). |
| Contrato (C-01, A-03) | El struct de `createProduct` **falla al deserializarse** con `sellingPrice` y el de `updateProduct` con `quantity`, `sellingPrice` o `cost`; con los campos válidos pasan. **Con `tauri::test`** (§1): se invoca el comando real con un campo prohibido y se comprueba el texto exacto del rechazo y que el comando no se ejecuta; y los argumentos de cada wrapper de `services/*.js` coinciden con las firmas Rust (Vitest, por expresión regular, como el test de `generate_handler!`). |
| Caracteres no permitidos (M-06) | NUL, otros controles, U+00AD, U+200B, U+200D, U+202E y U+FEFF rechazados en nombre, abreviación y descripción; solo-invisibles = "no permitido" (no "vacío"); `ñ`, acentos y U+FE0F aceptados; `\n` y `\r\n` aceptados en la descripción y reducidos a espacio en nombres. |
| Conexión (M-03) | Abierta sobre un archivo temporal: `synchronous` = 2 (FULL), `journal_mode` = `delete`, `foreign_keys` = 1. |
| Dashboard | Límite de 3 categorías, desempate alfabético, productos descontinuados excluidos y estado vacío en cero (con fixtures de cantidad 0). |
| Errores | `validationFailed` no genera fila en `errorLogs`; `duplicateField`, `notFound`, `invalidState` e `internal` sí; `cause` nunca contiene valores ingresados (se fuerza un error interno con un nombre de producto reconocible y se comprueba el log). |

**Frontend (Vitest + jsdom):** con `window.__TAURI__` sustituido por un stub **solo dentro de los tests**. Se verifica que cada vista llame al servicio correcto y use la respuesta del backend (sin datos de ejemplo), los estados de carga/vacío/error, `combobox` (opción "Agregar nueva" solo sin coincidencia exacta, botón "Sin categoría", texto sin elegir bloquea el guardado, cancelar la creación de unidad no crea nada), `confirmDialog`, `formatters` (la caducidad sin desplazamiento por zona horaria, dinero, ID con ceros), que los campos numéricos envían el texto tal cual, que el Conversor muestra el texto del backend sin pasarlo por `Number`, que `invoke.js` convierte un rechazo en texto plano en "Los datos enviados no son válidos" **sin alterar** los errores `AppError` (el test existente sigue en verde), y la protección del dashboard (C-09): un test que carga `dashboard-unificado.html` y comprueba que existe `#reserved-data-warning` y que siguen referenciados `usuarios.js` y `perfil.js`. **Además (A-01, M-02, M-01, B-04):** en cada vista nueva, un nombre con HTML se muestra como texto sin crear elementos; búsquedas, vista previa y Conversor con `vi.useFakeTimers()` (300 ms, una sola llamada por ráfaga y descarte de respuestas desordenadas); un doble clic produce una llamada, con el botón deshabilitado y rehabilitado también tras un error; `invoke.js` escribe `console.error` ante un error sin forma `AppError`; el formulario de edición envía **todos** los campos editables (la descripción precargada viaja; vaciarla es intencional); y la regla `[hidden]` de `dashboard.css` oculta de verdad un elemento cuya clase fija `display`; ante un `expiredCleanupWarning` no nulo la vista muestra el aviso de forma discreta sin romper el listado, y con `null` lo limpia.

**Teclado y foco (Q-06, Vitest + jsdom):** `combobox` — ↑/↓ cambian la opción activa (`aria-activedescendant`), Enter elige, Escape cierra sin elegir, Tab cierra y sigue, y la doble coincidencia exacta de unidades muestra ambas sin "Agregar nueva" (Q-07); `confirmDialog` — foco inicial en Cancelar, Tab y Shift+Tab ciclan dentro, Escape cancela y el foco vuelve al elemento que abrió el diálogo.

**Verificación manual (Q-04, Q-12):** (a) como ningún comando deja un producto en 0, para ver un agotado en la app real se detiene la aplicación y, **sobre la base de desarrollo**, se ejecuta `UPDATE products SET quantityMilli = 0 WHERE id = ?` con un cliente SQLite: es un paso manual de prueba, nunca código ni comando de la aplicación; (b) al cierre se repite la pasada completa en la **app compilada** (`npm run tauri:build`), no solo en `tauri:dev`, porque la carga de módulos y recursos puede comportarse distinto.

**Contrato `invoke` ↔ comandos registrados:** el test existente que compara por expresión regular `generate_handler!` de `lib.rs` con los nombres que invocan los `services/*Service.js` cubre automáticamente los servicios nuevos; el límite conocido (no detecta typos en los argumentos) se mantiene.

Ningún test se satisface con valores hardcodeados o casos artificiales (`Constitution.md` §2.III / `Agents.md` §4). Antes de dar por terminada cualquier tarea: `cargo fmt`, `cargo clippy` sin warnings, `npx prettier --write` y las dos baterías de tests en verde.

---

## 9. Complexity Tracking (desviaciones de `Constitution.md`)

| Desviación | Razón | Alternativa más simple considerada |
|---|---|---|
| Módulo `pricing.rs` con `weighted_average_cost_cents` (P-10) **sin consumidor** hasta PEN-04, frente a `Constitution.md` §2.II (nada de código sin un problema real y presente) | Decisión explícita del dueño: especificarlo e implementarlo completo desde esta spec, con sus tests, para que las facturas de entrada solo tengan que invocarlo. Es un módulo `pub` del crate, sin warnings de `clippy` | Dejarlo para la spec de Facturas de entrada — descartado por el dueño |
| El rechazo de campos no admitidos al crear/editar producto lo hace Tauri **antes** del comando: no pasa por `errorLogs` ni devuelve `validationFailed` (C-01) | Es un error de contrato inalcanzable desde la interfaz; el struct con `deny_unknown_fields` es la forma más simple de que el campo prohibido no se ignore en silencio. Recibe el mismo trato que `validationFailed`: un rechazo esperado que no es un fallo del sistema | Recibir `serde_json::Value` y deserializar a mano para devolver un `validationFailed` real — descartado por más código manual |
| Textos compuestos en el frontend, frente a "Rust es dueño de los textos" (`plan-01` §4/§5): "Los datos enviados no son válidos" de `invoke.js` (el rechazo de Tauri nunca llega como `AppError`) y el aviso "Elige una opción de la lista o vacía el campo" del combobox (T-06), que se decide antes de llamar al backend | No hay respuesta de Rust de la que tomar el texto: es la misma excepción acotada de `invoke.js` y de `formValidation.js` en la spec 1. Los mensajes de resultado de las eliminaciones ("quedaron N productos sin categoría") componen cifras que sí devuelve el backend | Enviar el texto sin elegir al backend para que lo rechace — descartado: el formulario nunca lo enviaría |
| Los comandos de **lectura** purgan productos vencidos (escriben) antes de listar o contar (P-08) | Cumple la spec sin temporizador ni proceso de fondo (`Constitution.md` §2.II); el mismo patrón ya existe en `validate_session` | Purga al arrancar o en segundo plano — descartado por la spec (P-08) |
| Regla global `[hidden] { display: none !important; }` en `dashboard.css` (componente compartido), frente a `Constitution.md` §2.V | Sin ella, `hidden` falla en silencio si otra regla fija `display`, y D-01/D-02/D-03 dependen de `hidden` (M-01) | Ajustar `display` elemento por elemento — descartado: frágil y toca más reglas compartidas |
| Feature `test` de `tauri` en `[dev-dependencies]`, frente a `Constitution.md` §2.II (nada nuevo sin justificar) | Es la única forma de probar el rechazo real de Tauri antes del comando (A-03); no añade crates ni entra en el binario | Probar solo el struct de serde — descartado: no cubre la capa de Tauri ni su mensaje |
| `utils/requestGuard.js` (tres funciones pequeñas) | Resuelve un problema presente: una consulta por pulsación, respuestas desordenadas y dobles envíos (M-02) | Repetir la lógica en cada vista — descartado: divergiría |
| La purga es **best-effort** en lecturas y comandos con `productId` (si falla, se registra, se filtran vencidos en la consulta y se avisa al usuario), frente a abortar el comando | Evita que un fallo puntual de un `DELETE` deje la aplicación inutilizable; el usuario nunca ve un vencido y el fallo queda registrado. La app es local y de un solo usuario | Abortar el comando — descartado por frágil |

No hay desviación del registro de errores: queda cubierta por `Constitution.md` v1.2.0 §2.VI (los rechazos esperados no se registran). Ninguna dependencia nueva (solo la feature `test` de `tauri` en desarrollo, arriba). La convención de nombres, la unicidad con columnas normalizadas y las reglas en el esquema cumplen `Constitution.md` §3 y §4 sin requerir fila de desviación.

---

## 10. Límites conocidos y decisiones de este plan (confirmadas)

**Límites aceptados**
- **Precio de venta máximo:** costo $999.999,99 y 1000 % dan ≈ $11.000.000. Cabe en `i64` y no lleva `CHECK` por ser derivado; se documenta aquí.
- **Productos vencidos** pueden quedar en la base más de 60 días si ningún comando los toca; nunca se cuentan, muestran ni operan (spec P-08). Si la purga falla, se filtran en la consulta y se avisa al usuario.
- **Normalización:** sin NFC/NFD. Limitación aceptada conscientemente: el proyecto es local, offline y de un solo usuario, que usa el mismo sistema operativo y teclado, por lo que el riesgo de mezclar formas es bajo en la práctica. Si el proyecto cambia de contexto, se reevaluará añadiendo `unicode-normalization` (spec 7.1).
- **Reloj del sistema:** cambiar la fecha del equipo puede adelantar o retrasar la purga; se acepta por el mismo motivo que en la spec 1. Un adelanto grande de la fecha puede purgar de golpe varios descontinuados y no hay respaldo automático: queda como PEN-11 (M-03).
- **Registro de duplicados:** `duplicateField`, `notFound` e `invalidState` se registran (como en la spec 1); con un uso intenso de nombres repetidos el log podría ganar ruido. No se cambia `should_log()` en esta feature.
- **Purga best-effort (M-04):** si la purga falla, el comando no aborta; se registra el error, se filtran vencidos en la consulta y se muestra un aviso visible. La app sigue usable; el fallo queda para diagnóstico. Reemplaza la decisión anterior (C-06) de abortar.
- **Plegado parcial de acentos (Q-08):** solo se pliegan 6 vocales; `Cafè` y `Cafe` son distintos y `ç`, `â`, `ï`… ordenan después de la `z` (spec 7.1). Ampliarlo es un cambio local en `text.rs`.
- **Unicidad por columna en unidades (Q-07):** una unidad puede llamarse como la abreviación de otra; el buscador muestra ambas (spec 7.2). No se añade unicidad cruzada nombre↔abreviación por no tener un problema real y presente (§2.II).
- **Agotados no observables en la app real (Q-04):** con cantidad inicial > 0 y sin ventas, esos estados solo se ven con fixtures o con el `UPDATE` manual de §8.
- **Sin CSP (A-01):** `app.security.csp` sigue en `null`. El proyecto es local, offline y de un solo usuario; activar CSP exigiría migrar los manejadores y estilos inline de todo el HTML heredado, incluidas secciones sin spec. La defensa de esta feature es la salida segura (§5, T-12). PEN-12 queda como pendiente de baja prioridad y se reevaluará si el proyecto cambia de contexto.
- **Eliminaciones sin registro de autor (M-03):** ambos roles pueden eliminar y no se guarda quién borró qué (PEN-13). Se añaden `createdAt`/`updatedAt` a las tablas nuevas, pero no autoría.
- **Cantidad inicial no editable (confirmado):** una cantidad escrita mal solo se corrige eliminando el producto y creándolo de nuevo, igual que el costo.
- **Facturas simuladas ocultas:** la sección de facturas del dashboard queda `hidden` hasta su spec. No hay adaptador ni caché; cuando se implemente, se conectará a `productsService`.

**Decisiones que este plan tomó y la spec no fijaba (todas confirmadas por el dueño; las 9 a 18 son de la revisión posterior a la auditoría; las 19 a 26 son de la segunda revisión QA y el dueño las delegó en el criterio del auditor)**
1. El precio de venta **se calcula al leer**, no se guarda (§2).
2. Los decimales viajan como **texto con coma** y los precios como enteros en centavos (§3.2).
3. `updateProduct` es de **reemplazo completo** y `createProduct`/`updateProduct` reciben un único argumento `input` (§4).
4. La purga se ejecuta en **todo comando que recibe un `productId`**, no solo en los que la spec enumera (§3.6).
5. El Conversor reduce por **máximo común divisor** para evitar desbordes falsos en cadenas de unidades reales (§3.5).
6. Orden de validación de `createConversion` y **textos exactos** del catálogo de §4 (los que la spec solo describía).
7. El Conversor sin camino entre dos unidades responde `validationFailed` y no `notFound`, para que un resultado esperado no se registre en el log (§3.5).
8. Los filtros de `listProducts` viajan como un struct `filters` con `categoryId` y `withoutCategory` separados (§4).

9. **Cantidad inicial obligatoria y mayor que 0 al crear**, no editable después (§2, §4). Cambia S-9: el costo y la cantidad iniciales pesan en el promedio ponderado. Consecuencia visible: en el uso real, la pestaña Agotados y la tarjeta del dashboard quedan vacías hasta que existan ventas.
10. **No se activa CSP** y la sección de facturas simuladas se oculta hasta su spec (§5); sin adaptador. **Registra PEN-12** (endurecimiento del frontend heredado, baja prioridad).
11. **`PRAGMA journal_mode = DELETE` y `synchronous = FULL`** explícitos (§2). **Pendientes derivados de M-03 (independientes del PRAGMA):** **PEN-11** (respaldo de la base de datos) y **PEN-13** (registro de autor de eliminaciones).
12. Mensaje propio para el desborde del cálculo del Conversor (M-07) y rechazo de caracteres de control y de formato en los textos (M-06, §3.1).
13. **Purga best-effort con aviso visible** si falla (§3.6, §4).
14. **Columnas de auditoría** `createdAt`/`updatedAt` en `categories`, `units`, `products` y `unitConversions` (§2).
15. **Límite de dígitos por campo** en `parse_scaled` (8/9/15/15/6), sin número mágico (§3.2).
16. **El costo vigente lo lee el backend de la BD** en `previewSellingPrice` al editar (el frontend envía solo `%` e `id`); al crear, que aún no tiene producto, envía el costo escrito (§3.4, §4).
17. **NFC/NFD sin dependencia**; limitación aceptada y documentada (§3.1).
18. **Rediseño de notificaciones tipo toast** registrado como PEN-14 (spec §10); N-08 y N-09 se guardan como texto plano.
19. **Eliminar una categoría o unidad actualiza `updatedAt`** de los productos afectados con un `UPDATE` explícito dentro de la transacción (§3.7, Q-02).
20. **Timestamps validados también con `strftime` ida y vuelta** en el esquema; `None` al leer = fila corrupta → `internal` (§2, §3.3, Q-05).
21. **`invoke.js` distingue** el rechazo de argumentos de Tauri ("Los datos enviados no son válidos") de cualquier otro error sin forma `AppError` ("Ocurrió un error inesperado") (§4, §5, Q-09).
22. **Aviso de purga sin cifra** si falla también el conteo de vencidos (§3.6, Q-10).
23. **Teclado y foco:** combobox con roles ARIA y flechas/Enter/Escape; `confirmDialog` con trampa de foco, Cancelar como foco inicial y retorno del foco (§5, Q-06).
24. **Unidades con doble coincidencia exacta:** se muestran ambas, sin "Agregar nueva" y sin unicidad cruzada nombre↔abreviación (spec 7.2, Q-07).
25. **Plegado parcial de acentos** como limitación aceptada junto a NFC/NFD (spec 7.1, Q-08).
26. **Verificación manual:** agotados con `UPDATE` sobre la BD de desarrollo y pasada final en la app compilada (§8, Q-04, Q-12).

---

### Trazabilidad de las decisiones de auditoría (C-01 a C-10)

Las etiquetas `C-xx` que aparecen en este plan y en `tasks.md` identifican decisiones de la auditoría de la spec, todas confirmadas por el dueño. Dónde vive cada una:

| ID | Decisión | Sección |
|---|---|---|
| C-01 | Campos no admitidos: struct `deny_unknown_fields` con un único `input`; rechazo de Tauri convertido por `invoke.js`. | §4 (contrato de entrada), §5, §9 |
| C-02 | Orden español en Rust con clave de orden; ninguna consulta ordena por nombre. | §3.1 |
| C-03 | Búsqueda con `instr()` sobre columnas normalizadas. | §3.1 |
| C-04 | "Hoy" local inyectado (`today`) y verificación de `chrono::Local`. | §1, §3.3 |
| C-05 | Enteros escalados con `i128` verificado; redondeo único en el Conversor. | §2, §3.2, §3.5 |
| C-06 | Purga única, en una transacción con la lectura, sin índice. | §3.6 |
| C-07 | Eliminación de categoría/unidad en una transacción con la notificación. | §3.7 |
| C-08 | Catálogo de errores: `invalidState`, textos exactos y deducción de duplicados. | §4 |
| C-09 | Protección de los elementos de la spec 1 en el dashboard. | §7.5, §8 |
| C-10 | Fecha de descontinuación con formato fijo y helper único. | §2, §3.3 |

### Trazabilidad de los hallazgos de la auditoría (A-01 a B-05)

| ID | Hallazgo | Resolución | Dónde |
|---|---|---|---|
| A-01 | XSS almacenado | Salida segura con `textContent` en código nuevo; sin CSP (motivos en §5); sin adaptador: el código simulado de facturas se elimina y su sección se oculta; PEN-12 (baja prioridad) | §5, §7.1 T-10 y T-12, §10 |
| A-02 | Orden de tareas rompe secciones | Cada sección borra solo funciones; arreglos globales en una tarea final | §7.7, `tasks.md` §2.6, §3.5, §4.3, §6.10 |
| A-03 | Supuestos de Tauri sin verificar | Spike y tests con `tauri::test`; `console.error` en `invoke.js`; test de argumentos de wrappers | §1, §4, §5, §8 |
| M-01 | `hidden` puede no ocultar | Regla global `[hidden]` documentada como excepción de §2.V | §7.7, T-13, §9 |
| M-02 | Carreras y doble envío | `requestGuard.js` (300 ms, descarte, `withBusy`) | §5, T-11, §9 |
| M-03 | Pérdida de datos | PRAGMA explícitos; PEN-11 (respaldo) y PEN-13 (autor) | §2, §10 |
| M-04 | Purga como punto único de fallo | Purga best-effort con aviso visible (`expiredCleanupWarning`) | §3.6, §4, §10 |
| M-05 | Cantidad fija en 0 | Cantidad inicial obligatoria > 0 | spec P-01, 5, 7.4; §2, §4 |
| M-06 | Normalización admite invisibles | Rechazo de Cc y Cf; `\n` permitido en descripción | §3.1, §4 |
| M-07 | Desborde del Conversor engañoso | Mensaje propio | §3.5, §4 |
| M-08 | Tareas demasiado grandes | Sub-casillas por regla | `tasks.md` (encabezado y secciones) |
| B-01 | Adaptador frágil | Sin objeto: se elimina el adaptador y el código simulado de facturas | §5 |
| B-02 | Esquema en desarrollo | Aviso de borrar la BD al cambiar un `CHECK` | §2, `tasks.md` §0 |
| B-03 | Notificaciones sin límite | Nota de retención en PEN-01 | spec §10 |
| B-04 | `updateProduct` de reemplazo completo | Test: la edición envía todos los campos editables | §8, `tasks.md` §6.9 |
| B-05 | Filtros sin `serde(default)` | `#[serde(default)]` en los opcionales | §4, `tasks.md` §6.7 |

### Trazabilidad de la segunda revisión QA (Q-01 a Q-12)

| ID | Hallazgo | Resolución | Dónde |
|---|---|---|---|
| Q-01 | Faltaba fixture de conversiones antes de `createConversion` | `insert_conversion_fixture` en la §1.6 de las tareas | `tasks.md` §1.6 |
| Q-02 | `updatedAt` desfasado al eliminar categoría o unidad | `UPDATE` explícito en la transacción | §2, §3.7, §8 |
| Q-03 | Tareas de test demasiado densas | Sub-casillas por grupo de reglas | `tasks.md` (encabezado, §1.3, §1.5, §4.1, §6.1, §6.7, §6.9, §6.10) |
| Q-04 | Agotados invisibles en la app real | `UPDATE` manual sobre la BD de desarrollo | §8, §10, `tasks.md` §7.1 |
| Q-05 | `GLOB` acepta fechas imposibles | `strftime` ida y vuelta; regla para `None` | §2, §3.3, §8 |
| Q-06 | Sin cobertura de teclado ni foco | ARIA y teclado en combobox; trampa de foco en diálogos | §5, §8, spec §9 |
| Q-07 | Doble coincidencia exacta en unidades | Se muestran ambas, sin "Agregar nueva" | §5, §10, spec 7.2 |
| Q-08 | Plegado parcial de acentos | Límite documentado | §10, spec 7.1 |
| Q-09 | `invoke.js` mezclaba todo error en un texto | Clasificación: rechazo de argumentos vs. otro | §4, §5, §7.1 T-03 |
| Q-10 | El conteo del aviso puede fallar | Aviso sin cifra | §3.6, spec P-08 |
| Q-11 | Redacción ambigua ("en memoria", `today`) | Aclarado en la consulta; `today` lo calcula el envoltorio | §3.6, §4 |
| Q-12 | Sin prueba en app compilada | Pasada final con `tauri:build` | §8, `tasks.md` §9 |

## 11. Siguiente paso

Plan confirmado. `tasks.md` (ya redactado) ordena las tareas empezando por las verificaciones previas de §1; el inventario de `dashboard.js`/`dashboard-unificado.html` ya está levantado en `tasks.md` §0. Se sigue el flujo test-first de `Agents.md` §4.
