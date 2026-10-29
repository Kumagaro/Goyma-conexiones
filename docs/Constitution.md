# Constitution — Goyma Conexiones

**Versión:** 1.3.0
**Ratificada:** 2026-09-20
**Última modificación:** 2026-10-28

Este documento define los principios no negociables del proyecto Goyma Conexiones. Toda especificación (`spec.md`), plan técnico (`plan.md`) y lista de tareas (`tasks.md`) dentro de `specs/` debe cumplir estos principios. Cualquier violación debe justificarse explícitamente en el plan correspondiente (sección "Complexity Tracking") o la spec debe rediseñarse.

---

## 1. Naturaleza del proyecto

Goyma Conexiones es una aplicación de **escritorio, 100% local y offline**, construida con **Tauri (backend en Rust + frontend en HTML/CSS/JS vanilla, sin frameworks como React o Vue)**. No existe servidor remoto. La persistencia se realiza en una única base de datos **SQLite local**.

La aplicación gestiona: inventario, ventas, clientes, proveedores, facturas, categorías, unidades y usuarios, con autenticación local y dos roles: **Administrador** y **Contador**.

Está dirigida exclusivamente a usuarios en **Cuba** (esto es una decisión de producto explícita, no un dato de prueba — por ejemplo, la validación de teléfono usa el prefijo `+53`).

Solo puede haber **una sesión de usuario activa a la vez** (no hay concurrencia multiusuario en tiempo real), aunque la base de datos puede contener múltiples usuarios registrados.

El proyecto es desarrollado por **un único desarrollador**, apoyado por agentes de IA. Las reglas de este documento están calibradas para ese contexto: priorizan claridad y mantenibilidad a largo plazo por una sola persona, no coordinación de equipo.

---

## 2. Principios fundamentales

### I. Integridad de datos de extremo a extremo (NO NEGOCIABLE)

Toda información mostrada en el frontend **debe** originarse en la base de datos SQLite, pasando siempre por el backend de Rust (comandos Tauri). Está **prohibido**:
- Mostrar datos hardcodeados, simulados (mock) o de ejemplo en el frontend de producción.
- Que el frontend escriba directamente a la base de datos, saltándose el backend.

Toda entrada de usuario capturada en la interfaz **debe** llegar al backend, validarse allí (no solo en el cliente) y procesarse correctamente antes de persistirse. La validación en el frontend (HTML5, JS) es solo una capa de UX; la validación real y autoritativa vive en Rust.

### II. Simplicidad sobre abstracción

El código debe ser **directo, efectivo y eficiente**. Se prohíben abstracciones, patrones de diseño o capas arquitectónicas que no resuelvan un problema real y presente del proyecto. Ante dos soluciones que cumplen el mismo requisito, se elige la más simple de leer y mantener por una sola persona.

No se introduce una librería, framework o dependencia nueva sin que su necesidad esté justificada en el plan técnico de la feature correspondiente.

### III. Test-first honesto (NO NEGOCIABLE)

Se sigue una disciplina de test-first: los tests se escriben antes o junto con el diseño de la funcionalidad, y deben fallar antes de implementar el código que los satisface.

Sin embargo, el objetivo del código **nunca** es "pasar el test". El código debe resolver el problema real de forma completa y correcta; el test existe únicamente para **validar** que ese comportamiento correcto se mantiene, no para definir el mínimo aceptable. Está prohibido escribir implementaciones que hagan trampa para satisfacer un test (valores hardcodeados, casos especiales artificiales, lógica incompleta que "coincidentemente" pasa).

### IV. Offline-first absoluto

El proyecto no depende de ningún servicio de red, API externa, servidor propio o de terceros para funcionar. Toda funcionalidad —incluida la recuperación de contraseña, que verifica identidad (usuario + teléfono + carnet) contra la base de datos local— debe operar sin conexión a internet.

### V. Consistencia visual e integración estricta

