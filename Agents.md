# Agents.md — Goyma Conexiones

Guía operativa para cualquier agente de IA (Claude Code u otro) que trabaje en este repositorio. Este documento complementa a `Constitution.md`: la constitución define los **principios**, este archivo define **cómo ejecutarlos en la práctica**. Ante cualquier conflicto, `Constitution.md` tiene prioridad. *(Alineado con `Constitution.md` v1.3.0.)*

---

## 1. Resumen del proyecto

- **Nombre:** Goyma Conexiones — sistema de gestión empresarial (inventario, ventas, clientes, proveedores, facturas, categorías, unidades, usuarios).
- **Tipo:** Aplicación de escritorio 100% offline, sin servidor.
- **Stack:**
  - Backend: **Rust** vía **Tauri** (comandos Tauri, sin framework web adicional).
  - Frontend: **HTML + CSS + JavaScript vanilla** (sin React, Vue ni similares).
  - Base de datos: **SQLite local** (única BD para todo el proyecto).
  - Tests: Rust nativo (`#[cfg(test)]`) en el backend + **Vitest/jsdom** en el frontend (dependencia de desarrollo, justificada en `plan.md` de cada feature).
- **Estado actual:** las cuatro specs están implementadas en Rust/SQLite y frontend: Autenticación y Usuarios (`spec-01`), Inventario-Productos (`spec-02`), Contactos-Facturación-Historial (`spec-03`) y Cierre del MVP (`spec-04`: notificaciones, reservas, configuración, impresión, conversiones, dashboard, borradores, registro de actividad, CSV y respaldo). `spec-01`, `spec-03` y `spec-04` están **pendientes de la verificación manual en la app real** (`tauri:dev`/`tauri:build`, `task-04.md` §5.6); `spec-02` ya la tiene. Fuera del MVP queda lo indicado en `specs/pendientes.md`.
- **Usuarios:** exclusivos de Cuba (validaciones como teléfono `+53XXXXXXXX` son intencionales, no placeholders). Una sola sesión activa a la vez. Roles: `Administrador` y `Contador`.
- **Desarrollador:** una sola persona, apoyada por agentes de IA. No hay equipo que coordinar.

---

## 2. Antes de escribir código

1. **Revisa `Constitution.md`** si no lo has hecho en esta sesión — especialmente los principios de integridad de datos, simplicidad y no modificar estilos sin necesidad.
2. **Busca o crea la spec de la feature** en `specs/<nombre-feature>/`:
   - Si no existe `spec.md`, `plan.md` y `tasks.md` para lo que se pide, créalos primero (ver plantillas en la sección 6) y espera confirmación del usuario antes de implementar, salvo que él pida explícitamente saltar este paso para un cambio trivial.
   - Si ya existen, sigue las tareas de `tasks.md` en orden; no te desvíes del plan sin señalarlo.
3. **No asumas comportamiento del backend** que aún no existe: si una feature de frontend requiere datos reales, verifica si el comando Tauri correspondiente ya está implementado. Si no, decláralo como pendiente en la spec/tareas en vez de simular datos en el frontend (viola el principio de integridad de datos).

---

## 3. Reglas de código

| Aspecto | Regla |
|---|---|
| Identificadores (variables, funciones, tipos, columnas de BD, comandos Tauri) | Inglés |
| Convención de nombres | `camelCase` en JS y en columnas/tablas de SQLite; `snake_case` idiomático en Rust (funciones, variables, campos, comandos Tauri). Tauri convierte los argumentos de `invoke()` a `snake_case` automáticamente; los structs devueltos a JS llevan `#[serde(rename_all = "camelCase")]`; el nombre `snake_case` del comando solo aparece dentro de `services/*.js`. Nunca `#[allow(non_snake_case)]` |
| Reglas de esquema | Lo que el esquema puede garantizar (`UNIQUE`, `CHECK`, `COLLATE NOCASE` en `username`, FK) se declara en el esquema, no solo en código Rust. `COLLATE NOCASE` solo pliega ASCII: para texto con acentos se usa una columna normalizada con `UNIQUE` (o índice parcial) |
| Cambios de esquema | Desde `spec-04`, cada cambio es una migración numerada en `src-tauri/src/db/migrations.rs` (con su test), nunca una edición de `create_schema` (`Constitution.md` §4 v1.3.0) |
| Comentarios en el código | Español |
| Textos de interfaz (UI) | Español |
| Contraseñas | Nunca en texto plano; hash con `bcrypt` antes de guardar |
| Validación de datos | Doble: en el frontend para UX, y **obligatoria y autoritativa** en el backend Rust antes de persistir |
| Origen de datos en frontend | Siempre desde SQLite vía comando Tauri; nunca hardcodeado ni mock en código de producción |
| Formateo | `Prettier` para HTML/CSS/JS; `rustfmt` + `clippy` para Rust — el código debe quedar formateado y sin warnings antes de darse por terminado |
| Complejidad | Preferir siempre la solución más simple que cumpla el requisito; no introducir librerías o patrones nuevos sin justificarlo en `plan.md` |
| Estilos visuales | Toda interfaz sigue `specs/05-sistema-de-diseno/ui-spec.md`: colores, tamaños y radios salen de las variables de `design-system.css`, y los componentes nuevos van en `components.css`. No modificar estilos existentes sin necesidad directa de la feature en curso |
| Autorización de comandos protegidos | Toda mutación de `users` o `sessions` vive en una función que exige `&Actor`/`&AdminActor` en su firma; esos tipos solo se obtienen vía `require_session`/`require_admin` (constructores privados al módulo), nunca fabricados por un comando. Todo comando nuevo que necesite sesión o rol Administrador se añade a la matriz de tests de autorización de la feature correspondiente — ningún comando protegido queda sin su fila |

