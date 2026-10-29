# Plan: Contactos, Facturación e Historial de Productos

- **Feature:** `contactos-facturacion-historial`
- **Basado en:** `spec-03.md` (esta misma carpeta), `Constitution.md` v1.2.0, `Agents.md`, `spec-01` (autenticación y usuarios), `spec-02` (inventario y productos), `pendientes.md`, y el frontend real `dashboard-unificado.html` + `dashboard.js`.
- **Estado:** **Confirmado por el dueño. Listo para implementar.** Incorpora todas las decisiones del dueño (A1–A2, B1–B12, C1–C12 de la ronda de clarificación del frontend real) y las 27 decisiones D-01 a D-27 de §10. Todas las correcciones de la auditoría QA están aplicadas. **Revisión QA posterior:** aplicados los arreglos H-1 a H-11 y O-1 a O-4 (ver §10.4). **Segunda revisión QA:** aplicados N-1 a N-4 y O-B (ver §10.5). **Alineación con `spec-03.md` final:** corregido el conteo de comandos en §4.1, confirmada la decisión sobre `updatedAt` en §3.8, y documentada la decisión sobre `listHistoryProducts` (C-7). **Auditoría de inventario:** aplicados H-A1 a H-A4 (ver §10.6): helper `reverse_lots_of_invoice`, tope de stock con `validate_stock_limit` y `stockLimitExceeded`, política de registro con throttle (H-A2) y regla de Ajuste solo por cambio de cantidad (H-A4).
- **Nota:** es el **tercer módulo con backend Rust y SQLite** y el primero con inventario transaccional. Reutiliza sin cambios la infraestructura de `plan-01` y `plan-02` (conexión compartida, `AppError`, `require_session`, `setup_test_db`, patrón `xxx_inner` con envoltorio delgado, `purge_best_effort`, `map_db_error`, `validate_*`/`parse_*`, `sort_spanish`) y solo la **amplía**. Nada de lo implementado en las specs 1 y 2 se reabre, salvo los cambios que la propia `spec-03` ordena sobre `spec-02` (§12 bis de la spec): crear producto con lote inicial, retirada de `weighted_average_cost_cents` y las columnas "Valor Total" e "Ingresos".

---

## 1. Dependencias y configuración

**Dependencias nuevas: tres** (`Constitution.md` §2.II). Las versiones y *features* exactas se fijan en `tasks-03.md` §0. Todas son Rust síncrono, sin `tokio`.

| Crate | Uso | Justificación |
|---|---|---|
| ~~`rust_xlsxwriter`~~ → `zip` 9 (sin features) | Exportar el historial a `.xlsx` (HP-03) | **Cambio (2026-10-16, decisión del dueño):** todas las versiones publicadas de `rust_xlsxwriter` dependen de `zip` 2.x–8.x, retiradas (*yanked*) de crates.io, y no se pueden instalar. Se usa `zip` 9 sin *features* (escritura sin compresión) y `export.rs` escribe el SpreadsheetML mínimo (6 partes XML). `zip` 9 exige Rust 1.88, así que `rust-version` pasa de 1.77.2 a 1.88 (el toolchain instalado es 1.98). Alternativa descartada: CSV con extensión `.xlsx` (Excel lo abre con aviso y pierde formato). |
| `printpdf` 0.7 (sin features por defecto) | Exportar el historial a `.pdf` (HP-03) | Rust puro. Sin imágenes ni HTML, con la fuente integrada Helvetica en `WinAnsiEncoding` (sin incrustar fuentes; ver D-18 y §3.10). La serie 0.12 se descartó: su *feature* por defecto trae un motor de maquetación HTML (`azul`) y exige Rust 1.88. Exigida por la spec. |
| `uuid` (feature `v4`) | Nombre del PDF adjunto en disco (FE-07) | Ya es dependencia transitiva de `tauri`; se declara directa para usarla. Alternativa: generar 16 bytes aleatorios con `getrandom` y formatearlos a mano — más código propio para el mismo resultado. |

| Necesidad | Solución | Por qué no una dependencia |
|---|---|---|
| Validar el correo (§7.1) | Función propia `is_valid_email` (~15 líneas, equivalente al patrón de la spec) | `regex` sobra para un único patrón simple (mismo criterio de `plan-02` §1). |
| Aritmética de líneas, totales y costos | Enteros escalados: `i64` en SQLite, `i128` al calcular (§3.1) | `rust_decimal` es innecesario: los rangos caben en `i128`. |
| Relevancia de búsqueda (§7.1) | Función propia en `search.rs` (§3.5) | `fuzzy-matcher` y similares dan coincidencia aproximada, que la spec no pide (exacto > prefijo > subcadena). |
| Pruebas pseudoaleatorias de invariantes (§8) | Generador xorshift de 10 líneas dentro del módulo de tests | `proptest` es una dependencia de desarrollo que no se justifica por una sola batería. |
| Throttle del registro (H-A2) | `HashMap<(String, i64), DateTime<Utc>>` bajo `Mutex` en `errors.rs` | No hay concurrencia real (una sola sesión); persistirlo exigiría columnas nuevas en `errorLogs` sin valor de diagnóstico. |
| Diálogo nativo "Guardar como" y "Abrir" | `tauri-plugin-dialog` (**ya presente**, `plan-01` §1) usado desde el frontend vía `window.__TAURI__.dialog` | `tauri-plugin-fs` sería una dependencia nueva: el backend lee y copia los archivos, el frontend solo recibe la ruta (spec §5.5). |

**Verificaciones previas** — cada una tiene su tarea espejo en `tasks-03.md` §0. Se ejecutan **antes** de empezar la implementación.

- [ ] Fijar versiones y *features* de `rust_xlsxwriter`, `printpdf` y `uuid`; comprobar que ninguna arrastra `tokio` ni un motor de imágenes pesado y que `cargo clippy` y `cargo build` siguen sin warnings.
- [ ] Confirmar que `dialog:default` en `capabilities` incluye `save` y `open` y que `window.__TAURI__.dialog` existe con `withGlobalTauri: true`. Si no, añadir solo `dialog:allow-save` y `dialog:allow-open`.
- [ ] **Spike de triggers (D-17):** comprobar que (a) `BEFORE UPDATE OF …` sobre `productMovements` **no** se dispara cuando una FK `ON DELETE SET NULL` anula `productId`, `invoiceId` o `userId`; (b) `BEFORE DELETE` bloquea un `DELETE` directo; (c) borrar un producto, una factura y un usuario con movimientos funciona. Anotar el texto exacto de `RAISE(ABORT)`.
- [ ] Anotar las firmas reales de lo que este plan reutiliza: `Actor`, `round_half_up`, `purge_best_effort`, `map_db_error`, `validate_name`, `validate_description`, `parse_cost_cents`, `parse_quantity_milli`, `format_milli`, `insert_product_fixture`, `assert_requires_session`, `assert_allows_both_roles`. Si `round_half_up` o `format_milli` son privados, pasan a `pub(crate)` sin cambiar su comportamiento.
- [ ] Confirmar la conversión `chrono::Local` ↔ UTC en un día de cambio de hora (hueco y repetición) con `earliest()`/`latest()`.
- [ ] Verificar contra `dashboard-unificado.html` y `dashboard.js` **reales** el inventario de ids y funciones de la spec §9.X (el `grep` es la fuente de verdad). Anotar el `id` o `data-page` **real** del enlace del submenú de facturas en el sidebar (para desocultarlo en la fase correspondiente, §7.10).
- [ ] Comprobar con `printpdf` que Helvetica integrada pinta `á é í ó ú ü ñ ¿ ¡ €` en un PDF de prueba (D-18).
- [ ] **Borrar la base de desarrollo** antes de la primera ejecución.
- [ ] **Reconocimiento de `innerHTML` (O-2):** contar con `grep -c 'innerHTML'` sobre `dashboard.js` las asignaciones con datos del usuario por vista (facturas, clientes, proveedores, historial). Anotar el número por vista; alimenta la estimación de esfuerzo de `tasks-03.md` §4.5.7.

**Concurrencia y transacciones:** igual que `plan-01` §1 y `plan-02` §1: una conexión compartida (`Mutex<Connection>`, `lock_conn()`), todas las funciones internas `xxx_inner` reciben `conn: &mut Connection`, y la transacción garantiza solo el rollback (el `Mutex` ya serializa los comandos). **Cada operación de §3.2 es una sola transacción** (spec 7.6). Lo único que no es transaccional son los archivos PDF en disco (§3.8), que se ordenan para que un fallo nunca deje una referencia a un archivo inexistente.

**Testing:** `#[cfg(test)]` nativo en Rust y Vitest + jsdom en el frontend, como en las specs 1 y 2.

---

## 2. Esquema de base de datos

`init_schema(conn, now)` (`db/schema.rs`) se **amplía** con tablas nuevas y no toca las de las specs 1 y 2: `products` **no cambia de definición** (conserva `quantityMilli` y `costCents` como cachés, §3.2). Todo usa `CREATE … IF NOT EXISTS`, por lo que sigue siendo idempotente; las advertencias de `plan-02` §2 sobre borrar la base de desarrollo al cambiar un `CHECK` aplican igual. Nombres de tablas y columnas en `camelCase`. `PRAGMA foreign_keys = ON` ya está activo: `SET NULL` y `CASCADE` dependen de él.

`ts_check(col)` y `ts_check_opt(col)` son dos funciones de `schema.rs` que generan el `CHECK` de timestamp de `plan-02` §2 (`GLOB` de forma + `strftime(...) IS col` de ida y vuelta; la segunda admite `NULL`). Así la expresión larga no se copia 20 veces. En el SQL de abajo se escriben como `TS(col)` y `TS?(col)`.