No se modifica el estilo visual, la estructura HTML/CSS existente ni el comportamiento ya definido de un componente **a menos que sea estrictamente necesario** para cumplir el requisito de la feature en curso. Todo cambio visual debe:
- Mantener el sistema de diseño actual (colores, tipografía, espaciados, comportamiento responsive ya definidos en `auth.css` / `dashboard.css`).
- Documentarse explícitamente en el plan si implica alterar un componente compartido.

### VI. Manejo de errores responsable

El usuario final siempre ve un **mensaje de error claro y en español**, apropiado al contexto: **específico** cuando necesita corregir algo (qué dato está duplicado o qué regla se incumple) y **genérico** ante errores internos o de autenticación; nunca detalles técnicos, trazas de pila ni mensajes de Rust/SQLite crudos.

Todo error **inesperado o de sistema** —sin importar su severidad— se registra en un **log local detallado y 100% claro**, con suficiente contexto (módulo, operación, causa) para poder depurarlo en producción sin acceso al código en ese momento. **Excepción acotada:** no se registran los rechazos esperados de una acción del usuario que pueden repetirse sin límite y no indican un fallo del sistema (credenciales incorrectas y datos que no pasan la validación), para que no tapen los errores reales. Ningún registro contiene contraseñas, hashes, datos personales ni valores ingresados por el usuario.

---

## 3. Convenciones de código

- **Idioma de identificadores** (variables, funciones, tipos, comandos Tauri, nombres de tablas/columnas): **inglés**.
- **Convención de nombres**: `camelCase` en JavaScript y en los nombres de tablas/columnas de SQLite; `snake_case` idiomático en Rust (funciones, variables, campos de structs, comandos Tauri). La conversión entre ambos mundos es mecánica y no se hace a mano: (1) Tauri convierte automáticamente los **argumentos** de un comando de `camelCase` (JS) a `snake_case` (Rust); (2) los structs que Rust devuelve al frontend llevan `#[serde(rename_all = "camelCase")]` para que JS reciba `camelCase`; (3) el nombre del comando en `invoke()` es el de la función Rust (`snake_case`), y solo se escribe dentro de `services/*.js`, cuyos wrappers exponen el nombre `camelCase` al resto del frontend. No se usa `#[allow(non_snake_case)]`: el código Rust debe compilar sin warnings con las reglas estándar del lenguaje.
- **Comentarios de código**: en **español**, claros y explicando el "por qué", no el "qué" obvio.
- **Interfaz de usuario (UI)**: todo texto visible para el usuario final, en **español**.
- **Contraseñas**: nunca se almacenan en texto plano; se hashean con **bcrypt** antes de persistirse.
- **Formateo automático**: Prettier para JS/HTML/CSS; `rustfmt` + `clippy` para Rust. El código debe pasar el formateador y no generar warnings de `clippy` antes de considerarse terminado.

---

## 4. Base de datos

- Una **única base de datos SQLite** para todo el proyecto.
- **El esquema está versionado** (desde `cierre-del-mvp`): la versión vive en la propia base (`PRAGMA user_version`) y todo cambio de esquema es una **migración** numerada, con su test, que se aplica en orden al arrancar dentro de su propia transacción. La versión 1 es el esquema de las tres primeras specs. Si una migración falla, la base queda en la versión anterior y la aplicación no arranca a medias; una base de una versión mayor que la conocida no se abre. No se edita el esquema base para añadir cambios: se escribe una migración nueva.
- Todo acceso a la base de datos ocurre exclusivamente desde el backend de Rust.
- Toda regla de negocio que el esquema pueda expresar (`UNIQUE`, `CHECK`, `COLLATE`, claves foráneas) se declara **en el esquema**, no solo en el código Rust: la base de datos es la última línea de defensa de la integridad (§2.I) y no debe depender de que cada comando recuerde aplicar la regla. Ejemplo: la unicidad sin distinguir mayúsculas de `username` usa `COLLATE NOCASE` en la columna, además de normalizar en el código.
- **Alcance de `COLLATE NOCASE`:** solo pliega mayúsculas y minúsculas ASCII; no equipara letras con acento (`Á` y `á` son distintas) ni `Ñ` con `ñ`. Por eso se usa únicamente en campos ASCII como `username`. Para texto con acentos (por ejemplo nombres de categorías, unidades o productos), la unicidad se declara con `UNIQUE` (o índice parcial cuando aplique) sobre una columna normalizada que el código rellena con la misma función de normalización, en lugar de depender de `COLLATE NOCASE`.