---

## 4. Testing

- Se sigue test-first: escribe (o esboza) el test antes de implementar la lógica que debe cumplir.
- El test valida que la funcionalidad **funcione de verdad**, no es el objetivo en sí. Nunca implementes código que solo "engañe" al test (valores hardcodeados para pasar un caso concreto, ramas artificiales, lógica incompleta).
- Los stubs de `invoke()` / `window.__TAURI__` (Vitest + jsdom) se permiten **solo dentro de los tests**, nunca en código de producción: no violan el principio de integridad de datos porque no llegan al frontend real.
- Si el backend de Rust aún no existe para una feature, y no es viable escribir un test de integración real, documenta explícitamente en `tasks.md` que el testing de esa parte queda pendiente hasta que el comando correspondiente esté implementado — no lo omitas en silencio.

---

## 5. Manejo de errores

- **Frontend / UI:** mostrar siempre un mensaje claro y en español, apropiado al tipo de error: específico cuando el usuario debe corregir algo (ej. "El teléfono ya está registrado") y genérico ante errores internos o de autenticación (ej. "No se pudo verificar tu identidad" en vez de un mensaje técnico de SQLite).
- **Backend:** todo error **inesperado o de sistema** debe registrarse en un log local con suficiente detalle (módulo, operación, causa, timestamp) para depurar en producción sin necesitar reproducir el bug en desarrollo. No se registran los rechazos esperados de una acción del usuario (`validationFailed`, `invalidCredentials`; ver `Constitution.md` §2.VI): los decide `AppError::should_log()`. Ningún registro contiene contraseñas, hashes, datos personales ni valores ingresados.
- Si falla el registro en la tabla `errorLogs` (o la BD no está disponible), el error se escribe en `errors.log` dentro del `app_data_dir` de Tauri, junto a la BD; si tampoco se puede escribir ahí, se ignora en silencio (registrar un error nunca debe provocar un panic).
- Nunca expongas al usuario final trazas de pila, mensajes de Rust/SQLite crudos, ni detalles internos de implementación.

---

## 6. Estructura de `specs/`

Cada feature vive en su propia carpeta:

```
specs/
  <nombre-feature>/
    spec.md    # Qué se construye, para quién, por qué. Sin detalles técnicos de implementación.
    plan.md    # Cómo se construye: decisiones técnicas, comandos Tauri necesarios, esquema de BD afectado, justificación de cualquier desviación de Constitution.md
    tasks.md   # Lista de tareas concretas, ordenadas y verificables (checkbox), derivadas del plan
```

Plantilla mínima de `tasks.md`:

```markdown
# Tareas — <nombre-feature>

- [ ] Test: <descripción del caso a validar>
- [ ] Implementación: <descripción>
- [ ] Verificación manual / UI
- [ ] Actualizar documentación si aplica
```

No marques una tarea como completa sin haber verificado que el código funciona de verdad (no solo que "compila" o que "el test pasa").

---

## 7. Comandos del proyecto

- **Desarrollo / build:** `npm run tauri:dev` y `npm run tauri:build` (Tauri v2, frontend estático en `src/`). Requiere Rust 1.88 o superior (`rust-version` en `src-tauri/Cargo.toml`).
- **Formateo:** `npx prettier --write .` para frontend (respeta `.prettierignore`: librerías minificadas, generados, `*.md`, `auth.css` y `dashboard.css`); `cargo fmt --manifest-path src-tauri/Cargo.toml` y `cargo clippy --manifest-path src-tauri/Cargo.toml` para Rust.
- **Tests:** `cargo test --manifest-path src-tauri/Cargo.toml` (backend) y `npm test` (frontend, Vitest + jsdom). Lint estricto: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.

---

## 8. Control de versiones

- **Convención de commits:** Conventional Commits — `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`.
- **Ramas:** trabajo directo en `main`. Se crean ramas solo para cambios grandes o experimentales que puedan dejar el proyecto inestable mientras se desarrollan.
- Cada commit debe corresponder idealmente a una tarea completa de `tasks.md`, no a cambios parciales o mezclados de varias features.

---

## 9. Qué NO hacer (resumen rápido)

- No mostrar datos falsos/hardcodeados en el frontend "para probar" y dejarlo así.
- No escribir lógica de backend que salte la validación solo para simplificar.
- No cambiar estilos de `auth.css` / `dashboard.css` u otros ya definidos sin necesidad real de la feature.
- No usar `camelCase` en identificadores de Rust (usar `snake_case`), ni `snake_case` en JS ni en columnas de SQLite (usar `camelCase`). No silenciar `non_snake_case` con `#[allow]`.
- No confiar solo en normalizar en código para invariantes que el esquema puede garantizar (ej. `username` único sin distinguir mayúsculas: `COLLATE NOCASE` + normalización). No usar `COLLATE NOCASE` para texto con acentos: no equipara `Á` con `á`; usar una columna normalizada con `UNIQUE`.
- No mezclar inglés en comentarios/UI ni español en nombres de variables/funciones.
- No implementar una feature sin su `spec.md` / `plan.md` / `tasks.md` correspondiente (salvo cambios triviales explícitamente autorizados por el usuario).
- No mostrar mensajes de error técnicos al usuario final.
- No asumir servicios externos o conexión a internet en ninguna funcionalidad.
- No escribir una mutación de `users` o `sessions` fuera de una función que exija `&Actor`/`&AdminActor` en su firma, ni fabricar esos tipos por fuera de `require_session`/`require_admin`. No añadir un comando protegido sin su fila en la matriz de tests de autorización.