```sql
-- ===== Contactos (CL-*, PV-*) =====
CREATE TABLE IF NOT EXISTS clients (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  firstName TEXT NOT NULL CHECK (length(firstName) BETWEEN 2 AND 50),
  lastName TEXT NOT NULL CHECK (length(lastName) BETWEEN 2 AND 50),
  nationalId TEXT NOT NULL UNIQUE                                   -- CI único dentro de su tabla (CL-01)
    CHECK (nationalId GLOB '[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
  phone TEXT NOT NULL                                               -- NO es único (spec 5.1)
    CHECK (phone GLOB '+53[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
  company TEXT CHECK (company IS NULL OR length(company) BETWEEN 1 AND 100),
  position TEXT CHECK (position IS NULL OR length(position) BETWEEN 1 AND 100),
  email TEXT CHECK (email IS NULL OR length(email) BETWEEN 3 AND 100),  -- el patrón lo valida el código (§3.1)
  address TEXT CHECK (address IS NULL OR length(address) BETWEEN 1 AND 200),
  invoiceInfo TEXT CHECK (invoiceInfo IS NULL OR length(invoiceInfo) BETWEEN 1 AND 200), -- solo clientes
  createdAt TEXT NOT NULL CHECK TS(createdAt),                      -- "fecha de alta" (spec 5.1)
  updatedAt TEXT NOT NULL CHECK TS(updatedAt)
);
-- suppliers: idéntica a clients, sin invoiceInfo.

-- ===== Numeración (7.4) =====
CREATE TABLE IF NOT EXISTS invoiceCounters (
  type TEXT PRIMARY KEY CHECK (type IN ('E','V','S','GG','GI','P')),
  nextNumber INTEGER NOT NULL CHECK (nextNumber >= 1)
);
INSERT OR IGNORE INTO invoiceCounters (type, nextNumber)
  VALUES ('E',1),('V',1),('S',1),('GG',1),('GI',1),('P',1);          -- estructura, no datos de ejemplo (7.8)

-- ===== Facturas: una tabla con discriminador (spec 5.3) =====
CREATE TABLE IF NOT EXISTS invoices (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL CHECK (type IN ('E','V','S','GG','GI','P')),
  number TEXT NOT NULL UNIQUE CHECK (substr(number, 1, length(type) + 1) = type || '-'),
  invoiceDate TEXT NOT NULL                                         -- día de calendario elegido por el usuario (7.3)
    CHECK (invoiceDate GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
       AND date(invoiceDate) IS invoiceDate AND invoiceDate >= '2000-01-01'),   -- "no futura" lo valida el código
  status TEXT CHECK (status IN ('pending','completed','cancelled')),            -- NULL solo en pérdidas (CHECK de tabla)
  notes TEXT CHECK (notes IS NULL OR length(notes) BETWEEN 1 AND 500),          -- observaciones / descripción (D-02)
  paymentMethod TEXT CHECK (paymentMethod IN ('cash','transfer','card','zelle')),
  paidAt TEXT CHECK TS?(paidAt),                                    -- fecha y hora UTC de pago o cobro
  clientId INTEGER REFERENCES clients(id) ON DELETE SET NULL,
  supplierId INTEGER REFERENCES suppliers(id) ON DELETE SET NULL,
  contactFullName TEXT CHECK (contactFullName IS NULL OR length(contactFullName) BETWEEN 5 AND 101), -- copia (7.7)
  contactFullNameNormalized TEXT,
  contactCompany TEXT,
  contactCompanyNormalized TEXT,
  contactNationalId TEXT,
  contactPhone TEXT,
  contactEmail TEXT,
  associatedInvoiceId INTEGER REFERENCES invoices(id) ON DELETE SET NULL,       -- garantía e insumos
  associatedNumber TEXT,                                            -- copia del número
  associatedType TEXT CHECK (associatedType IN ('V','S')),          -- copia del tipo
  attachmentFileName TEXT,                                          -- UUID.pdf en app_data_dir/attachments/ (FE-07)
  attachmentOriginalName TEXT CHECK (attachmentOriginalName IS NULL OR length(attachmentOriginalName) BETWEEN 1 AND 100),
  attachmentSizeBytes INTEGER,
  attachmentUploadedAt TEXT CHECK TS?(attachmentUploadedAt),
  createdByUserId INTEGER REFERENCES users(id) ON DELETE SET NULL,
  createdByName TEXT NOT NULL CHECK (length(createdByName) BETWEEN 1 AND 50),   -- copia del username (7.7)
  createdAt TEXT NOT NULL CHECK TS(createdAt),
  updatedAt TEXT NOT NULL CHECK TS(updatedAt),
  -- Estado: obligatorio salvo en pérdidas. `status IS NOT NULL AND …` porque un NULL dejaría pasar el CHECK.
  CHECK ((type = 'P' AND status IS NULL)
      OR (type <> 'P' AND status IS NOT NULL AND status IN ('pending','completed','cancelled'))),
  -- Pago/cobro: solo E, V, S; obligatorio al estar completada; se conserva si se cancela desde completada (D-07).
  CHECK (paidAt IS NULL OR (type IN ('E','V','S') AND status IN ('completed','cancelled'))),
  CHECK (NOT (type IN ('E','V','S') AND status = 'completed' AND paidAt IS NULL)),
  CHECK (paymentMethod IS NULL OR paidAt IS NOT NULL),
  -- Contacto: toda factura salvo la pérdida guarda la copia; el vínculo depende del tipo.
  CHECK ((type = 'P' AND contactFullName IS NULL)
      OR (type <> 'P' AND contactFullName IS NOT NULL AND contactFullNameNormalized IS NOT NULL
          AND contactNationalId IS NOT NULL AND contactPhone IS NOT NULL)),
  CHECK (clientId IS NULL OR type IN ('V','S','GG','GI')),
  CHECK (supplierId IS NULL OR type = 'E'),
  -- Factura asociada: obligatoria en GG (venta o servicio) y GI (servicio); prohibida en el resto.
  CHECK ((type IN ('GG','GI') AND associatedNumber IS NOT NULL AND associatedType IS NOT NULL
          AND (type = 'GG' OR associatedType = 'S'))
      OR (type NOT IN ('GG','GI') AND associatedInvoiceId IS NULL AND associatedNumber IS NULL AND associatedType IS NULL)),
  -- Adjunto: los cuatro campos juntos o ninguno, solo en entradas, hasta 5 MB.
  CHECK ((attachmentFileName IS NULL AND attachmentOriginalName IS NULL
          AND attachmentSizeBytes IS NULL AND attachmentUploadedAt IS NULL)
      OR (type = 'E' AND attachmentFileName IS NOT NULL AND attachmentOriginalName IS NOT NULL
          AND attachmentSizeBytes IS NOT NULL AND attachmentSizeBytes BETWEEN 1 AND 5242880
          AND attachmentUploadedAt IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS invoiceLines (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  invoiceId INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 100),
  productId INTEGER REFERENCES products(id) ON DELETE SET NULL,
  productIdOriginal INTEGER NOT NULL,                               -- copia del ID (nunca se reutiliza)
  productName TEXT NOT NULL CHECK (length(productName) BETWEEN 2 AND 100),
  productNameNormalized TEXT NOT NULL CHECK (length(productNameNormalized) >= 1),
  unitId INTEGER REFERENCES units(id) ON DELETE SET NULL,
  unitName TEXT,
  unitAbbreviation TEXT,
  quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 1 AND 999999999),
  unitPriceCents INTEGER NOT NULL CHECK (unitPriceCents BETWEEN 0 AND 9999999999),   -- se guarda; no se recalcula al leer (5.3)
  subtotalCents INTEGER NOT NULL CHECK (subtotalCents BETWEEN 0 AND 99999999999),    -- se guarda; total = SUM (D-03)
  categoryId INTEGER,                                               -- snapshot SIN clave foránea (solo V y S, 5.3)
  categoryName TEXT,
  costAtSaleCents INTEGER CHECK (costAtSaleCents IS NULL OR costAtSaleCents BETWEEN 1 AND 99999999), -- solo V y S
  createdAt TEXT NOT NULL CHECK TS(createdAt),
  updatedAt TEXT NOT NULL CHECK TS(updatedAt),
  UNIQUE (invoiceId, position),
  UNIQUE (invoiceId, productIdOriginal),                            -- "un mismo producto no se repite" en el esquema
  CHECK ((unitName IS NULL) = (unitAbbreviation IS NULL)),
  CHECK (categoryId IS NULL OR categoryName IS NOT NULL)
);

-- ===== Inventario por lotes (LT-*) =====
CREATE TABLE IF NOT EXISTS lots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  productId INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  origin TEXT NOT NULL CHECK (origin IN ('initial','entry','restitution')),
  invoiceLineId INTEGER UNIQUE REFERENCES invoiceLines(id) ON DELETE CASCADE,   -- eliminar la compra elimina sus lotes
  arrivalOrder INTEGER NOT NULL,                                    -- orden FIFO por producto (§3.2)
  initialMilli INTEGER NOT NULL CHECK (initialMilli BETWEEN 1 AND 999999999),
  remainingMilli INTEGER NOT NULL CHECK (remainingMilli BETWEEN 0 AND initialMilli),
  unitCostCents INTEGER NOT NULL CHECK (unitCostCents BETWEEN 1 AND 99999999),
  createdAt TEXT NOT NULL CHECK TS(createdAt),
  updatedAt TEXT NOT NULL CHECK TS(updatedAt),
  UNIQUE (productId, arrivalOrder),
  CHECK ((origin = 'entry' AND invoiceLineId IS NOT NULL) OR (origin <> 'entry' AND invoiceLineId IS NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS lotsOneInitial ON lots (productId) WHERE origin = 'initial';

CREATE TABLE IF NOT EXISTS consumptions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  invoiceLineId INTEGER NOT NULL REFERENCES invoiceLines(id) ON DELETE CASCADE,
  lotId INTEGER REFERENCES lots(id) ON DELETE SET NULL,             -- vacío si el lote se eliminó (7.7)
  quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 1 AND 999999999),
  unitCostCents INTEGER NOT NULL CHECK (unitCostCents BETWEEN 1 AND 99999999),  -- del momento del consumo: sobrevive al lote
  createdAt TEXT NOT NULL CHECK TS(createdAt)                       -- solo se inserta y se borra: sin updatedAt
);

-- ===== Historial (HP-01): inmutable =====
CREATE TABLE IF NOT EXISTS productMovements (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  createdAt TEXT NOT NULL CHECK TS(createdAt),                      -- inmutable: sin updatedAt
  productId INTEGER REFERENCES products(id) ON DELETE SET NULL,
  productIdOriginal INTEGER NOT NULL,
  productName TEXT NOT NULL CHECK (length(productName) BETWEEN 2 AND 100),
  type TEXT NOT NULL CHECK (type IN ('initial','entry','saleOut','serviceOut','warrantyOut',
                                     'supplyOut','loss','adjustment','reversal')),
  quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN -999999999 AND 999999999),   -- con signo
  stockAfterMilli INTEGER NOT NULL CHECK (stockAfterMilli BETWEEN 0 AND 999999999),
  unitPriceCents INTEGER NOT NULL CHECK (unitPriceCents BETWEEN 0 AND 9999999999),
  totalPriceCents INTEGER NOT NULL CHECK (totalPriceCents BETWEEN 0 AND 99999999999),
  invoiceId INTEGER REFERENCES invoices(id) ON DELETE SET NULL,
  invoiceNumber TEXT,                                               -- copia; NULL solo en 'initial'
  invoiceType TEXT CHECK (invoiceType IN ('E','V','S','GG','GI','P')),
  userId INTEGER REFERENCES users(id) ON DELETE SET NULL,
  userName TEXT NOT NULL CHECK (length(userName) BETWEEN 1 AND 50),
  CHECK ((type = 'initial' AND invoiceNumber IS NULL AND invoiceType IS NULL)
      OR (type <> 'initial' AND invoiceNumber IS NOT NULL AND invoiceType IS NOT NULL))
);

-- ===== Índices: solo en tablas de crecimiento ilimitado y para las consultas de §4 (D-16) =====
CREATE INDEX IF NOT EXISTS invoicesTypeDate ON invoices (type, invoiceDate);
CREATE INDEX IF NOT EXISTS invoicesClient ON invoices (clientId);
CREATE INDEX IF NOT EXISTS invoicesSupplier ON invoices (supplierId);
CREATE INDEX IF NOT EXISTS invoiceLinesProduct ON invoiceLines (productId);
CREATE INDEX IF NOT EXISTS consumptionsLot ON consumptions (lotId);
CREATE INDEX IF NOT EXISTS consumptionsLine ON consumptions (invoiceLineId);
CREATE INDEX IF NOT EXISTS productMovementsProduct ON productMovements (productIdOriginal, createdAt);

-- ===== Reglas que el esquema sí puede garantizar (Constitution §4) =====
CREATE TRIGGER IF NOT EXISTS invoiceCountersNoDecrease BEFORE UPDATE OF nextNumber ON invoiceCounters
  WHEN NEW.nextNumber < OLD.nextNumber
  BEGIN SELECT RAISE(ABORT, 'invoiceCounters: nextNumber no puede decrecer'); END;
CREATE TRIGGER IF NOT EXISTS invoiceCountersNoDelete BEFORE DELETE ON invoiceCounters
  BEGIN SELECT RAISE(ABORT, 'invoiceCounters: no se puede borrar'); END;
-- Los únicos UPDATE permitidos son los de las claves foráneas ON DELETE SET NULL (productId, invoiceId, userId).
CREATE TRIGGER IF NOT EXISTS productMovementsImmutable
  BEFORE UPDATE OF createdAt, productIdOriginal, productName, type, quantityMilli, stockAfterMilli,
                   unitPriceCents, totalPriceCents, invoiceNumber, invoiceType, userName
  ON productMovements
  BEGIN SELECT RAISE(ABORT, 'productMovements: el historial es inmutable'); END;
CREATE TRIGGER IF NOT EXISTS productMovementsNoDelete BEFORE DELETE ON productMovements
  BEGIN SELECT RAISE(ABORT, 'productMovements: el historial es inmutable'); END;
```

**Notas de diseño (resumen; el detalle vive en la spec):**

- **Estados como códigos neutrales (D-01).** `pending`, `completed`, `cancelled` valen para los seis tipos; la etiqueta visible (Por Pagar / Pagada, Por Cobrar / Cobrada, Pendiente / Completada) depende del tipo y la pone el frontend.
- **Un solo texto libre por factura, `notes` (D-02).** La etiqueta es "Observaciones" en entradas y "Descripción" en el resto.
- **El total no se guarda; el subtotal de la línea sí (D-03).** El total es `SUM(subtotalCents)` de sus líneas.
- **`CHECK` que un `NULL` dejaría pasar:** las ramas de los `CHECK` de tabla exigen `IS NOT NULL` antes de comparar. Cada uno tiene su test con un valor que fallaría por `NULL` (§8).
- **Unicidad y reglas en el esquema:** `UNIQUE(invoiceId, productIdOriginal)` impide repetir un producto; `UNIQUE(productId, arrivalOrder)` fija un orden FIFO sin empates; `lotsOneInitial` impide dos lotes iniciales; `lots.remainingMilli BETWEEN 0 AND initialMilli` hace imposible un stock negativo.
- **Claves foráneas (7.7):** contacto, producto, unidad, factura asociada, usuario y lote usan `ON DELETE SET NULL`; lote → producto y lote → línea de compra usan `CASCADE`; líneas → factura y consumos → línea usan `CASCADE`. `categoryId`/`categoryName` de la línea **no** son FK a propósito.
- **`ON DELETE CASCADE` en `lots.invoiceLineId`:** borra los lotes cuando se elimina la línea de compra. **Con H-A1**, el comando `deleteInvoice` ajusta el inventario antes del `DELETE` cuando la factura está Cancelada y algún lote tiene `remainingMilli > 0`, para que la cascada no borre stock restaurado.
- **Orden FIFO (`arrivalOrder`).** Cada lote nuevo toma `MAX(arrivalOrder) + 1`; un lote de restitución toma `MIN(arrivalOrder) − 1`. El consumo ordena por `(arrivalOrder, id)`.
- **Auditoría.** Todas las tablas nuevas llevan `createdAt` y `updatedAt` con el formato fijo de `plan-02` §3.3, **salvo** `consumptions` (solo `createdAt`), `productMovements` (solo `createdAt`) e `invoiceCounters` (técnica). **Regla general:** `updatedAt` se actualiza en toda escritura posterior de la fila. Aplica a `update*Invoice`, `changeInvoiceStatus` y `saveInvoiceAttachment` (O-B, §3.8). Cada comando lo verifica con un test dedicado (N-1, `tasks-03.md` §3.1.3, §3.2.2, §3.2.3 y §4.2).
- **Tope de stock (H-A3).** No es un `CHECK`: la suma cruza filas. Se valida en código con `validate_stock_limit` (§3.2) y código `stockLimitExceeded`.
- **Inmutabilidad y contadores con *triggers* (D-17).** La spec exige que el historial sea inmutable y que `nextNumber` nunca decrezca.
- **`products` no cambia.** `quantityMilli` y `costCents` siguen siendo `NOT NULL`; se convierten en cachés (§3.2).
- **Base inicial vacía (7.8):** `init_schema` solo inserta las seis filas estructurales de `invoiceCounters`; ningún cliente, proveedor, factura, lote ni movimiento de ejemplo.
- **Arranque.** La secuencia de `plan-01` §3 bis añade el **barrido de adjuntos huérfanos** (§3.8).

---

## 3. Reglas transversales en código

Todo vive en módulos pequeños reutilizados por los comandos, para que una regla no quede divergente entre comandos (`Constitution.md` §2.II), igual que `validators.rs` en la spec 1 y `text.rs` en la spec 2.

### 3.1 Texto, correo y validaciones (`text.rs`, `validators.rs`)

- Se **reutilizan** `clean_name`, `clean_description`, `normalize_text`, `spanish_sort_key`, `sort_spanish`, `validate_name`, `validate_description` de `spec-02`. No se duplican.
- `is_valid_email(raw) -> bool`: patrón `^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$` en ASCII, hasta 100 caracteres. Se aplica a `email` de contactos. Es la única función nueva en `text.rs`.
- **CI y teléfono:** `validate_national_id` (11 dígitos) y `validate_phone` (`+53` + 8 dígitos) se reutilizan sin cambios de `spec-01`.
- **Caracteres no permitidos (M-06):** se reutiliza la lógica de `spec-02` §3.1 (rechazo de Cc y Cf) para `firstName`, `lastName`, `company`, `position`, `address`, `invoiceInfo`, `notes`. El campo `email` se valida con `is_valid_email` (el rechazo de Cc/Cf está implícito en el patrón ASCII).
- **Envoltorios por campo** en `validators.rs`: `validate_client_input`, `validate_supplier_input`, `validate_invoice_notes`, `validate_attachment_original_name`. Cada uno devuelve el valor limpio y su versión normalizada cuando aplica (como `validate_name`).

### 3.2 Inventario por lotes (`inventory.rs`, nuevo)

Todas las operaciones sobre lotes y sus efectos derivados viven en un único módulo `inventory.rs`, para que la sincronización de las cachés no quede duplicada por comando. Las funciones toman `conn: &mut Connection` (el comando ya tiene el `Mutex` tomado y la transacción abierta).

- `consume_fifo(conn, product_id, quantity_milli, invoice_line_id, now) -> Result<Vec<Consumption>, AppError>`:
  1. Carga los lotes del producto con `remainingMilli > 0`, ordenados por `(arrivalOrder, id)`.
  2. Consume cada uno hasta agotarlo o hasta completar la cantidad.
  3. Inserta una fila por cada consumo en `consumptions` con el `unitCostCents` **del lote en ese momento**.
  4. Si la cantidad total de lotes es menor que `quantity_milli`, devuelve `AppError` con código `stockInsuficiente` y mensaje "No hay stock suficiente de «X» (disponible: N)". Se revierte la transacción.
  5. Devuelve la lista de consumos para que el llamador calcule el costo de la línea.