---

## 5. Flujo de trabajo (Spec-Driven Development)

Toda funcionalidad nueva o cambio significativo sigue el flujo:

```
specs/<nombre-feature>/spec.md   → Qué se construye y por qué (sin detalles de implementación)
specs/<nombre-feature>/plan.md   → Cómo se construye (arquitectura, decisiones técnicas)
specs/<nombre-feature>/tasks.md  → Lista de tareas concretas y ordenadas
```

No se escribe código de una feature nueva sin que exista primero su `spec.md` correspondiente, salvo correcciones triviales (typos, ajustes de estilo menores) que no alteran comportamiento.

- **Control de versiones**: Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`). Trabajo directo sobre `main`; se usan ramas solo para cambios grandes o experimentales que puedan dejar el proyecto en estado inestable.

---

## 6. Gobernanza

Esta constitución tiene precedencia sobre cualquier otra práctica o preferencia ad-hoc. Toda spec o plan que la contradiga debe:
1. Documentar explícitamente la razón de la desviación, o
2. Ser rediseñado para cumplirla.

Las modificaciones a este documento requieren actualizar el número de versión (semver: MAYOR.MENOR.PARCHE) y la fecha de "Última modificación", registrando qué cambió y por qué.

---

## 7. Historial de versiones

- **1.3.0 (2026-10-28):** §4 sustituye "no se requieren migraciones versionadas" por el versionado del esquema con migraciones numeradas. Motivo: `cierre-del-mvp` introduce el respaldo y la restauración de toda la información (PEN-15), que deben sobrevivir a una actualización de la aplicación; sin migraciones, una copia de una versión anterior no se podría usar. Cambia una regla de §4, no un principio de fondo.
- **1.2.0 (2026-09-30):** §2.VI acota el registro de errores a los inesperados o de sistema y fija la excepción de los rechazos esperados de una acción del usuario (credenciales incorrectas y datos que no pasan la validación); añade que ningún registro contiene contraseñas, hashes, datos personales ni valores ingresados. También aclara que el mensaje al usuario es específico cuando debe corregir algo y genérico ante errores internos o de autenticación (antes decía "siempre genérico"). Motivo: la regla anterior ("todo error se registra") no coincidía con la política real de las specs `autenticacion-y-usuarios` e `inventario-productos` y obligaba a documentar la misma desviación en cada `plan.md`; la redacción de los mensajes tampoco reflejaba que las specs muestran mensajes específicos de validación. Cambia el alcance de una regla, no un principio de fondo.
- **1.1.1 (2026-09-30):** se aclara en §4 que `COLLATE NOCASE` solo pliega ASCII y que, para texto con acentos, la unicidad se declara con `UNIQUE` (o índice parcial) sobre una columna normalizada. Motivo: la regla de §4 podía leerse como suficiente para cualquier texto, pero no detecta duplicados como `Café` y `cafe`. No cambia ningún principio ni la regla de `username`.
- **1.1.0 (2026-09-23):** se reemplaza la regla "`camelCase` también en Rust" por `snake_case` idiomático en Rust con conversión automática de Tauri/serde hacia `camelCase` en JS (§3). Motivo: la regla anterior era incompatible con "sin warnings" (el lint `non_snake_case` de `rustc` es warn-by-default) y con el principio de simplicidad (§2.II), ya que Tauri resuelve la conversión de forma nativa. Se agrega a §4 que las reglas expresables en el esquema se declaran en el esquema.
- **1.0.0 (2026-09-20):** versión inicial.