- `line_total_cost_cents(consumptions: &[Consumption]) -> i64`: suma `quantityMilli × unitCostCents` de cada consumo y **redondea a centavos una sola vez** (mitad hacia arriba, `round_half_up`). Aritmética en `i128`.
- `line_unit_cost_cents(line_total_cost_cents: i64, line_quantity_milli: i64) -> i64`: `round_half_up(line_total / (line_quantity_milli / 1000))` en aritmética entera. **Es la única función** que produce el `unitPriceCents` de una línea de garantía/insumo/pérdida y el `unitPriceCents` del movimiento (HP-01).
- `recalculate_product_cost(conn, product_id) -> Result<(), AppError>`: recalcula `products.costCents` como promedio ponderado de los lotes con existencia (redondeo único, mitad hacia arriba). Si no queda existencia, conserva el último costo. **Siempre se llama dentro de la misma transacción** que la operación que cambió lotes. Actualiza también `updatedAt`.
- `recalculate_product_quantity(conn, product_id) -> Result<(), AppError>`: recalcula `products.quantityMilli` como `SUM(lotes.remainingMilli)`. Se llama junto con `recalculate_product_cost`. Actualiza `updatedAt`.
- `restore_consumption(conn, consumption_id, now) -> Result<Restoration, AppError>`: devuelve una cantidad consumida a su lote original. Si el lote ya no existe, crea un **lote de restitución** con `origin = 'restitution'`, `unitCostCents = consumption.unitCostCents`, `arrivalOrder = MIN(arrivalOrder) − 1`, y reutiliza uno existente con el mismo costo y `remainingMilli > 0` si lo hay. Antes de restaurar, llama a `validate_stock_limit(conn, product_id, quantity)` para defensa en profundidad.
- `next_arrival_order(conn, product_id) -> Result<i64, AppError>`: `MAX(arrivalOrder) + 1` para un lote nuevo de origen entrada o inicial.
- `next_restitution_order(conn, product_id) -> Result<i64, AppError>`: `MIN(arrivalOrder) − 1`.
- **`validate_stock_limit(conn, product_id, adding_milli) -> Result<(), AppError>` (H-A3):** calcula `SUM(lotes.remainingMilli) + adding_milli` para el producto y, si supera `999_999_999`, devuelve `AppError` con código `stockLimitExceeded` y mensaje "El stock total de «X» no puede superar 999.999,999" (`X` = nombre del producto). Se llama **antes** de crear un lote, aumentar una cantidad o restaurar consumos; el cálculo y la operación van en la misma transacción. No se puede expresar como `CHECK` porque la suma cruza filas; es una regla de código.
- **`reverse_lots_of_invoice(conn, invoice_id, now) -> Result<(), AppError>` (H-A1):** quita las unidades que quedan de cada lote de la factura (`remainingMilli > 0`), recalcula el costo del producto y registra una Reversión por línea. Lo usan los tres caminos: eliminar una entrada no cancelada, cancelar una entrada, y **eliminar una entrada Cancelada con lotes restaurados (H-A1)**. Centralizar evita que el caso Cancelada+restos quede con una rama paralela. Cuando un producto ya no existe, se salta el ajuste de esa línea y se avisa.

**Invariante de caché (test obligatorio en §8):** tras cualquier operación sobre lotes, `products.quantityMilli = SUM(lotes.remainingMilli)` y `products.costCents = promedio_ponderado(lotes con existencia)`. No se puede expresar como `CHECK`; se prueba con un test dedicado que recorre cada comando. **Con H-A1**, el invariante se cumple también tras eliminar una compra Cancelada con lotes restaurados.

### 3.3 Numeración (`invoice_counter.rs`, nuevo)

- `next_invoice_number(conn, type) -> Result<String, AppError>`: lee `invoiceCounters.nextNumber` del tipo, forma `{type}-{nextNumber:05}`, incrementa el contador en la misma transacción y devuelve el número formado. El contador **nunca se decrementa** (lo garantiza el trigger) ni se reinicia.
- Si el contador supera 99999, el formato muestra los dígitos necesarios a partir de 6 (por ejemplo `E-100000`). `format!("{}-{:05}", type, n)` cubre ambos casos sin lógica extra.
- Se llama siempre dentro de la transacción que crea la factura, **antes** del `INSERT` en `invoices`.

### 3.4 Snapshots en la factura y sus líneas (`invoice_snapshot.rs`, nuevo)

Un único módulo que arma las copias, para que un comando no pueda olvidar un campo.

- `build_contact_snapshot(contact: &Contact) -> ContactSnapshot`: extrae `fullName`, `company`, `nationalId`, `phone`, `email` del contacto. Se usa en todas las facturas no-pérdida.
- `build_line_snapshot(product: &Product, line: &LineInput, is_sale: bool) -> LineSnapshot`: extrae `productIdOriginal`, `productName`, `productNameNormalized`, `unitId`, `unitName`, `unitAbbreviation`; si `is_sale`, también `categoryId`/`categoryName` (del producto en ese momento) y `costAtSaleCents` (del `products.costCents` vigente).
- `build_movement_snapshot(product: &Product, invoice_number, invoice_type, user: &Actor, quantity_milli, stock_after_milli, unit_price_cents, total_price_cents, type) -> MovementSnapshot`: arma la fila de `productMovements`.
- **Regla:** ningún comando arma snapshots a mano; todos pasan por estos constructores. Un cambio futuro (por ejemplo, añadir un campo a la copia del contacto) se hace en un solo lugar.

### 3.5 Búsqueda por relevancia (`search.rs`, nuevo)

- `normalize_query(raw) -> String`: `normalize_text(clean_name(raw))`. Igual que la búsqueda de `spec-02`, pero se expone como función separada para que los tests la ejerciten directamente.
- `relevance_rank(field_normalized: &str, query_normalized: &str) -> Option<u8>`: devuelve `Some(0)` si el campo **es igual** a la consulta, `Some(1)` si **empieza por** alguna palabra, `Some(2)` si **contiene**, `None` si no coincide.
- `search_contacts(conn, table, query, limit) -> Result<Vec<ContactMatch>, AppError>`: para clientes o proveedores. Carga las filas, calcula el rango por campo (nombre completo, apellidos, nombre, CI, teléfono normalizado, empresa) y devuelve hasta `limit` ordenadas por `(rank, sort_spanish_key(nombre o empresa), id)`. El teléfono se normaliza quitando espacios y `+` **en ambos lados**.
- `search_products_for_invoice(conn, query, purpose, limit) -> Result<Vec<ProductMatch>, AppError>`: calcula el rango por nombre, luego categoría, luego descripción, y devuelve hasta 20. **Actualizado en task-03 §7.2 (2026-10-28):** `purpose` (`"entry"`, `"outgoing"` —por defecto— o `"loss"`) decide qué productos se ofrecen: los **activos, incluidos los agotados** marcados `isOutOfStock` (la entrada los repone; en salidas el frontend los muestra como "Agotado" sin poder elegirlos, FS-01); solo `loss` añade los **descontinuados con stock** marcados `isDiscontinued` (FP-01, S-22). Un `purpose` desconocido es `validationFailed`. Cada resultado trae `sellingPriceCents` (precio vigente) para el total preliminar de ventas y servicios.
- `search_invoices(conn, query, limit) -> Result<Vec<InvoiceListItem>, AppError>`: para la lista general. Busca por número y por la copia del contacto (nombre completo, empresa, CI, teléfono normalizado).
- **Reutilización:** `instr()` de SQLite sobre las columnas normalizadas para el filtro rápido cuando la consulta es larga; el cálculo de relevancia se hace en Rust sobre el subconjunto ya filtrado.

### 3.6 Fechas y conversión local ↔ UTC (`timestamps.rs`, ampliación)

- Se reutilizan `format_utc_timestamp` y `parse_utc_timestamp` de `plan-02` §3.3 sin cambios.
- `today_local(now: DateTime<Utc>) -> NaiveDate`: `now.with_timezone(&Local).date_naive()`. Es la fuente de "hoy" para validar fechas de factura y para los filtros.
- `parse_invoice_date(raw: &str) -> Result<NaiveDate, AppError>`: valida `YYYY-MM-DD` con `NaiveDate::parse_from_str` y rechaza `2026-02-30` (el parseo ya lo hace). Aplica la cota inferior `2000-01-01`.
- `is_future(date: NaiveDate, today: NaiveDate) -> bool`: `date > today`.
- **Cambio de hora:** en el día de cambio de hora, `Local::now()` puede ser ambiguo. Se usa `now.with_timezone(&Local).date_naive()` que resuelve a una fecha coherente. La ambigüedad de hora local nunca afecta a una **fecha de calendario** (día completo); solo afectaría a comparaciones horarias, que no se hacen sobre fechas de factura.
- **Filtros por rango de día local:** `fromDate`/`toDate` se comparan como texto contra `invoices.invoiceDate` (que ya está en `YYYY-MM-DD`); la comparación es directa y no requiere conversión.

### 3.7 Estados y transiciones (`state.rs`, nuevo)

- `enum InvoiceStatus { Pending, Completed, Cancelled }` con `from_str`/`as_str`.
- `enum InvoiceType { E, V, S, GG, GI, P }` con `from_str`/`as_str`.
- `validate_transition(type, from, to, has_payment) -> Result<(), AppError>`: implementa la tabla de §7.5 de la spec. Devuelve `invalidState` con el mensaje correspondiente. Un comando nunca decide una transición por su cuenta.
- `validate_edit_allowed(type, status) -> Result<(), AppError>`: solo se edita en `pending` (o `null` en pérdidas, que no se editan). Devuelve `invalidState` con el mensaje específico por tipo (§4.3).
- `validate_status_payload(type, status, payment_method, paid_at) -> Result<(), AppError>`: comprueba que `paymentMethod` solo se acepta con `completed`/`cancelled` y en tipos E/V/S, que `paidAt` se registra al pasar a `completed`, etc.

### 3.8 Adjuntos PDF (`attachments.rs`, nuevo)

- `validate_pdf_header(path: &Path) -> Result<(), AppError>`: abre el archivo, lee los primeros 5 bytes y comprueba que son `%PDF-`. No confía en la extensión.
- `validate_attachment_size(path: &Path) -> Result<u64, AppError>`: `metadata().len()`. Rechaza > 5 MB (5242880 bytes).
- `validate_original_name(name: &str) -> Result<(), AppError>`: rechaza nombres con `/` o `\`, longitud > 100 o < 1. Los nombres se guardan como metadato (spec §FE-07).
- `store_attachment(app_data_dir: &Path, source: &Path) -> Result<StoredAttachment, AppError>`: genera un UUID v4, copia el archivo a `app_data_dir/attachments/{uuid}.pdf`, y devuelve `{ uuid, original_name, size_bytes, uploaded_at }`. **Se llama antes de abrir la transacción del comando de factura**, para que el archivo esté en disco antes del `INSERT`.
- `delete_attachment(app_data_dir: &Path, uuid: &str) -> Result<(), AppError>`: borra `attachments/{uuid}.pdf`. Un fallo se registra pero no aborta la operación del comando (una factura no debe quedar bloqueada por un adjunto huérfano).
- `sweep_orphans(app_data_dir: &Path, conn: &Connection) -> Result<usize, AppError>`: lista `attachments/*.pdf`, lee los `attachmentFileName` referenciados en `invoices` y borra los que no están referenciados. Best-effort: si el listado o un borrado falla, se registra en `errors.log` y se continúa. Es O(n) sobre un directorio pequeño; se ejecuta al arrancar junto a `init_schema`, `clear_non_persistent_sessions` y la purga de `errorLogs`.
- **Orden transaccional del reemplazo (D-05):** al reemplazar un adjunto, primero se escribe el nuevo archivo con UUID nuevo, luego se ejecuta el `UPDATE` de `invoices` dentro de la transacción, y tras el `commit` se borra el archivo viejo. Si el `UPDATE` falla, el nuevo archivo queda huérfano y el barrido del arranque lo limpia; la factura sigue apuntando al viejo. Nunca queda una factura apuntando a un archivo inexistente.
- **`saveInvoiceAttachment` actualiza `updatedAt` (O-B / N-1):** aunque el adjunto es un metadato de la factura, escribir los cuatro campos de attachment cambia la fila y, por la regla general de `plan-02` §2 ("`updatedAt` se actualiza en toda escritura posterior"), el comando actualiza `updatedAt` con `format_utc_timestamp(now)`. La misma regla aplica a `update*Invoice` y a `changeInvoiceStatus`.

### 3.9 Purga de productos vencidos (integración con `spec-02`)

Se reutiliza `purge_best_effort(conn, now)` de `plan-02` §3.6 sin cambios. La lista de comandos de `spec-03` que la ejecutan está en `spec-03` §7.6 (lista consolidada). `require_session` se resuelve **antes** de abrir la transacción; la purga y la lectura/escritura posterior van en la misma transacción.

**Decisión de diseño sobre `listHistoryProducts` (C-7 de la spec):** `listHistoryProducts` **no purga**, aunque lista productos. Su propósito es listar los productos que tienen historial (activos y eliminados con movimientos). Si un producto vencido aún no ha sido purgado, aparece en el grupo de activos; cuando la purga se ejecute (por cualquier otro comando), pasará al grupo de eliminados y seguirá apareciendo porque sus movimientos se conservan. Purgar en este comando no aporta nada al usuario (el resultado es equivalente) y añade trabajo innecesario. **No es un hueco**, es una decisión de diseño, documentada también en `spec-03 §7.6`.

### 3.10 Exportación del historial (`export.rs`, nuevo)

- `export_history_xlsx(items: &[ProductHistoryItem], header: &HistoryHeader, destination: &Path) -> Result<(), AppError>`: arma el `.xlsx` con `zip` 9 (sin compresión) y SpreadsheetML propio (ver la tabla de dependencias); los importes son celdas numéricas con formato `#,##0.00 "USD"`. Escribe un encabezado (producto, rango de fechas, tipos filtrados, fecha de generación) y las filas. Formato de fecha `dd/mm/aaaa` y de importes `0,00 USD`.
- `export_history_pdf(items: &[ProductHistoryItem], header: &HistoryHeader, destination: &Path) -> Result<(), AppError>`: usa `printpdf` con Helvetica integrada. **Limitación aceptada (D-18):** Helvetica cubre Latin-1/WinAnsi; `á é í ó ú ü ñ ¿ ¡ €` se pintan. Si en el futuro se necesitan otros alfabetos, se incrusta una fuente TrueType (PEN-17 de `spec-03` §10).
- **Verificación previa (§1, punto 7):** spike con `printpdf` para confirmar que los caracteres se pintan bien.
- Un fallo de escritura se registra y se devuelve `AppError` con código `internal` y mensaje "No se pudo guardar el archivo". El comando nunca deja un archivo a medias: escribe a un temporal y renombra al final.

### 3.11 Registro de errores con throttle (H-A2)

Se reutiliza `log_error(conn, module, operation, cause)` de `plan-01` §1, con un **filtro de frecuencia** para los códigos nuevos con estado.

**Política de registro (H-A2):**

- **No se registran:** `validationFailed` e `invalidCredentials`. Son rechazos de formato del usuario que se repiten sin límite y no indican un fallo del sistema (`Constitution.md` §2.VI, excepción acotada).
- **Se registran con throttle de 60 s por `(code, userId)`:** `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded`. El throttle vive en un `HashMap<(String, i64), DateTime<Utc>>` bajo `Mutex` en `errors.rs`. Al superar la ventana o alcanzar un tamaño máximo (por ejemplo, 1000 entradas), la entrada se limpia. Se pierde al reiniciar la app: aceptable porque el log es para diagnóstico, no para auditoría exacta.
- **Se registran siempre, sin throttle:** `duplicateField`, `notFound`, `invalidState`, `exportFailed`, `internal`.
- **`AppError::should_log()` se amplía:** `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded` devuelven `true`; `validationFailed` e `invalidCredentials` siguen devolviendo `false`. El throttle decide si la escritura se materializa.
- Los textos exactos de los mensajes se fijan en §4.3.

**Notificaciones:** N-08 y N-09 se siguen generando desde la eliminación de categorías y unidades de `spec-02`; esta spec no toca `notifications.rs` ni las notificaciones.

---

## 4. Comandos Tauri

Convenciones de `plan-01` §4: nombre lógico `camelCase`, función Rust `snake_case` (`createEntryInvoice` → `create_entry_invoice`) y ese es el nombre que recibe `invoke()`, escrito solo dentro de los servicios. Los **argumentos** los convierte Tauri; las **respuestas** llevan `#[serde(rename_all = "camelCase")]`. `today` nunca lo envía el frontend: el envoltorio lo calcula con `chrono::Local::now().date_naive()`.

**Autorización:** los **35 comandos** exigen sesión. Cada `xxx_inner` empieza con `let _actor = require_session(conn, now)?;` antes de tocar ningún dato; ambos roles pueden ejecutarlos todos. Ninguno actualiza `lastActivityAt` (eso es solo de `touchSession()` de `spec-01`). La mutación de `users`/`sessions` sigue regida por `Agents.md` §3 y esta feature no las toca.

**Contrato de entrada (C-01):** con argumentos individuales Tauri **ignora en silencio** las claves extra. Los comandos de creación y edición de facturas reciben **un único argumento `input`**: un struct `#[serde(deny_unknown_fields, rename_all = "camelCase")]`. Los comandos de lectura reciben argumentos individuales o un `filters` con `#[serde(default)]` (B-05 de `plan-02`). El rechazo de Tauri llega al frontend como texto (no `AppError`) y no pasa por `errorLogs`; `invoke.js` lo convierte en "Los datos enviados no son válidos" y lo escribe en `console.error` con el nombre del comando.

**Orden de validación de los `input` de factura** (se informa la primera regla incumplida):

1. **Estructura:** `type` válido; `lines` con 1–100 entradas; cada línea con `productId` o `newProduct`.
2. **Texto:** `notes` (recorte → Cc/Cf → largo); nombre/apellidos/empresa/cargo/dirección de contacto si viene como `newContact`.
3. **Fechas:** `invoiceDate` no futura, ≥ 2000-01-01; `paidAt` coherente con `status`.
4. **Números:** cada línea — `quantity` (con `parse_quantity_milli`), `unitPrice`/`cost` según tipo (con `parse_cost_cents` o `parse_selling_price` de `spec-02`).
5. **Referencias:** `clientId`/`supplierId` existen; `associatedInvoiceId` es del tipo correcto y del mismo cliente y no está cancelada; `productId` existe y no está descontinuado (o sí, en pérdidas).
6. **Estado:** `validate_status_payload`.
7. **Tope de stock (H-A3):** `validate_stock_limit` para cada línea cuyo producto ya exista; el producto nuevo en una línea de compra no necesita validación (nace con el stock de la línea, ya acotado por `parse_quantity_milli`).
8. **Unicidad:** `UNIQUE(invoiceId, productIdOriginal)` la decide el `INSERT`.

### 4.1 Índice de comandos

La lista completa de los 35 comandos, con su módulo y su requisito funcional, vive en **§5.5 de `spec-03.md`**. Aquí se resume en un índice de consulta rápida:

| Módulo | Comandos |
|---|---|
| `commands/clients.rs` | `listClients`, `searchClients`, `getClientDetail`, `createClient`, `updateClient`, `deleteClient` |
| `commands/suppliers.rs` | `listSuppliers`, `searchSuppliers`, `getSupplierDetail`, `createSupplier`, `updateSupplier`, `deleteSupplier` |
| `commands/invoices.rs` | `listInvoices`, `getInvoice`, `createEntryInvoice`, `createSaleInvoice`, `createServiceInvoice`, `createGuaranteeInvoice`, `createInstallationInvoice`, `createLossInvoice`, `updateEntryInvoice`, `updateSaleInvoice`, `updateServiceInvoice`, `updateGuaranteeInvoice`, `updateInstallationInvoice`, `deleteInvoice`, `changeInvoiceStatus`, `saveInvoiceAttachment`, `downloadInvoiceAttachment` |
| `commands/history.rs` | `getProductHistory`, `listHistoryProducts`, `exportHistoryExcel`, `exportHistoryPdf` |
| `commands/products.rs` (ampliación) | `searchProductsForInvoice`, `getProductAssociatedInvoices` |

**Total:** 6 + 6 + 17 + 4 + 2 = **35 comandos nuevos**. Con los 15 de `spec-01` y los 26 de `spec-02`, el `generate_handler!` pasa a tener **76 comandos**.

**Precisión de conteo (H-5):** `commands/invoices.rs` tiene 17 comandos: `listInvoices`, `getInvoice`, 6 `create*Invoice` (uno por tipo), **5 `update*Invoice`** (entrada, venta, servicio, garantía, insumos — la pérdida no se edita), `deleteInvoice`, `changeInvoiceStatus`, `saveInvoiceAttachment`, `downloadInvoiceAttachment`. Coincide con `spec-03 §5.5` (11 comandos para create/update: 6 + 5).

### 4.2 DTOs (contrato de datos Rust ↔ JS)

Todo struct devuelto lleva `#[serde(rename_all = "camelCase")]`. Los textos decimales son los de `spec-02` §3.2 (cantidades como texto con coma, sin ceros finales). Los precios viajan como enteros de centavos.

| DTO | Lo devuelven | Campos (como los ve JS) |
|---|---|---|
| `ContactListItem` | `listClients`, `listSuppliers` | `id: number`, `fullName: string`, `company: string \| null`, `phone: string` |
| `ContactDetail` | `getClientDetail`, `getSupplierDetail` | `id`, `firstName`, `lastName`, `fullName`, `nationalId`, `phone`, `company`, `position`, `email`, `address`, `invoiceInfo?` (solo cliente), `createdAt`, `indicators: { totalInvoicedCents?/totalPurchasedCents, pendingBalanceCents, invoiceCount, lastPurchaseDate: string \| null }`, `invoiceHistory: InvoiceListItem[]` |
| `ContactMatch` | `searchClients`, `searchSuppliers` | `id`, `fullName`, `company: string \| null`, `isExactMatch: boolean` |
| `CreateContactResult` / `UpdateContactResult` | `createClient`, `updateClient`, `createSupplier`, `updateSupplier` | `ContactListItem` |
| `InvoiceListItem` | `listInvoices` y cada elemento de `invoiceHistory` | `number: string`, `type: 'E'\|'V'\|'S'\|'GG'\|'GI'\|'P'`, `contactName: string \| null`, `date: string` (`YYYY-MM-DD`), `totalCents: number`, `status: 'pending'\|'completed'\|'cancelled' \| null`, `associatedInvoice: { number: string, type: 'V'\|'S' } \| null` |
| `InvoiceDetail` | `getInvoice` | Los de `InvoiceListItem` más `notes`, `paymentMethod`, `paidAt`, `contactId: number \| null` y `associatedInvoiceId: number \| null` (ids vigentes para editar; `null` si se eliminaron, task-03 §7.3), `contactFullName`, `contactCompany`, `contactNationalId`, `contactPhone`, `contactEmail`, `createdByName`, `createdAt`, `updatedAt`, `lines: InvoiceLineDetail[]`, `attachmentFileName`, `attachmentOriginalName`, `attachmentSizeBytes`, `attachmentUploadedAt` |
| `InvoiceLineDetail` | dentro de `InvoiceDetail.lines` | `position: number`, `productIdOriginal`, `productName`, `unitName`, `unitAbbreviation`, `quantity: string`, `unitPriceCents`, `subtotalCents`, `categoryName?`, `costAtSaleCents?`, `productStatus: 'active' \| 'discontinued' \| null` (`null` = producto eliminado; limita su edición, task-03 §7.3) y, solo en compras, `remainingQuantity?`/`consumedQuantity?` (unidades del lote que quedan y las consumidas por salidas no canceladas; aviso al eliminar de FE-05, §7.8) |
| `InvoiceActionResult` | `create*Invoice`, `update*Invoice`, `deleteInvoice`, `changeInvoiceStatus` | `invoiceId: number`, `number: string`, `expiredCleanupWarning: string \| null` |
| `ProductMatch` | `searchProductsForInvoice` | `id`, `name`, `categoryName`, `unitAbbreviation`, `stock: string`, `sellingPriceCents: number` (solo para el total preliminar), `isOutOfStock: boolean`, `isDiscontinued: boolean` |
| `ProductHistoryItem` | `getProductHistory` | `id`, `createdAt`, `type: 'initial'\|'entry'\|...`, `quantity: string` (con signo), `stockAfter: string`, `unitPriceCents`, `totalPriceCents`, `invoiceNumber: string \| null`, `invoiceType: 'E'\|'V'\|... \| null`, `invoiceId: number \| null` (factura que sigue existiendo con ese número y tipo, que nunca se reutilizan; HP-02, task-03 §7.8), `userName`, `productIdOriginal` |
| `ProductHistoryList` | `getProductHistory` | `items: ProductHistoryItem[]`, `expiredCleanupWarning: string \| null` |
| `HistoryProductOption` | `listHistoryProducts` | `id: number`, `name: string`, `isDeleted: boolean` |
| `ProductAssociatedInvoice` | `getProductAssociatedInvoices` | `invoiceId`, `invoiceNumber`, `invoiceType`, `invoiceDate`, `status`, `quantity: string` |
| `ClientDetail` / `SupplierDetail` | `getClientDetail`, `getSupplierDetail` | Alias de `ContactDetail` con `invoiceInfo` (cliente) o sin él (proveedor) |

### 4.3 Catálogo de errores (código → texto exacto)

`errors.rs` **añade** los códigos nuevos y no cambia el comportamiento de los existentes. **Política de registro (H-A2):**

- **No se registran:** `validationFailed` e `invalidCredentials`.
- **Se registran con throttle de 60 s por `(code, userId)`:** `stockInsuficiente`, `loteEnUso`, `attachmentInvalid` y `stockLimitExceeded`.
- **Se registran siempre:** `duplicateField`, `notFound`, `invalidState`, `exportFailed`, `internal`.

| Código | Condición | Texto |
|---|---|---|
| `validationFailed` | campos de contacto (nombre, apellidos, empresa, cargo, dirección, información de factura) fuera de rango | Los del catálogo de `spec-02` (ya existen). |
| `validationFailed` | correo inválido | El correo no tiene un formato válido |
| `validationFailed` | CI inválido / teléfono inválido | Los de `spec-01` (ya existen). |
| `validationFailed` | fecha de factura futura | La fecha no puede ser futura |
| `validationFailed` | fecha de factura anterior a 2000-01-01 | La fecha no puede ser anterior al 1 de enero de 2000 |
| `validationFailed` | rango de fechas invertido | La fecha inicial no puede ser posterior a la final |
| `validationFailed` | más de 100 líneas | Una factura admite entre 1 y 100 productos |
| `validationFailed` | producto repetido en la misma factura | El producto ya está en la factura |
| `validationFailed` | total mayor que $999.999.999,99 | El total de la factura no puede superar $999.999.999,99 |
| `validationFailed` | producto descontinuado en factura nueva | El producto «X» está descontinuado. Actívalo para usarlo en facturas |
| `validationFailed` | cantidad de pérdida mayor que el stock | No hay stock suficiente de «X» (disponible: N) |
| `validationFailed` | adjunto no es PDF o supera 5 MB | El archivo debe ser un PDF de hasta 5 MB |
| `validationFailed` | factura asociada no es válida para el tipo | La factura original debe ser una venta o servicio de este cliente que no esté cancelada / La factura de servicio debe ser de este cliente y no estar cancelada |
| `validationFailed` | rango de fechas de historial invertido | La fecha inicial no puede ser posterior a la final |
| `duplicateField` | CI duplicado en clientes o proveedores | Ya existe un cliente con ese carnet de identidad / Ya existe un proveedor con ese carnet de identidad |
| `notFound` | entidad inexistente o purgada | El cliente ya no existe / El proveedor ya no existe / La factura ya no existe / El producto ya no existe |
| `invalidState` | editar una factura de entrada no editable | Solo se pueden editar las facturas que están Por Pagar |
| `invalidState` | editar una factura de venta o servicio no editable | Solo se pueden editar las facturas que están Por Cobrar |
| `invalidState` | editar una factura de garantía o insumo no editable | Solo se pueden editar las facturas que están Pendiente |
| `invalidState` | factura cancelada | Una factura cancelada no se puede reactivar ni editar |
| `invalidState` | mismo estado | La factura ya está en ese estado |
| `invalidState` | transición no permitida | La transición de estado no es válida para este tipo de factura |
| `invalidState` | lote con consumo (FE-04) | No se puede cambiar el costo de «X»: ya se usaron unidades de esta compra en la factura {número} / No se puede reducir «X» por debajo de las N unidades ya usadas / No se puede quitar «X»: ya se usaron unidades de esta compra |
| `stockInsuficiente` | consumo FIFO sin stock suficiente | No hay stock suficiente de «X» (disponible: N) |
| `stockLimitExceeded` | el stock total del producto superaría 999.999.999 milésimas | El stock total de «X» no puede superar 999.999,999 |
| `loteEnUso` | reservado para futuras restricciones de consumo (hoy no se usa, pero queda registrado para coherencia del catálogo) | — (no emitido actualmente) |
| `attachmentInvalid` | reservado para errores específicos de adjunto más allá de `validationFailed` (hoy no se usa) | — (no emitido actualmente) |
| `exportFailed` | reservado para errores específicos de exportación más allá de `internal` (hoy no se usa) | — (no emitido actualmente) |
| `notFound` | adjunto no encontrado en disco | El archivo adjunto no se encuentra en el equipo. Puede haber sido movido o eliminado |
| `internal` | fallo de exportación | No se pudo guardar el archivo |
| `internal` | cualquier error interno (incluida la purga best-effort **y la violación de los triggers de inmutabilidad de `productMovements` o `invoiceCounters`, que solo puede ocurrir por un bug interno**) | Ocurrió un error inesperado |

**Nota sobre `loteEnUso`, `attachmentInvalid`, `exportFailed`:** el catálogo los define para que la política de registro H-A2 sea explícita, pero en la práctica estos mensajes de usuario los emite `validationFailed` (`"El archivo debe ser un PDF..."`) o `internal` (`"No se pudo guardar el archivo"`). Se mantienen como códigos reservados por si en el futuro conviene distinguirlos. Si no se emiten, no generan filas en `errorLogs`.

**La lista completa de los 35 comandos y su firma interna vive en §5.5 de `spec-03.md`; este plan no la duplica, y §4.1 ofrece un índice resumido.** Cada `xxx_inner` recibe `(conn: &mut Connection, now, [today,] …)` y devuelve `Result<T, AppError>`.

---

## 5. Frontend: servicios, vistas y componentes

Se mantiene la separación de `plan-01` §6 y `plan-02` §5: las **vistas** manejan DOM y eventos; los **servicios** son la única capa que llama `invoke()` (siempre a través de `invoke.js`, que sigue siendo el único punto con `window.__TAURI__.core.invoke`). Sin estado propio fuera de la última respuesta como caché de vista; tras cada mutación se recarga. Toda vista tiene estados de carga, vacío ("Aún no hay clientes", "Aún no hay facturas") y error.

### 5.1 Servicios y `invoke.js`

- **`services/invoke.js`:** se reutiliza el de `spec-02` sin cambios. Sigue clasificando el rechazo de Tauri como "Los datos enviados no son válidos" y cualquier otro error sin forma `AppError` como "Ocurrió un error inesperado". El test existente sigue en verde.
- **Servicios nuevos:**
  - `services/clientsService.js`: wrappers de los 6 comandos de clientes.
  - `services/suppliersService.js`: wrappers de los 6 comandos de proveedores.
  - `services/invoicesService.js`: wrappers de los 17 comandos de facturas. Incluye `saveInvoiceAttachment(invoiceId, sourcePath)` y `downloadInvoiceAttachment(invoiceId, destinationPath)` que reciben rutas ya elegidas con `window.__TAURI__.dialog`.
  - `services/historyService.js`: wrappers de los 4 comandos de historial, más `searchProductsForInvoice` y `getProductAssociatedInvoices`.

### 5.2 Vistas nuevas (módulos ES)

- `views/clientesView.js`: dueño único de la sección Clientes (listar, crear, editar, eliminar, ver detalle). Registra listeners sobre los `<a>`/botones del sidebar y el toolbar. Recibe el historial de facturas del cliente **dentro de `getClientDetail`** (decisión B4): no llama a `invoicesService` por separado.
- `views/proveedoresView.js`: igual para proveedores, con la pestaña adicional "Historial de Compras" (PV-07) que usa `getSupplierDetail` para los indicadores y `listInvoices({ type: 'E', supplierId })` para el listado histórico.
- `views/facturasView.js`: dueño único de las 7 páginas de facturas (Entrada, Venta, Servicio, Insumo, Garantía, Pérdida, General). Un solo archivo con **submódulos internos** por tipo (decisiones B1 y A1 del dueño). Expone al resto de módulos, a través de `window`, las funciones `irCrearCliente`, `irCrearProveedor` y una función para abrir un detalle de factura desde otra vista (`irAFacturaDesdeGeneral`).
- `views/historialView.js`: dueño único de la pestaña Historial dentro de la página Productos (decisión B1). Escucha el evento `product:viewHistory` (disparado por el botón "Ver Historial" del detalle de producto, gestionado por `productosView.js` de `spec-02`) para abrir la pestaña con ese producto preseleccionado.

### 5.3 Componentes

Ninguno nuevo. Se reutilizan `components/combobox.js` y `components/confirmDialog.js` de `spec-02`. El combobox se usa para el buscador de productos en facturas, el buscador de clientes/proveedores con alta sobre la marcha, y el buscador de productos en pérdidas (sin "Agregar nueva" en este último caso).

### 5.4 `utils/formatters.js`

Se reutiliza. Añade si no está:
- `formatMoney(cents)` (ya existe en `spec-02`).
- `formatId(n)` con relleno de ceros (ya existe).
- `formatQuantity(milli)` (ya existe).
- `formatDate(iso)` — se usa para **fechas de calendario** (`YYYY-MM-DD`): formatea por partes de la cadena, sin `Date`. Se usa también para las fechas de factura.
- `formatTimestamp(iso)` — para timestamps UTC (fecha + hora), los convierte a hora local.

### 5.5 `dashboard.js` — orquestador

Sigue siendo script clásico. Se le hacen estos cambios mínimos:
- **`formatearFecha(iso)`**: se mantiene como wrapper del helper `formatDate` con import dinámico cacheado (patrón confirmado en la spec §9). El fallback es un formateo por partes de la cadena; la salida es idéntica. **Solo para fechas nulas o vacías** devuelve `"—"`; los demás `|| ":"` del código legado se quedan como están.
- **`showPage()`**: se mantiene y sigue llamando a las funciones de navegación. **Las llamadas a `renderClientes()` y `renderProveedores()` se eliminan** (ya no existen). Se añade el despacho de un evento `page:change` con `{ detail: { pageId } }` para que `clientesView.js` y `proveedoresView.js` se inicialicen cuando corresponda (decisión B2).
- **Lista de `knownPages`**: se añaden los ids de las 7 páginas de facturas.
- **`ordenFechaState`/`ordenFechaThIds`**: se mantienen; los módulos nuevos llaman a `window.toggleOrdenFecha` cuando necesitan ordenar.
- **Bloque final de notificaciones iniciales**: se elimina completo, incluida la llamada `crearNotificacion('factura', 'Bienvenido al Sistema', ...)` y el `.forEach` que marca las notificaciones como leídas.
- **Llamadas a `crearNotificacion` y `notificarFactura` desde facturas y pérdidas**: se eliminan. La función `crearNotificacion` se mantiene (la usa la página de notificaciones).

### 5.6 Manejadores inline del HTML

Se **eliminan** y se reemplazan por listeners registrados desde los módulos ES, siguiendo `plan-02` §7.7. Esto cubre:
- Todos los `onclick="showClienteTab(…)"` y `onclick="showProveedorTab(…)"` de los tab-buttons.
- Todos los `oninput="renderClientes()"` / `oninput="renderProveedores()"` de los buscadores.
- Todos los `onclick="showFactura*Tab(…)"` de los tab-buttons de facturas.
- Todos los `oninput="renderFacturas*()"` / `onchange="renderFacturasGeneral()"` de los buscadores y filtros.
- Todos los `onclick="eliminarFactura*()"` / `onclick="verFactura*()"` de las acciones de fila.
- Los `onclick="toggleOrdenFecha('facturas*', renderFacturas*)"` de los `<th>` ordenables.
- Los `onclick="exportarFactura*PDF(factura*VerNumero)"` de los botones de exportación.
- Los `onclick="return showPage('facturas-*', this, '…')"` del submenú del sidebar.
- Los `onclick="toggleOrdenFecha('historialProveedor', renderHistorialProveedor)"` y `onchange="renderHistorialProveedor()"` de la pestaña de historial del proveedor.
- El `onclick="toggleOrdenFecha('historialTasas', renderHistorialTasas)"` de la pestaña de tasas **no se toca** (Configuración no se migra en esta spec).

### 5.7 `session.js`

Se reutiliza sin cambios. Los módulos nuevos lo importan para saber el rol y el username.

### 5.8 Formato de números y fechas en la UI

Se reutilizan los helpers de `spec-02`. La única novedad es que las fechas de factura se muestran como `dd/mm/aaaa` (ya es el formato de `formatDate`).

### 5.9 Salida segura (A-01)

Todo dato del usuario se pinta con `textContent`, `createElement` y `replaceChildren`. Las vistas y componentes nuevos no usan `innerHTML` con datos. Un test estático en `environment.test.js` y, por vista, un test jsdom que guarda un nombre como `<img src=x onerror=…>` y comprueba que se ve como texto y no crea ningún elemento.

### 5.10 Pausa, orden de respuestas y doble envío

Se reutiliza `utils/requestGuard.js` de `spec-02`. Las búsquedas y filtros de facturas usan `debounce(300)`; las llamadas al backend en general usan `latestOnly`; los botones de guardar/confirmar usan `withBusy`.

### 5.11 Aviso de purga fallida

Los servicios exponen `expiredCleanupWarning` de las respuestas que lo traen (§4.2). Cada vista que lo recibe lo muestra como texto discreto (`textContent`, `role="status"`) y lo limpia cuando llega `null`.

### 5.12 Contrato de `expiredCleanupWarning`

Los comandos de la lista de `spec-03` §7.6 que listan, cuentan o reciben un `productId` devuelven el campo. En las respuestas de escritura (`createEntryInvoice`, etc.) también se incluye como campo opcional (`InvoiceActionResult`).

### 5.13 Orden de los `<script type="module">` en `dashboard-unificado.html`

Los módulos de `spec-03` se añaden **después** de los de `spec-02`, en el orden: `clientesView.js`, `proveedoresView.js`, `facturasView.js`, `historialView.js`. Todos se ejecutan antes de `dashboard.js` (script clásico, va después).

### 5.14 Módulos internos de `facturasView.js`

Un solo archivo con submódulos por tipo, cada uno con sus funciones de listado, formulario, detalle y acciones. Comparte entre ellos:
- El manejo de errores de `invoke.js`.
- Los helpers de formato (`formatMoney`, `formatDate`, `formatId`, `formatQuantity`).
- El componente `combobox` y `confirmDialog`.
- El diálogo nativo para elegir rutas de PDF (`saveInvoiceAttachment`, `downloadInvoiceAttachment`).
- Las funciones de filtros y búsqueda.

### 5.15 Cambios a `spec-02` (backend)

Esta spec **modifica** varias decisiones de `spec-02`. `spec-02` no se reescribe: sigue siendo la referencia histórica de la feature 02. Los cambios se implementan desde esta spec y se documentan aquí.

| Elemento de `spec-02` | Cambio | Motivo |
|---|---|---|
| `pricing.rs::weighted_average_cost_cents` | Eliminar la función y su test | SP-02 (el costo se calcula sobre lotes) |
| `ProductListItem` (DTO Rust) | Añadir `valorTotalCents: number` | SP-03 |
| `ProductDetail` (DTO Rust) | Añadir `valorTotalCents: number` | SP-03 |
| `CategoryListItem` (DTO Rust) | Añadir `ingresosCents: number` | SP-03 |
| `CategoryDetail` (DTO Rust) | Añadir `ingresosCents: number` | SP-03 |
| `commands/products.rs::list_products_inner` | Calcular `valorTotalCents` desde los lotes | SP-03 |
| `commands/products.rs::get_product_inner` | Calcular `valorTotalCents` desde los lotes | SP-03 |
| `commands/categories.rs::list_categories_inner` | Calcular `ingresosCents` desde `invoiceLines.categoryId` + `subtotalCents` de facturas V y S no canceladas | SP-03 |
| `commands/categories.rs::get_category_inner` | Calcular `ingresosCents` | SP-03 |
| Tests de `spec-02` §6.1 y §6.6 ("ningún comando modifica la cantidad") | Anotar como "válido solo dentro de `spec-02`"; añadir un comentario en el código | §12 bis de la spec |
| Test de `spec-02` §14 ("con datos reales ningún activo está agotado") | Anotar como "válido solo dentro de `spec-02`" | §12 bis de la spec |

### 5.16 Cambios a módulos frontend de `spec-02`

| Módulo | Cambio | Motivo |
|---|---|---|
| `views/productosView.js` | Al pulsar el botón "Ver Historial" del detalle, disparar el evento `product:viewHistory` con `{ productId }` | HP-02: `historialView.js` lo escucha para abrir la pestaña con el producto preseleccionado |
| `views/categoriasView.js` | Mostrar la columna "Ingresos" del listado y del detalle (quitar `hidden` y pintar `ingresosCents`) | SP-03 |
| `views/productosView.js` | Mostrar la columna "Valor Total" del listado y el campo del detalle (quitar `hidden` y pintar `valorTotalCents`) | SP-03 |

**Nota de compatibilidad:** estos cambios no rompen el comportamiento existente de `spec-02`. El evento `product:viewHistory` es nuevo y solo lo emite `productosView.js` cuando el usuario pulsa el botón; si `historialView.js` no está cargado (por ejemplo, en un test aislado de `productosView.js`), el evento se ignora.

---

## 6. Estructura de carpetas (ampliación)

**Backend (`src-tauri/src/`)** — se añade a lo existente (`commands/auth.rs`, `commands/users.rs`, `commands/categories.rs`, `commands/units.rs`, `commands/conversions.rs`, `commands/products.rs`, `commands/dashboard.rs`, `db/schema.rs`, `db/connection.rs`, `db/notifications.rs`, `models/*.rs`, `validators.rs`, `errors.rs`, `text.rs`, `timestamps.rs`, `pricing.rs`, `main.rs`, `lib.rs`):

```
src-tauri/src/
  commands/
    clients.rs         // CL-01..06
    suppliers.rs       // PV-01..07
    invoices.rs        // FE-*, FS-*, FP-*, FG-01, adjunto
    history.rs         // HP-01..04 (consulta y exportación)
  db/
    schema.rs          // se amplía con las tablas de §2
    notifications.rs   // sin cambios (reutilizado)
  models/
    client.rs  supplier.rs  invoice.rs  invoice_line.rs
    lot.rs  consumption.rs  product_movement.rs  // DTOs de §4
  inventory.rs         // lotes, FIFO, consumo, devolución, recálculo de cachés, validate_stock_limit, reverse_lots_of_invoice (§3.2)
  invoice_counter.rs   // numeración por tipo (§3.3)
  invoice_snapshot.rs  // copias de contacto, línea y movimiento (§3.4)
  state.rs             // enum y validaciones de estados (§3.7)
  attachments.rs       // PDF: validar, guardar, borrar, barrido (§3.8)
  export.rs            // exportación a Excel y PDF (§3.10)
  search.rs            // búsqueda por relevancia (§3.5)
  pricing.rs           // se elimina weighted_average_cost_cents
  errors.rs            // se amplía: códigos nuevos, should_log y throttle del registro (§3.11)
  validators.rs        // se amplía: validate_client_input, etc.
  lib.rs               // declara los módulos nuevos y registra los 35 comandos
```

**Frontend (`js/`)** — se añade a lo existente de `spec-01` y `spec-02`:

```
js/
  views/       clientesView.js  proveedoresView.js  facturasView.js  historialView.js
  services/    clientsService.js  suppliersService.js  invoicesService.js  historyService.js
  __tests__/   un archivo por módulo nuevo (§8)
```

No hay archivos de `components/` ni `utils/` nuevos: se reutilizan los de `spec-02`.

---

## 7. Adaptación de interfaz existente

Los cambios de interfaz salen de la revisión completa de `dashboard-unificado.html` y `dashboard.js`. **Criterio rector: la spec manda**; cuando el HTML/JS actual la contradice, se cambia el frontend. Cada fila indica la tarea de `tasks-03.md` donde se implementa (T §n).

### 7.1 Cambios transversales (T-01 a T-15)

| ID | Cambio | Spec | Tarea |
|---|---|---|---|
| T-01 | **Capa de acceso a datos.** Cada listado, alta, edición y baja llama a un comando Tauri; el frontend solo conserva la última respuesta como caché de vista y recarga tras cada mutación. Se eliminan los datos de ejemplo y los contadores `next…Id`. | §2, §8 | T §2, §3, §4, §5 |
| T-02 | **Referencias por id.** Selectores y combobox manejan `{id, nombre}`; nunca se envía ni compara un nombre como clave. | 7.7 | T §5 |
| T-03 | **Errores.** Se muestra el mensaje específico del backend en pantalla; los internos, el genérico. `invoke.js` convierte el rechazo de Tauri en "Los datos enviados no son válidos" y cualquier otro error sin forma `AppError` en "Ocurrió un error inesperado"; los `AppError` se devuelven intactos. | §8 | T §1 bis |
| T-04 | **Diálogo de confirmación reutilizable** (`confirmDialog.js` de `spec-02`) con texto multilínea y conteos; reemplaza cada `confirm()`. | CL-05, PV-05, FE-05, FE-06, FS-01, FP-01 | T §1 bis |
| T-05 | **Normalización de búsqueda única** (la de `spec-02` §7.1) hecha por el backend; el cliente no tiene copia. | 7.1, 7.2 | T §2.1 |
| T-06 | **Combobox con creación sobre la marcha** (`combobox.js` de `spec-02`) para cliente, proveedor y producto. En pérdidas, sin "Agregar nueva". | CL-06, PV-06, FE-01, FS-01, FP-01 | T §5 |
| T-07 | **Números y dinero.** Todo campo numérico es `<input type="text" inputmode="decimal">`; el texto se envía tal cual y el backend valida y convierte con enteros. | 7.4 | T §5 |
| T-08 | **UTF-8.** Corregir las cadenas dañadas (por ejemplo "Garantía", "Pérdidas") en cada función que se toque. | — | T §2, §3, §4, §5 |
| T-09 | **Paginación.** Se mantiene el paginador global (DOM, `MutationObserver` sobre el `<tbody>`); los nuevos `<tbody>` se añaden a `PAGINABLES` en `dashboard.js`. | §9 | T §5 |
| T-10 | **Código simulado de facturas.** `facturasEntradaData`, `facturasVentaData`, etc., y las funciones que los consumen se eliminan. | §3.2 | T §2, §3, §4, §5 |
| T-11 | **Pausa, respuestas viejas y doble envío** (`requestGuard.js` de `spec-02`). | 9 | T §1 bis, §5 |
| T-12 | **Salida segura** (A-01): datos del usuario siempre como texto. | 9 | T §1 bis, §2, §3, §4, §5 |
| T-13 | **Regla global `[hidden]`** ya está en `dashboard.css` desde `spec-02`; no se toca. | — | — |
| T-14 | **Manejadores inline.** Se eliminan todos los `onclick`/`oninput`/`onchange`/`onsubmit` de las secciones migradas y se reemplazan por listeners desde los módulos ES. | §9 | T §2, §3, §4, §5 |
| T-15 | **`formatearFecha`** se mantiene como wrapper del helper `formatDate` con import dinámico cacheado. Devuelve `"—"` solo cuando la fecha es nula o vacía. | §9 | T §5 |

### 7.2 Clientes y Proveedores

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-CL-01 | `#cliente-fecha-entrada` y su `<label>` | **Eliminar del formulario** (se calcula en el backend al crear; se muestra en el detalle como `#cliente-ver-fecha-entrada`). | 5.1 | T §2 |
| FE-CL-02 | `#cliente-ver-historial-facturas` | Se llena con `getClientDetail().invoiceHistory` (decisión B4). | CL-03 | T §2 |
| FE-CL-03 | Botón "Editar" del detalle y "Eliminar" | Reemplazar `onclick="editarClienteDesdeVer()"` / `eliminarClienteDesdeVer()` por listeners del módulo. | CL-04, CL-05 | T §2 |
| FE-CL-04 | `#cliente-search` | Reemplazar `oninput="renderClientes()"` por listener con `debounce(300)`. | CL-02 | T §2 |
| FE-CL-05 | Tab-buttons `#tab-cliente-list` / `#tab-cliente-create` | Reemplazar `onclick="showClienteTab(…)"` por listeners. | CL-01 | T §2 |
| FE-PV-01 a 05 | Ídem para proveedores (incluido `#proveedor-historial`) | | PV-01 a 07 | T §3 |
| FE-PV-06 | Pestaña "Historial de Compras" | Datos reales de `getSupplierDetail().invoiceHistory`. Filtros de proveedor y rango de fechas por listener. | PV-07 | T §3 |
| FE-PV-07 | Botón "Historial" en la fila | Abre la pestaña con el proveedor seleccionado. | PV-07 | T §3 |

### 7.3 Facturas de entrada

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-FE-01 | `<select>` de estado | `<option value="pending">Por Pagar</option>` · `<option value="completed">Pagada</option>` · `<option value="cancelled">Cancelada</option>` (sin "Vencida"). | FE-06 | T §4 |
| FE-FE-02 | `<select>` de método de pago | Renombrar "Efectivo USD" → "Efectivo", "Transferencia USD" → "Transferencia". Eliminar "Efectivo CUP". Solo visible con estado Pagada/Cobrada. | §9 | T §4 |
| FE-FE-03 | `#factura-descuento`, `#factura-impuesto` | **Eliminar** del formulario y del detalle. | §3.2 | T §4 |
| FE-FE-04 | `#factura-buscar-producto` | Combobox (no `<datalist>`). Con botón `+` de agregar. | FE-01 | T §4 |
| FE-FE-05 | `#factura-proveedor` | Combobox con alta rápida. El botón `+` abre el formulario del proveedor. | FE-01 | T §4 |
| FE-FE-06 | Botón "Editar" del detalle | Listener que abre la edición solo si `status === 'pending'`; en otro estado, oculto. | FE-04 | T §4 |
| FE-FE-07 | Botón "Exportar Factura" | Mantener `exportarFacturaPDF` adaptado al DTO (`getInvoice`). | §9.X.4.b | T §4 |
| FE-FE-08 | Adjunto PDF (dropzone) | Se mantiene el dropzone existente `#factura-dropzone`; el guardado usa `saveInvoiceAttachment`. Se añade un botón "Guardar en el equipo" en el detalle que usa `downloadInvoiceAttachment`. | FE-07 | T §4 |
| FE-FE-09 | `#factura-lineas-body` y `#factura-ver-productos` | Mantienen la estructura `li-row li-body`. Se pasa a `textContent` para los datos. | §9.X.2 | T §4 |

### 7.4 Facturas de salida

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-FS-01 | `#factura-venta-estado` | `<option value="pending">Por Cobrar</option>` · `<option value="completed">Cobrada</option>` · `<option value="cancelled">Cancelada</option>` (sin "Vencida", sin "Pagada"). | FS-01 | T §5 |
| FE-FS-02 | `#factura-servicio-estado` | Igual que venta. | FS-01 | T §5 |
| FE-FS-03 | `#factura-garantia-estado` y `#factura-insumo-estado` | `<option value="pending">Pendiente</option>` · `<option value="completed">Completada</option>` · `<option value="cancelled">Cancelada</option>`. | FS-04, FS-05 | T §5 |
| FE-FS-04 | `#factura-garantia-dropzone` | **Eliminar.** Reemplazar por `<select id="factura-garantia-original" required>` que se llena al elegir cliente con las ventas y servicios del cliente. | FS-04 | T §5 |
| FE-FS-05 | `#factura-insumo-dropzone` | **Eliminar.** Reemplazar por `<select id="factura-insumo-servicio" required>`. | FS-05 | T §5 |
| FE-FS-06 | `#factura-garantia-ver-original` | Mostrar `{numero} · {tipo visible}`. | FS-04 | T §5 |
| FE-FS-07 | `#factura-insumo-ver-asociada` | Ídem. | FS-05 | T §5 |
| FE-FS-08 | `#factura-venta-lineas-body` | Se añade el botón "Agregar Fila" (no existe hoy) y el botón `+` junto al buscador, replicando el patrón de los otros formularios. | FS-01 | T §5 |
| FE-FS-09 | `#factura-servicio-buscar`, `#factura-garantia-buscar` | Combobox (no `<datalist>`). El botón "+" de servicio se conserva; el de garantía se elimina porque el producto se crea al guardar la factura. | FS-03, FS-04 | T §5 |
| FE-FS-10 | `#factura-venta-descuento`, `#factura-venta-impuesto`, `#factura-servicio-descuento`, `#factura-servicio-impuesto`, `#factura-insumo-descuento`, `#factura-insumo-impuesto`, `#factura-garantia-descuento`, `#factura-garantia-impuesto` | **Eliminar** (inputs y bloques del detalle). | §3.2 | T §5 |

### 7.5 Facturas de pérdida

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-FP-01 | `#factura-perdida-costo` | `<input type="text" readonly>` con `placeholder="—"`. No se envía. | FP-01 | T §5 |
| FE-FP-02 | `#factura-perdida-cantidad` | `<input type="text" inputmode="decimal">` (acepta 3 decimales). | §7.2 | T §5 |
| FE-FP-03 | `#factura-perdida-producto` | Combobox sin "Agregar nueva" (solo activos y descontinuados con stock). | FP-01 | T §5 |
| FE-FP-04 | Formulario de pérdida | Solo producto, cantidad, fecha y descripción. Sin costo. | FP-01 | T §5 |
| FE-FP-05 | Detalle de pérdida | `#factura-perdida-ver-costo-total` y `#factura-perdida-ver-costo` se llenan con el costo real del backend. | FP-01 | T §5 |

### 7.6 Lista general y detalle de factura (transversal a todos los tipos)

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-FG-01 | `#factura-general-tipo` | `<option value="">Todos</option>` + `<option value="E">Entrada</option>` + `<option value="V">Venta</option>` + `<option value="S">Servicio</option>` + `<option value="GG">Garantía</option>` + `<option value="GI">Insumo</option>` + `<option value="P">Pérdida</option>`. | FG-01 | T §6 |
| FE-FG-02 | `#factura-general-estado` | `<option value="">Todos</option>` · `<option value="pendientes">Pendientes</option>` · `<option value="completadas">Completadas</option>` · `<option value="canceladas">Canceladas</option>`. | FG-01 | T §6 |
| FE-FG-03 | `renderFacturasGeneral` | Reescribir para usar `listInvoices(filters)` con datos reales. `textContent` para los datos. | FG-01 | T §6 |
| FE-FG-04 | `obtenerFacturasCombinadas` y `facturaTipoInfo` | **Eliminar.** La lista general la sirve el backend. | §9.X.4.a | T §6 |
| FE-FG-05 | Bloques de "Descuento" e "Impuestos" en los 6 detalles | **Eliminar**. | §3.2 | T §6 |

### 7.7 Historial de producto

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-HP-01 | `#tab-producto-historial`, `#producto-historial` | Quitar `hidden`. | HP-02 | T §7 |
| FE-HP-02 | `#producto-ver-historial`, `#producto-ver-facturas`, `#producto-facturas-asociadas` | Quitar `hidden`. | HP-02, HP-04 | T §7 |
| FE-HP-03 | `#historial-producto-tipo` | `<select multiple>` con 3 `<optgroup>`; valores = códigos internos; sin "Todos". | HP-02 | T §7 |
| FE-HP-04 | `#historial-producto-select` | Poblar con `listHistoryProducts()`, con `<optgroup>` de activos y eliminados. | HP-02 | T §7 |
| FE-HP-05 | `#historial-exportar-excel`, `#historial-exportar-pdf` | **Crear** dos `<button>` al final del `.historial-toolbar` (con `title` y `aria-live`). Deshabilitados cuando no hay movimientos. | HP-03 | T §7 |
| FE-HP-06 | `#historial-producto-body` | Se llena con `getProductHistory(filters)`. `textContent` para los datos. | HP-02 | T §7 |
| FE-HP-07 | `#producto-ver-facturas` | Sección "Facturas Asociadas". Se llena con `getProductAssociatedInvoices({ productId })`. | HP-04 | T §7 |
| FE-HP-08 | Botón "Ver Historial" del detalle de producto | Dispara el evento `product:viewHistory` con `{ productId }`. `historialView.js` lo escucha y abre la pestaña con el producto preseleccionado. | HP-02 | T §7 |

### 7.8 Productos y categorías

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-SP-01 | `#productos-valor-total` (columna del listado) | Quitar `hidden`. Se llena con `valorTotalCents` del `ProductListItem` (ampliación de `spec-02`). | SP-03 | T §7 |
| FE-SP-02 | Columna "Ingresos" del listado de categorías y del detalle | Quitar `hidden`. Se llena con `ingresosCents` del `CategoryListItem`. | SP-03 | T §7 |
| FE-SP-03 | Sección "Facturas Asociadas" del detalle de producto | Ya está en FE-HP-02. | SP-03 | T §7 |

### 7.9 Notificaciones

| ID | Elemento | Cambio requerido | Spec | Tarea |
|---|---|---|---|---|
| FE-NT-01 | Bloque final de `dashboard.js` | **Eliminar completo** (incluida "Bienvenido al Sistema"). | §3.2 | T §8 |
| FE-NT-02 | Llamadas a `crearNotificacion`/`notificarFactura` desde facturas y pérdidas | **Eliminar**. | §3.2 | T §5, §8 |
| FE-NT-03 | Pestaña `#page-notificaciones` y sus funciones | **No tocar.** | §3.2 | — |

### 7.10 Sidebar de facturas (verificación en implementación)

El id o `data-page` **real** del enlace del submenú de facturas en el sidebar se verifica en `tasks-03.md` §0 con `grep` sobre `dashboard-unificado.html` (verificación previa §1, punto 6). El valor se anota en `plan-03` §5.6/§7.1 una vez conocido. El patrón de ocultación/mostrado sigue siendo `a[data-page="<valor real>"]` + `.closest('li').hidden = false`, con la página `#page-facturas-*` también con `hidden` quitado. No se asume ningún valor antes de la verificación.

---

## 8. Estrategia de testing (test-first, `Constitution.md` §2.III)

**Backend (Rust):** tests sobre las funciones `xxx_inner` y los módulos puros, con `setup_test_db()` (en memoria, `foreign_keys = ON`) y fechas inyectadas (`now`, `today`). Los productos se crean con `insert_product_fixture` (de `spec-02`) o mediante `createProduct` cuando el test necesita el lote inicial. Los fallos forzados usan **triggers de prueba** (`RAISE(ABORT)`) creados solo en el test.

### 8.1 Convivencia de las suites de test

Cada spec tiene su propio módulo de test:

- `mod tests_spec01` — 97 tests de `spec-01`.
- `mod tests_spec02` — los tests de `spec-02`.
- `mod tests_spec03` — los tests de esta spec.

Los tres módulos se declaran en sus respectivos archivos con `#[cfg(test)]` y se ejecutan en el mismo binario de `cargo test`. Como el esquema es acumulativo (`init_schema` crea todas las tablas), cada suite ve el esquema completo y no colisiona con las demás. Cada suite usa sus propios fixtures y sus propias aserciones.

**Anotaciones necesarias en tests de `spec-02`:** los tests que dejaron de ser ciertos globalmente se mantienen con un comentario explícito:

```rust
// Nota: esta aserción es válida solo dentro de spec-02. spec-03 cambia el
// comportamiento (los comandos de facturas sí modifican quantityMilli).
```

Los tests afectados son:
- `spec-02` §6.1: "ningún comando de esta spec la modifica después".
- `spec-02` §6.6: "los comandos ... no alteran la cantidad".
- `spec-02` §14: "con datos reales ningún activo está agotado".
- `spec-02` §8: "P-10 sin consumidor".

**Test de contrato `invoke ↔ generate_handler!`:** el test existente (de `tasks-02.md` §8) pasa a esperar **76 comandos** (15 de `spec-01` + 26 de `spec-02` + 35 de `spec-03`). El test **calcula el recuento a partir de los archivos** (nombres `snake_case` en `generate_handler!` y en los wrappers `services/*.js`), no lo compara contra un literal fijo; la cifra 76 es informativa y no aparece en el código del test.

### 8.2 Tabla de áreas cubiertas

| Área | Qué se prueba |
|---|---|
| Autorización | Matriz de 35 filas (una por comando de §5.5 de la spec): sin sesión → `sessionExpired`; con Administrador y con Contador → permitido. La verificación final confirma que ningún comando de esta feature quedó fuera. |
| Esquema | Un test por cada `CHECK`, `UNIQUE`, clave foránea y trigger: contacto con CI duplicado falla, teléfono duplicado pasa; `invoiceCounters` no decrece ni se borra; `productMovements` inmutable; `lotsOneInitial`; par de `arrivalOrder` duplicado; `invoices.type` fuera de la lista falla; los `CHECK` de estado/pago/adjunto con un valor que fallaría por `NULL`; los timestamps imposibles rechazados. **Tests dedicados de triggers (H-3):** `invoiceCountersNoDecrease`, `invoiceCountersNoDelete`, `productMovementsImmutable` (incluido el `UPDATE` mixto de N-4), `productMovementsNoDelete` y la interacción FK↔trigger (`SET NULL` no se bloquea). |
| Texto y orden | Reutilizado de `spec-02`. Los contactos y las facturas usan las mismas funciones. |
| Números | Reutilizado de `spec-02`. Las líneas de factura usan `parse_quantity_milli` y `parse_cost_cents` (o `parse_selling_price` para ventas). |
| Fechas | Fecha de factura futura rechazada, anterior a 2000 rechazada, `2026-02-30` rechazada. Rango invertido rechazado. Rango con una sola fecha funciona. |
| FIFO | El ejemplo de LT-02 de la spec (10 a $1, 5 a $3, 8 a $2; vender 12 → costo $16, quedan 3 y 8, costo del producto $2,27). Consumo que cruza varios lotes. Orden por registro, no por fecha de factura. Cantidad mayor que el stock rechazada sin cambios. |
| Devolución | Cancelar/eliminar/editar una salida devuelve a los mismos lotes. Con lote inexistente → lote de restitución con el costo del consumo original, el más antiguo. Con producto inexistente → operación continúa con aviso. **H-A1:** eliminar una compra Cancelada con lotes restaurados quita lo restante y registra la Reversión. |
| Costo del producto | Sin ventas, coincide con el promedio de `spec-02` ($1,33). Con stock 0, conserva el último costo. Con compra posterior al agotamiento, pasa al costo de la compra. Tras cualquier operación sobre lotes, `products.quantityMilli` = suma de `remainingMilli` y `products.costCents` = promedio de los lotes con existencia (invariante de caché). |
| Tope de stock (H-A3) | `validate_stock_limit` rechaza con `stockLimitExceeded` cuando la suma superaría 999.999.999; se prueba en `createProduct`, `createEntryInvoice`, `updateEntryInvoice` y `restore_consumption`. |
| Entrada | Crear con producto nuevo (se crea con la factura). Rollback completo si falla cualquier línea. Estado inicial Pagada con fecha de pago. Editar con lote consumido (rechazos con mensaje y factura de salida). Eliminar y cancelar quitan solo lo restante. Por Pagar ↔ Pagada no toca el inventario. |
| Salidas | Venta, servicio, garantía, insumos y pérdida descuentan por FIFO y registran su movimiento. Total de venta/servicio a precio de venta vigente; total de garantía/insumos/pérdida a costo real. Líneas editadas conservan precio. **H-A4:** un cambio de costo por re-FIFO sin cambio de cantidad no genera Ajuste. |
| Estados | Todas las transiciones de §7.5; Cancelada final; mismo estado rechazado. Método de pago solo con `completed`/`cancelled`. |
| Historial | Un movimiento por operación y por tipo; signo, stock resultante y precios correctos; no hay movimiento en cambios de estado entre pendiente y completada ni al eliminar una Cancelada sin restos; el movimiento sobrevive a la eliminación del producto, de la factura y del usuario; ningún comando lo edita ni lo borra; filtros por rango de días locales y por varios tipos (semántica OR); selector con productos eliminados. **H-A1:** eliminar una compra Cancelada con lotes restaurados sí registra una Reversión. |
| Exportación | El `.xlsx` y el `.pdf` contienen exactamente los movimientos filtrados (no solo la página). Encabezado correcto. Sin movimientos → botones deshabilitados. Fallo de escritura registrado y mensaje claro. |
| Adjunto PDF | PDF válido aceptado. Archivo no PDF (aunque tenga extensión `.pdf`), mayor de 5 MB o con ruta en el nombre rechazado. Reemplazo y quitado en Por Pagar. Descarga en cualquier estado (incluido el archivo corrupto de H-10). Archivo en `app_data_dir/attachments/` con nombre UUID. Eliminar la factura borra el archivo. Fallo de borrado registrado sin bloquear. Barrido del arranque borra huérfanos y es best-effort. |
| Numeración | Contadores independientes por tipo. Sin reutilización tras eliminar. `nextNumber` no decrece. `E-100000` a partir de 100.000. |
| Totales y fechas | Más de 100 líneas, producto repetido, total mayor que $999.999.999,99, fecha futura, anterior a 2000 e imposible rechazados. |
| Indicadores | Total facturado, saldo pendiente, número y última compra de clientes y proveedores. |
| Valor Total e Ingresos | Valor total por producto con varios lotes y redondeo único. Ingresos por categoría usando la categoría guardada en la línea. Sin canceladas ni líneas sin categoría. |
| Atomicidad | Con un fallo forzado en la inserción del movimiento, de un consumo o del recálculo de costo, no queda ningún cambio de factura, lote ni inventario. |
| Purga | Un descontinuado vencido se trata como inexistente en facturas y movimientos. Los comandos de esta spec que lo tocan ejecutan la purga best-effort. **`listHistoryProducts` no purga (decisión de diseño, §3.9).** |
| Errores | `validationFailed` e `invalidCredentials` no generan fila en `errorLogs`; `duplicateField`, `notFound`, `invalidState`, `stockInsuficiente`, `stockLimitExceeded`, `exportFailed` e `internal` sí (los que tienen throttle, bajo la política H-A2). `cause` nunca contiene valores ingresados. La violación de los triggers de inmutabilidad se traduce a `internal` (O-3). |
| Throttle del registro (H-A2) | El mismo `stockInsuficiente` con el mismo usuario produce 1 sola fila en 60 s; con `userId` distinto, 2 filas; tras 61 s (reloj inyectado) vuelve a registrar; `validationFailed` e `invalidCredentials` no generan fila nunca. |
| Auditoría | `invoices.updatedAt` cambia con `format_utc_timestamp(now)` en `update*Invoice`, `changeInvoiceStatus` y `saveInvoiceAttachment`; `createdAt` no cambia (N-1, O-B). |
| Contrato `invoke ↔ generate_handler!` | 76 comandos. El test calcula el recuento a partir de los archivos, no de un literal fijo. |
| Contrato de structs | Los structs de entrada de `createEntryInvoice`, `updateEntryInvoice`, etc. **fallan al deserializarse** con los campos prohibidos (por ejemplo, `sellingPriceCents` al crear, `quantity` al editar). Los campos válidos pasan. |
| Regresión de `selling_price_cents` | Tras eliminar `weighted_average_cost_cents`, `selling_price_cents` sigue produciendo los mismos valores para una tabla de casos (O-1). |
| `printpdf` con Latin-1 | Test que genera un PDF con `á é í ó ú ü ñ ¿ ¡ €` y comprueba que el archivo no está vacío y que los caracteres se pintan (verificación previa §1, punto 7). |

### 8.3 Frontend

Con `window.__TAURI__` sustituido por un stub **solo dentro de los tests**. Se verifica que cada vista llama al servicio correcto y usa la respuesta del backend (sin datos de ejemplo), los estados de carga/vacío/error, que el combobox se usa en los formularios correctos, que `confirmDialog` se usa para las eliminaciones y cancelaciones, que `formatters` da el formato correcto, que los campos numéricos envían el texto tal cual, que `invoke.js` convierte un rechazo en texto plano en "Los datos enviados no son válidos" sin alterar los `AppError`, y la protección del dashboard (que `#reserved-data-warning` y `usuarios.js`/`perfil.js` siguen referenciados). **Además:**
- El evento `page:change` se despacha con el `pageId` correcto y las vistas lo escuchan (N-3, en `tasks-03 §1bis.5`).
- Tests XSS simétricos en todas las vistas nuevas: `clientesView.js`, `proveedoresView.js`, `facturasView.js` (todos los submódulos), `historialView.js`, y las columnas "Valor Total" e "Ingresos" de `productosView.js` y `categoriasView.js` (N-2).
- Test transversal de `expiredCleanupWarning` por vista (`tasks-03 §5bis`, H-7).

### 8.4 Test de limpieza (`environment.test.js`)

Recorrer las fuentes de producción y comprobar que `dashboard.js` ya no contiene `clientesData`, `proveedoresData`, `facturasEntradaData`, `facturasVentaData`, `facturasServiciosData`, `facturasInsumosData`, `facturasGarantiaData`, `facturasPerdidasData`, `facturaTipoInfo`, `obtenerFacturasCombinadas`, ni los `next…Seq` de facturas. Comprobar que no queda ningún `onclick`/`oninput`/`onchange`/`onsubmit` inline de las secciones migradas. Comprobar que las llamadas a `crearNotificacion`/`notificarFactura` desde facturas y pérdidas se eliminaron.

### 8.5 Verificación manual

Con datos reales, crear un cliente, un proveedor, una factura de entrada con adjunto, una factura de venta, una de servicio, una de garantía, una de insumos, una de pérdida, y consultar el historial y exportar. Comprobar el detalle de cada factura. Probar la eliminación y cancelación con lotes consumidos. Probar el escenario H-A1 (compra cancelada + venta cancelada + eliminar compra).

---

## 9. Complexity Tracking (desviaciones de `Constitution.md`)

| Desviación | Razón | Alternativa más simple considerada |
|---|---|---|
| `products.quantityMilli` y `products.costCents` como **cachés** sincronizadas con los lotes, sin poder expresar la igualdad como `CHECK` de SQLite (`Constitution.md` §4) | Los lotes son la fuente de verdad, pero las cachés simplifican las consultas de `spec-02` y evitan recalcular el promedio en cada lectura. La igualdad no se puede expresar declarativamente. Se garantiza con sincronización en código + tests de invariante. | Guardar solo los lotes y calcular `quantityMilli`/`costCents` en cada consulta — descartado: complica las consultas de `spec-02` y obliga a reescribir DTOs existentes. |
| **Tope de stock por producto (H-A3)** sin `CHECK`: 999.999.999 milésimas | La suma cruza filas; no se puede expresar declarativamente. Se valida en código con `validate_stock_limit` antes de cada operación que aumente stock. | Bajar el tope por lote a un valor que, sumado, no supere `stockAfterMilli` — descartado: reduce artificialmente la capacidad de cada lote. |
| Los PDF adjuntos se guardan en `app_data_dir/attachments/` (fuera de SQLite), contra `Constitution.md` §4 | Son archivos binarios grandes (hasta 5 MB) que no aportan valor a las consultas. SQLite recomienda no almacenar blobs grandes. La información **mostrada** al usuario (nombre, tamaño, fecha) sigue viniendo de SQLite. El respaldo (PEN-15) copiará la base y el directorio. | Guardar el PDF como `BLOB` en `invoices` — descartado: la base crecería desproporcionadamente y las copias de seguridad serían pesadas. |
| Eliminación de `pricing.rs::weighted_average_cost_cents` (P-10 de `spec-02`) | El costo del producto se calcula sobre lotes (LT-03) y P-10 no tiene consumidor. Mantenerla sin uso violaría `Constitution.md` §2.II. Se revierte la desviación previa. | Dejarla con `#[allow(dead_code)]` — descartado: viola §2.II. |
| Módulo `export.rs` con `zip` 9 (sin features) + SpreadsheetML propio y `printpdf` 0.7 | La spec (HP-03) exige exportar a `.xlsx` y `.pdf`. Generar en el frontend con jsPDF o SheetJS duplicaría la lógica y enviaría miles de filas por IPC. Los crates son Rust puro, sin runtime async, y solo se usan en un comando. | CSV con extensión `.xlsx` — descartado: Excel lo abre con aviso y pierde formato. |
| **`rust-version` de 1.77.2 a 1.88** (2026-10-16, decisión del dueño) | `rust_xlsxwriter` no se puede instalar (todas sus versiones dependen de `zip` retirado de crates.io) y `zip` 9, la única versión vigente, exige Rust 1.88. El toolchain instalado es 1.98. | Escribir también el ZIP a mano para conservar 1.77.2 — descartado: ~200 líneas de formato binario propio que mantener. |
| `.prettierignore` (2026-10-16) | `npx prettier --write .` (Agents.md §7) reescribiría las librerías minificadas de `src/js/libs/`, los archivos generados por Tauri/npm, la documentación `*.md` (Agents.md §3 limita Prettier a HTML/CSS/JS) y `auth.css`/`dashboard.css`, que no se modifican sin necesidad (Agents.md §3 y §9). | Recordar no ejecutar Prettier sobre esas rutas — descartado: es fácil de olvidar y el comando de Agents.md es global. |
| Dependencia `uuid` (feature `v4`) | Ya es dependencia transitiva de `tauri`. Se declara directa para generar nombres de archivo únicos. La alternativa (`getrandom` + formateo manual) es más código propio para el mismo resultado. | Formatear 16 bytes a mano — descartado: más código sin beneficio. |
| Caché del import dinámico de `formatters.js` en `dashboard.js` (`formatearFecha` como wrapper) | `dashboard.js` es un script clásico y no puede importar módulos ES de forma estática. El wrapper con import dinámico cacheado da una sola fuente de verdad sin romper el archivo. | Cargar `formatters.js` como script clásico con global — descartado: rompe la disciplina "solo módulos ES para código nuevo" de `plan-01` §6. |
| Composición del total preliminar en el frontend al armar una factura, frente a `Constitution.md` §2.I | Los valores que el usuario tipea (cantidad, precio, costo) son entradas del usuario, no valores derivados de la base. El backend recalcula el total autoritativo al guardar. **No existe un comando `previewInvoiceTotals`.** | Añadir `previewInvoiceTotals` — descartado por el dueño: no aporta sobre un cálculo que se hace con las entradas del usuario. |
| Salida segura (A-01) y reescritura con `textContent` de las 12 vistas de facturas | El código actual interpola datos del usuario en `innerHTML` sin escapar. La spec §9 exige "todo dato del usuario se muestra como texto, nunca como HTML". Sin CSP (PEN-12). **Esfuerzo estimado a levantar en `tasks-03.md` §0 (reconocimiento de `innerHTML`, O-2); se documenta en `tasks-03 §4.5.7` como pre-tarea.** | Activar CSP estricta — descartado: rompería los ~226 manejadores inline del HTML heredado, incluidas secciones sin spec. |
| Sin CSP (`app.security.csp` sigue en `null`) | Ver `spec-02` §5 y PEN-12. El proyecto es local, offline y de un solo usuario. | Activar CSP — descartado por el mismo motivo. |
| La purga es **best-effort** en lecturas y comandos con `productId` (heredado de `plan-02` §3.6) | El usuario nunca ve un vencido y el fallo queda registrado. La app sigue usable. | Abortar el comando — descartado por frágil. |
| `printpdf` con Helvetica integrada (Latin-1/WinAnsi) | Cubre todo el español (`á é í ó ú ü ñ ¿ ¡ €`). El proyecto es 100 % en español y no necesita otros alfabetos. Si en el futuro los necesita, se incrusta una fuente TrueType (PEN-17 de `spec-03` §10). | Incrustar una fuente TrueType desde el inicio — descartado: añade peso al binario sin necesidad real. |
| **Registro con throttle (H-A2):** los códigos nuevos con estado (`stockInsuficiente`, `loteEnUso`, `attachmentInvalid`, `stockLimitExceeded`) se registran con un límite de 60 s por `(code, userId)`, frente a `Constitution.md` §2.VI | Aunque no son rechazos de formato, el usuario puede repetirlos en bucle. El throttle mantiene el log útil sin la excepción completa. | No registrarlos — descartado (se pierde el diagnóstico). Registrarlos sin throttle — descartado (un usuario en bucle llenaría el log). |
| **Cambio de costo sin cambio de cantidad no genera movimiento (H-A4)** | Reflejar cada cambio de costo exigiría recalcular el Ajuste al coste real y complicaría el cálculo. El precio ya quedó guardado en la factura. | Emitir Ajuste siempre que el `unitPriceCents` cambie — descartado por complejidad y ruido. |

No hay desviación del registro de errores con respecto a `Constitution.md` v1.2.0 §2.VI: la excepción de los rechazos esperados sigue aplicando a `validationFailed` e `invalidCredentials`, y los códigos nuevos no son rechazos "que se repiten sin límite" gracias al throttle. Ninguna dependencia nueva sin justificar. La convención de nombres, la unicidad con columnas normalizadas y las reglas en el esquema cumplen `Constitution.md` §3 y §4 sin requerir fila de desviación adicional.

---

## 10. Decisiones D-01 a D-27, trazabilidad y límites conocidos

### 10.1 Tabla de decisiones D-01 a D-27

| ID | Decisión | Sección del plan |
|---|---|---|
| D-01 | Estados como códigos neutrales (`pending`/`completed`/`cancelled`); la etiqueta visible depende del tipo y la pone el frontend. | §2, §3.7 |
| D-02 | Un solo texto libre por factura, `notes`, con etiqueta "Observaciones" o "Descripción" según el tipo. | §2 |
| D-03 | El total no se guarda; el subtotal de la línea sí. | §2 |
| D-04 | **No existe `previewInvoiceTotals`.** La vista previa del total la compone el frontend. | §5, §9 |
| D-05 | **Orden del reemplazo de adjunto:** escribir nuevo → `UPDATE` en transacción → `commit` → borrar viejo. | §3.8 |
| D-06 | `listInvoices` con `type` opcional cubre las 6 listas por tipo y la general. | §4 |
| D-07 | Cancelar una factura `completed` conserva `paidAt` y `paymentMethod`. | §2, §3.7 |
| D-08 | `InvoiceListItem` incluye `associatedInvoice` solo para GG y GI. | §4 |
| D-09 | El listado de proveedores para PV-07 usa `listSuppliers`. | §4 |
| D-10 | Una factura no puede tener más de 999.999,999 unidades en un lote. | §2 |
| D-11 | Los formularios de factura no tienen descuentos ni impuestos. | §3.2 de la spec |
| D-12 | El estado inicial de una factura nueva es `pending`. | §3.7 |
| D-13 | `changeInvoiceStatus` recibe `status` y, opcionalmente, `paymentMethod`. | §4 |
| D-14 | Los `<select>` de estado usan los códigos internos como `value`. | §7.3, §7.4 |
| D-15 | `#factura-general-tipo` usa los códigos de prefijo como `value`. | §7.6 |
| D-16 | Índices solo en tablas de crecimiento ilimitado y para las consultas de §4. | §2 |
| D-17 | Inmutabilidad de `productMovements` y de `invoiceCounters` por triggers. | §2 |
| D-18 | `printpdf` con Helvetica integrada (Latin-1/WinAnsi). | §3.10, §9 |
| D-19 | La fecha de alta del cliente se calcula en el backend al crear. | §5.1 |
| D-20 | El historial de facturas del cliente/proveedor viene dentro de `getClientDetail`/`getSupplierDetail`. | §4, §5 |
| D-21 | `historialView.js` es dueño único de la pestaña Historial. | §5.2 |
| D-22 | `dashboard.js` despacha un evento `page:change`. | §5.5 |
| D-23 | Los manejadores inline de las secciones migradas se eliminan. | §5.6, §7.1 |
| D-24 | `formatearFecha` es un wrapper del helper `formatDate` con import dinámico cacheado. | §5.5, §7.1 |
| D-25 | Los módulos de `spec-03` se añaden después de los de `spec-02`. | §5.13 |
| D-26 | `#factura-venta-lineas-body` recibe el botón "Agregar Fila" y el botón `+`. | §7.4 |
| D-27 | El botón "Editar" del detalle de una factura se oculta cuando el estado no permite la edición. | §7.3 |

### 10.2 Trazabilidad de los hallazgos de la auditoría QA (primera ronda)

| ID | Hallazgo | Resolución | Sección |
|---|---|---|---|
| H-1 | Conteo incorrecto de comandos (decía 41; el correcto es 76). | Corregido a 76. | §4.1, §8.1 |
| H-2 | Referencia incorrecta "§4 de la spec" (la tabla está en §5.5). | Corregido a §5.5 de `spec-03.md`. | §4.1 |
| H-3 | Faltaba la tabla de comandos en el plan. | Añadido el índice de 35 comandos en §4.1. | §4.1 |
| H-4 | Faltaban tablas de trazabilidad en §10. | Añadidas las tablas D-01 a D-27 y de hallazgos QA. | §10.1, §10.2 |
| H-5 | No se listaban los cambios a `spec-02`. | Añadida §5.15 (backend) y §5.16 (frontend). | §5.15, §5.16 |
| H-6 | §3.11 era casi vacía. | Eliminada; su contenido fusionado con §3.11 (registro de errores). | §3.11 |
| H-7 | No se mencionaban los cambios a `productosView.js`. | Añadido en §5.16. | §5.16 |
| H-8 | No se mencionaban los cambios a `categoriasView.js`. | Añadido en §5.16. | §5.16 |
| H-9 | Un solo mensaje de `invalidState` de edición. | Desglosado en tres mensajes específicos. | §4.3 |
| H-10 | No se decidía cómo conviven las suites de test. | Decidido: `mod tests_spec01/02/03`. | §8.1 |
| H-11 | Faltaban tablas de trazabilidad. | Cubierto por H-4. | §10.1, §10.2 |
| H-12 | `printpdf` con alfabetos no latinos sin cerrar. | Convertido en PEN-17. | §9, spec §10 |
| H-13 | Duplicado de H-1. | — | — |

### 10.3 Límites aceptados (registrados en `spec-03` §10)

- **Crecimiento de `productMovements`:** no se purga. Los filtros por rango de fechas permiten consultar cualquier período.
- **PDFs en disco:** el respaldo (PEN-15) tendrá que copiar la base y el directorio de adjuntos.
- **Eliminar un producto con consumos activos:** la devolución no se puede aplicar; se registra una Reversión con cantidad 0 y se avisa al usuario.
- **Adjuntos huérfanos:** el barrido del arranque es best-effort; si falla, los archivos quedan hasta el próximo arranque.
- **Precio de venta máximo:** costo $999.999,99 y 1000 % dan ≈ $11.000.000. Cabe en `i64`.
- **Productos vencidos pueden quedar más de 60 días** si ningún comando los toca; nunca se cuentan, muestran ni operan.
- **Reloj del sistema:** cambiar la fecha del equipo puede adelantar o retrasar la purga.
- **Sin CSP** (PEN-12).
- **Alfabetos no latinos en PDF** (PEN-17): cubierta Latin-1/WinAnsi; si el proyecto cambia de contexto, se incrusta una fuente TrueType en `export.rs`.
- **Cambio de costo por re-FIFO sin movimiento (H-A4):** editar una salida y no cambiar la cantidad de una línea puede darle un costo real distinto por re-FIFO. La factura actualiza su `unitPriceCents`, pero el historial no registra el cambio silencioso.

### 10.4 Trazabilidad de la auditoría QA posterior (H-1 a H-11, O-1 a O-4)

| ID | Hallazgo | Archivos afectados | Resolución |
|---|---|---|---|
| H-1 (nuevo) | `tasks-03.md` con mojibake (UTF-8 mal decodificado). | `tasks-03.md` | Re-guardar en UTF-8. |
| H-2 (nuevo) | Sin tareas explícitas para modificar los tests de `spec-02` que dejan de ser ciertos globalmente. | `tasks-03.md` | Añadida tarea en §3.5. |
| H-3 (nuevo) | Sin tareas de test de los triggers de inmutabilidad de `productMovements` y de `invoiceCounters`. | `tasks-03.md` | Nueva §1.5, sub-secciones §1.5.3, §1.5.6 y §1.5.8. |
| H-4 (nuevo) | §1 bis.4 de `tasks-03.md` era solo verificación, no implementación del orden de `<script>`. | `tasks-03.md` | Convertida en tarea de implementación + test. |
| H-5 (nuevo) | `plan-03 §5.5` decía "10 comandos" para create/update; son 11. | `plan-03.md`, `spec-03.md` | Corregido en `plan-03 §4.1` y en `spec-03 §5.5`. |
| H-6 (nuevo) | Sin test del evento `page:change`. | `tasks-03.md` | Añadido en §1bis.5 (nuevo) y referenciado desde §2.3.2. |
| H-7 (nuevo) | Sin tests sistemáticos de `expiredCleanupWarning` por vista. | `tasks-03.md` | Nueva §5 bis. |
| H-8 (nuevo) | `plan-03 §1` listaba las verificaciones previas como pendientes, sin espejo explícito con `tasks-03 §0`. | `plan-03.md` | Convertidas en checkboxes con espejo. |
| H-9 (nuevo) | Sin test de purga fallida en `getProductHistory`. | `tasks-03.md` | Añadido en §3.3.1. |
| H-10 (nuevo) | Sin test de descarga con archivo corrupto. | `tasks-03.md` | Añadido en §3.2.3. |
| H-11 (nuevo) | **`tasks-03.md` no tenía tarea de implementación del esquema de `plan-03 §2`.** | `tasks-03.md` | Nueva §1.5 "Esquema". |
| O-1 | Sin test de regresión de `selling_price_cents` tras eliminar `weighted_average_cost_cents`. | `tasks-03.md` | Añadido en §3.5. |
| O-2 | Sin reconocimiento de `innerHTML` para dimensionar la reescritura. | `plan-03.md`, `tasks-03.md` | Añadido en `plan-03 §1` y `tasks-03 §0`, y referenciado en §9. |
| O-3 | Trigger de inmutabilidad no mapeado a `internal`. | `plan-03.md` | Añadido en la fila de `internal` en §4.3. |
| O-4 | PEN-17 no estaba en `pendientes.md`. | `pendientes.md` | Añadida la fila. |

### 10.5 Trazabilidad de la segunda revisión QA posterior (N-1 a N-4, O-B)

| ID | Hallazgo | Archivos afectados | Resolución |
|---|---|---|---|
| N-1 | `invoices.updatedAt` sin test explícito de refresco tras escrituras. | `plan-03.md`, `tasks-03.md` | Nota en `plan-03 §2` y §3.8; tests añadidos en `tasks-03 §3.1.3`, §3.2.2, §3.2.3 y §4.2. |
| N-2 | Tests XSS no simétricos por vista. | `tasks-03.md` | Añadidos en `tasks-03 §2.3.3` (proveedores), §3.7.2 (historial) y §3.8 (productos y categorías). |
| N-3 | Test del evento `page:change` ubicado en la sección de clientes. | `tasks-03.md` | Movido a `tasks-03 §1bis.5` (nuevo, infraestructura compartida); `§2.3.2` solo verifica que `clientesView.js` lo escucha. |
| N-4 | Sin test de `UPDATE` mixto en `productMovementsImmutable`. | `tasks-03.md` | Añadido en `tasks-03 §1.5.6`. |
| O-B | `saveInvoiceAttachment` y `updatedAt`: ¿lo actualiza? | `plan-03.md` | Confirmado: sí, lo actualiza. Nota en `plan-03 §3.8`; test en `tasks-03 §3.2.3`. |
| C-7 | `listHistoryProducts` sin purga: ¿decisión o hueco? | `plan-03.md`, `spec-03.md` | Confirmado como decisión de diseño. Documentado en `plan-03 §3.9` y `spec-03 §7.6`. |

### 10.6 Trazabilidad de la auditoría de inventario (H-A1 a H-A4)

| ID | Hallazgo | Archivos afectados | Resolución |
|---|---|---|---|
| H-A1 | Eliminar una compra Cancelada con lotes restaurados perdía stock silenciosamente por el `ON DELETE CASCADE`. | `spec-03.md`, `plan-03.md`, `tasks-03.md` | Regla en `spec-03 FE-05`, §7.6 y §14; helper `reverse_lots_of_invoice` en `plan-03 §3.2`; tests en `tasks-03 §3.1.4`, §1.9 y §6. |
| H-A2 | Sin política clara de registro para `stockInsuficiente` y rechazos similares; inconsistencia entre `spec-03 §8.1` y `plan-03 §4.3`. | `spec-03.md`, `plan-03.md`, `tasks-03.md` | Throttle de 60 s por `(code, userId)` en memoria; solo los códigos nuevos con estado; `validationFailed` sigue sin registrarse. Política en `plan-03 §3.11` y §4.3; tests en `tasks-03 §1.4` y §6. |
| H-A3 | Sin tope de stock acumulado por producto. | `spec-03.md`, `plan-03.md`, `tasks-03.md` | Tope de 999.999.999 milésimas; código `stockLimitExceeded`; helper `validate_stock_limit` en `plan-03 §3.2`; tests en `tasks-03 §1.9`, §3.2.1, §3.2.2 y §6. |
| H-A4 | `Ajuste` ambiguo entre costo y cantidad; contradicción entre `FS-01` y `HP-01`. | `spec-03.md`, `plan-03.md`, `tasks-03.md` | `Ajuste` solo por cambio de cantidad. `spec-03 HP-01`, `S-24` y §14 reescritos; límite aceptado en `plan-03 §10.3`; test en `tasks-03 §4.2`. |

---

### 10.7 Huecos de la interfaz cerrados en task-03 §7 (2026-10-28)

Decisiones del dueño al implementar §7 (no añaden requisitos):

- **"Agregar Fila" y el "+" del buscador de producto se eliminan** de los cinco formularios (§7.1). Elegir un producto en el combobox agrega su línea, así que no existen líneas sin `productId`.
- **`purpose` en `searchProductsForInvoice`** en lugar de filtrar en el frontend (§7.2): la regla de qué productos ofrece cada formulario vive en Rust.
- **PV-07 y el recuento al eliminar (CL-05, PV-05) reutilizan `get*Detail`** (§7.5, §7.8): el historial de compras sale de `getSupplierDetail().invoiceHistory`, con rango de fechas y orden en el frontend; el recuento es `invoiceHistory.length` (todas las facturas conservan su copia, también las canceladas). Sin cambios en Rust.
- **`facturasView.js`** pasa a un submódulo genérico (`createInvoiceSubmodule`) para entrada, venta, servicio, garantía e insumos: listado con filtros, formulario de alta y edición, detalle con cambio de estado. La pérdida y la lista general siguen aparte. Las transiciones que ofrece el detalle reflejan `state.rs`; el backend las vuelve a validar.
- **Alta rápida** (§7.4): `components/quickContactDialog.js`, un modal con los campos de Contactos. El combobox ofrece "Crear nuevo…" solo sin coincidencias (`addOnlyWhenEmpty`), como pide spec §7.1.
- **`combobox.js`** gana `isOptionDisabled` (opción visible no elegible, "Agotado") y `addOnlyWhenEmpty`. Ambos son opcionales; el comportamiento por defecto no cambia.
- **Cambios de HTML necesarios** (Constitution §2.V): filtros de estado y fechas con `<label>` en cada listado; encabezado del historial de compras del proveedor (Fecha, No. Factura, Estado, Total, Acciones; las columnas anteriores no tenían datos); la sección "Facturas Asociadas" del producto pasa a un listado; `id` en los formularios de clientes y proveedores y en sus botones "Cancelar". Sin ese `id`, `clientesView.js` y `proveedoresView.js` no enlazaban el envío y guardar recargaba la página. Además, esas dos vistas enlazan sus listeners una sola vez (antes cada `page:change` los duplicaba y un envío habría creado el contacto dos veces).
- **Garantía e insumos** muestran "Se calcula al guardar" como total preliminar: van a costo real (FIFO) y el frontend no lo conoce antes de guardar.

## 11. Siguiente paso

Plan confirmado. `tasks-03.md` está redactado en las **4 fases de la spec §3.3**, con la granularidad de `tasks-01.md`/`tasks-02.md`:

1. **Fase 1 — Contactos:** clientes y proveedores (sin indicadores que dependan de facturas). Comandos: los 12 de `clients.rs` y `suppliers.rs`.
2. **Fase 2 — Lotes, compras e historial:** modelo de lotes, facturas de entrada, cambios a `spec-02` (SP-01, SP-02, SP-04), historial (registro y consulta). Comandos: los 17 de `invoices.rs` relacionados con entradas, los 2 de ampliación de `products.rs`, y `getProductHistory`, `listHistoryProducts` de `history.rs`.
3. **Fase 3 — Salidas:** venta, servicio, garantía, insumos, pérdidas, lista general, indicadores de contactos, "Valor Total" e "Ingresos". Comandos: los 5 comandos de creación de salidas + `changeInvoiceStatus` + `deleteInvoice` + `getClientDetail`/`getSupplierDetail` (los indicadores se activan aquí).
4. **Fase 4 — Exportación del historial:** `exportHistoryExcel` y `exportHistoryPdf`.

Cada fase tiene su propia matriz de autorización (parcial, se completa al final), sus tests de Rust y sus tests de UI. La verificación de contrato `invoke ↔ generate_handler!` de **76 comandos** y el test de limpieza de `environment.test.js` se hacen al cierre de la fase 4.

Verificaciones previas de §1 y §0 de `tasks-03.md` antes de empezar cualquier tarea de código.