# Spec: Autenticación y Usuarios

- **Feature:** `autenticacion-y-usuarios`
- **Estado:** Confirmada / lista para implementar. Todas las rondas de auditoría (seguridad, contrato frontend-backend, orden y cobertura de tests, higiene documental) están cerradas y confirmadas por el dueño del proyecto, y la trazabilidad contra `Requisitos_del_sistema.docx` se cerró sin dejar ninguna verificación pendiente. Las diferencias respecto de los requisitos originales quedan registradas en la sección 10 ("Desviaciones respecto de `Requisitos_del_sistema.docx`").
- **Basada en:** `Requisitos_del_sistema.docx` (RF-001 a RF-009), `Constitution.md` v1.1.0, `Agents.md`, mockups `index.html`, `recovery.html`, `dashboard-unificado.html`
- **Siguiente paso:** `plan.md` y `tasks.md` están alineados con esta spec; no queda ninguna confirmación pendiente para empezar a implementar (`Agents.md` §2)

---

## 1. Resumen

Esta spec cubre los dos primeros módulos del sistema Goyma Conexiones: **Autenticación y Sesión** y **Usuarios**. Define cómo un usuario accede al sistema, cómo se mantiene o expira su sesión, cómo recupera el acceso si olvida su contraseña, y cómo se gestionan las cuentas de usuario (creación, listado, edición, eliminación y perfil propio) según el rol del usuario.

## 2. Objetivo / Por qué

El sistema requiere control de acceso porque maneja información sensible del negocio (inventario, ventas, clientes, finanzas). Solo dos roles pueden operar el sistema — **Administrador** y **Contador** — y debe garantizarse que:

- Solo personas autorizadas accedan al sistema.
- El Administrador tenga control total sobre las cuentas de usuario.
- El Contador pueda operar el sistema y gestionar su propia cuenta, sin visibilidad ni control sobre las cuentas de otros usuarios.
- El sistema nunca quede en un estado inconsistente (por ejemplo, sin ningún Administrador) por una acción de gestión de usuarios.

## 3. Alcance

### 3.1 Incluido en esta spec

- Inicio de sesión (RF-001).
- Restauración de contraseña vía verificación de identidad local (RF-002).
- Cierre de sesión manual (RF-003).
- Cierre de sesión automático por inactividad (RF-004).
- "Mantener sesión activa" y su persistencia entre reinicios de la app (RF-005).
- Creación de nuevo usuario por el Administrador (RF-006).
- Listado de usuarios, visible solo para el Administrador (RF-007).
- Ver perfil propio (RF-008).
- Cambiar contraseña desde el perfil propio (RF-009).
- **Extensión no contemplada en los RF originales, agregada y confirmada en rondas de preguntas:**
  - Edición de datos de otros usuarios por el Administrador.
  - Eliminación de usuarios por el Administrador.
  - Edición de los propios datos de perfil (usuario, teléfono, CI) por cualquier rol, separada del cambio de contraseña.
  - Bootstrap del primer usuario Administrador en una base de datos vacía.
  - Reglas de integridad para evitar que el sistema quede sin ningún Administrador.

### 3.2 Fuera de alcance de esta spec

- Cualquier otro módulo del sistema (inventario, productos, facturas, clientes/proveedores, dashboard/KPIs, notificaciones, configuración de monedas, etc.) — se especificarán en specs separadas.
- Verificación de la veracidad de los datos ingresados (fuera del alcance general del proyecto, según `Requisitos_del_sistema.docx`).
- Autenticación multifactor, biometría, SSO o cualquier mecanismo que dependa de conexión a internet (el proyecto es 100% offline).
- Auditoría/log detallado de acciones de usuario más allá del registro de errores ya exigido por `Constitution.md` §2.VI.
- Recuperación de contraseña para el Administrador cuando no conoce ni su teléfono ni su CI registrados (no hay canal alterno; queda fuera de alcance por ser offline).
- **Riesgo residual aceptado (Administrador que nunca actualiza sus datos de contacto):** si el `admin` inicial conserva el teléfono y el CI reservados (AU-06), cambia su contraseña (US-05) y luego la olvida, la recuperación (AU-02) lo rechaza y, si no existe otro Administrador que la restablezca (US-06), la única salida sería recrear la base de datos. Se **mitiga** (los valores reservados no pueden asignarse a nadie, 7.3, y el dashboard muestra una advertencia visible mientras se conserven, AU-06) pero no se elimina: si, pese a la advertencia, el Administrador nunca actualiza sus datos, el bloqueo sigue siendo posible. Se acepta por ser una aplicación local, de un solo dispositivo y sin exposición a red.
- **Riesgo residual aceptado (doble instancia de la aplicación):** nada impide abrir la aplicación dos veces a la vez en el mismo equipo. La segunda instancia, al arrancar, invalida la sesión de la primera (ver AU-05 y `plan.md` §1/§9): la primera instancia recibe el mismo aviso genérico de sesión vencida en su siguiente interacción y no queda en un estado roto. Se acepta sin añadir una dependencia nueva para evitarlo, por ser una aplicación local, de un solo dispositivo y una sola oficina.

## 4. Roles y actores

| Rol | Puede |
|---|---|
| **Administrador** | Todo lo del Contador, más: crear, listar, ver, editar y eliminar cualquier usuario (incluidos otros Administradores, con las restricciones de la sección 7.4). |
| **Contador** | Iniciar/cerrar sesión, recuperar su contraseña, ver y editar su propio perfil, cambiar su propia contraseña. **No** ve la pestaña "Usuarios" ni la opción de crear usuarios — estas están ocultas para este rol. |

No existen otros roles. El primer usuario del sistema (bootstrap) siempre es Administrador.

## 5. Modelo conceptual del usuario

(Conceptual — no es un esquema de base de datos; eso corresponde a `plan.md`.)

Cada usuario tiene:

- **Nombre de Usuario** — identificador único, usado tanto para mostrar al usuario en la interfaz como para iniciar sesión. No existen campos separados de "Nombres" y "Apellidos" (el mockup `dashboard-unificado.html` se ajusta a este modelo más simple).
- **Rol** — `Administrador` o `Contador`.
- **Número de Teléfono** — único, formato cubano `+53XXXXXXXX`.
- **Carnet de Identidad (CI)** — único, 11 dígitos numéricos.
- **Contraseña** — se almacena de forma no reversible, nunca en texto plano.

## 6. Requisitos funcionales

Cada requisito incluye escenarios de aceptación en formato Given/When/Then, testeables independientemente de la implementación.

### Módulo: Autenticación y Sesión

#### AU-01 — Iniciar sesión (RF-001)

**Qué:** cualquier usuario registrado inicia sesión con nombre de usuario y contraseña.

- **Dado** un usuario registrado con credenciales válidas, **cuando** las ingresa en la pantalla de login, **entonces** el sistema le da acceso al sistema.
- **Dado** un usuario ingresa un nombre de usuario o contraseña incorrectos, **cuando** intenta iniciar sesión, **entonces** el sistema muestra un mensaje de error genérico (sin especificar cuál de los dos campos falló).
- **Dado** existe una sesión reanudable al abrir la aplicación —es decir, una sesión persistida con "Mantener sesión activa" marcado que aún no cumple los 30 días (ver AU-05)—, **cuando** el sistema arranca, **entonces** la pantalla de inicio de sesión **no se muestra en absoluto**: se accede directo al dashboard de esa sesión. Lo mismo ocurre si, con la app ya abierta, se vuelve a la pantalla de login mientras la sesión sigue activa: se redirige al dashboard. El formulario de login solo aparece cuando no hay ninguna sesión reanudable: porque el usuario cerró sesión manualmente (AU-03), porque la sesión expiró por inactividad o por los 30 días (AU-04/AU-05), porque la sesión de la ejecución anterior no tenía "Mantener sesión activa" (AU-05) o porque nunca hubo sesión. No existe una acción de "reemplazar" la sesión activa desde el login: para que otro usuario inicie sesión, la sesión actual debe cerrarse primero (manual o automáticamente) — esa pantalla nunca es alcanzable mientras haya una sesión activa.

#### AU-02 — Restaurar contraseña (RF-002)

**Qué:** el usuario recupera el acceso verificando su identidad localmente (usuario + teléfono + CI), sin depender de conexión a internet.

- **Dado** un usuario que olvidó su contraseña, **cuando** en un primer momento ingresa su nombre de usuario, teléfono y CI y estos coinciden con los registrados, **entonces** el sistema le permite, en un segundo momento, definir una nueva contraseña. La identidad se vuelve a comprobar al guardar la nueva contraseña.
- **Dado** el nombre de usuario no existe, o el teléfono o el CI ingresados no coinciden con los registrados para ese usuario, **cuando** intenta verificar su identidad, **entonces** el sistema deniega el proceso con un mismo mensaje genérico ("Los datos no coinciden"), sin indicar cuál dato fue incorrecto ni si el usuario existe.
- **Dado** el Administrador inicial (`admin`) que aún conserva el teléfono y el CI reservados del bootstrap (AU-06), **cuando** intenta recuperar su contraseña, **entonces** el proceso se deniega con el mismo mensaje genérico: debe reemplazar esos datos desde su Perfil (US-04) antes de poder usar la recuperación.
- **Dado** una nueva contraseña que no cumple los requisitos mínimos (ver 7.2), **cuando** el usuario intenta guardarla, **entonces** el sistema rechaza el cambio y explica el requisito faltante.
- Todo el proceso ocurre contra la base de datos local; no se envía ningún dato por red.

#### AU-03 — Cerrar sesión (RF-003)

- **Dado** un usuario autenticado, **cuando** presiona "Cerrar sesión", **entonces** la sesión actual termina (incluida cualquier persistencia de "Mantener sesión activa", ver AU-05) y el sistema regresa a la pantalla de login.
- **Dado** que la sesión ya venció (por inactividad o por los 30 días) cuando el usuario presiona "Cerrar sesión", **cuando** lo hace, **entonces** igualmente llega a la pantalla de login, sin mostrar ningún error.

#### AU-04 — Cierre de sesión automático por inactividad (RF-004)

**Qué:** la sesión se cierra automáticamente tras 3 horas sin interacción del usuario.

- **Actividad** que resetea el contador de 3h: cualquier interacción del usuario con la interfaz — clic, tecla, navegación entre módulos, **incluido el movimiento del mouse**.
- **Dado** un usuario autenticado sin "Mantener sesión activa" marcado, **cuando** transcurren 3 horas sin ninguna actividad, **entonces** la sesión se cierra automáticamente sin mensaje de advertencia previo, y el sistema regresa a la pantalla de login.
- **Dado** un usuario con "Mantener sesión activa" marcado (AU-05), **cuando** transcurre cualquier cantidad de tiempo de inactividad, **entonces** el cierre por inactividad de 3h **no aplica** — la sesión solo puede terminar por los 30 días de persistencia (AU-05) o por cierre de sesión manual (AU-03).
- **Decisión explícita, documentada aquí como limitación aceptada:** el cierre se dispara mediante un chequeo hecho en la siguiente interacción del usuario (clic, tecla, movimiento de mouse) tras cumplirse las 3h, no mediante un temporizador de fondo que actúe en el instante exacto del vencimiento. En la práctica esto significa que si el usuario deja la aplicación abierta y **completamente** inactiva más allá de las 3h, la sesión queda vencida pero la pantalla no cambia a login hasta que vuelve a tocar la interfaz — en ese momento se cierra de inmediato, antes de procesar esa interacción como actividad nueva. Se acepta porque un temporizador de fondo agrega complejidad no justificada (`Constitution.md` §2.II) para un caso límite poco realista en este contexto (un solo usuario/oficina); el objetivo real de AU-04 — que una sesión olvidada no quede reanudable — se cumple igual.

#### AU-05 — Mantener sesión activa / persistencia (RF-005)

**Qué:** define cómo sobrevive la sesión al cierre de la aplicación.

- **Dado** un usuario que inició sesión **sin** marcar "Mantener sesión activa", **cuando** cierra la aplicación, **entonces** al volver a abrirla debe iniciar sesión nuevamente (la sesión no persiste entre cierres) — **sin importar cuánto tiempo haya pasado** (aunque hayan transcurrido solo unos minutos y no se haya cumplido ni el timeout de 3h de inactividad ni los 30 días). Haber marcado o no "Mantener sesión activa" es la única condición que determina si una sesión puede reanudarse al reabrir la app; no es un atajo del timeout de inactividad, es una condición independiente y previa a cualquier chequeo de expiración.
- **Dado** un usuario que inició sesión **con** "Mantener sesión activa" marcado, **cuando** cierra y vuelve a abrir la aplicación (incluso tras reiniciar la PC), **entonces** el sistema lo mantiene autenticado automáticamente, sin pedir credenciales, **hasta un máximo de 30 días**.
- El conteo de los 30 días (y el de las 3 horas de inactividad de AU-04) se basa en el **reloj del sistema operativo** (la fecha de inicio de sesión y la de la última actividad se comparan contra la hora actual del SO). **Decisión explícita, documentada aquí como limitación aceptada:** un reloj verdaderamente interno de la app, inmune a cambios de fecha, no sobrevive un reinicio de la PC — y la persistencia de sesión sí debe sobrevivirlo (requisito de esta misma sección). Ambos requisitos no son alcanzables a la vez sin un ancla de tiempo externa, prohibida por el principio offline-first (`Constitution.md` §2.IV). Se prioriza la persistencia entre reinicios sobre la inmunidad a manipulación manual de fecha. Riesgo aceptado: si el usuario adelanta o atrasa manualmente la fecha del sistema, puede extender o acortar artificialmente su propia sesión. No se considera un defecto — no es un caso de uso realista para el contexto de negocio (un solo usuario/oficina, sin exposición a red).
- **Dado** que la sesión persistida cumple 30 días (según el reloj del sistema operativo), **cuando** el usuario abre la aplicación, **entonces** se le pide iniciar sesión de nuevo.
- **Dado** que solo puede existir una sesión activa a la vez en todo el sistema, **cuando** un usuario con sesión persistida (app cerrada) quiere que otro usuario inicie sesión, **entonces** debe cerrarse esa sesión persistida (AU-03) antes de que otro usuario pueda ingresar.
- **Dado** el usuario cuya cuenta respalda una sesión persistida fue eliminado del sistema mientras tenía la app cerrada (ver US-07 y 7.4), **cuando** reabre la aplicación dentro de los 30 días, **entonces** la sesión se invalida silenciosamente y se lo redirige al login con un mensaje genérico, sin indicar que su cuenta fue eliminada.

#### AU-06 — Bootstrap del primer Administrador *(nuevo — no está en los RF originales)*

**Qué:** al iniciar la aplicación por primera vez con una base de datos vacía, debe existir una forma de acceder para crear el resto de los usuarios.

- **Dado** una base de datos recién inicializada sin ningún usuario, **cuando** el sistema arranca por primera vez, **entonces** existe automáticamente un usuario Administrador con nombre de usuario `admin` (en minúsculas, consistente con la normalización de `username` de §7.1 — no `Admin`) y contraseña `Admin123!` (la contraseña sí distingue mayúsculas/minúsculas, solo el nombre de usuario se normaliza).
- El usuario `admin` inicial se crea con el teléfono `+5300000000` y el CI `00000000000`, **valores reservados** del sistema. El Administrador los reemplaza por los suyos desde su Perfil (US-04). Mientras los conserve, la recuperación de contraseña (AU-02) no está disponible para `admin`.
- **Advertencia visible:** mientras el usuario Administrador conserve el teléfono o el CI reservados, el dashboard muestra una advertencia visible ("Actualiza tu teléfono y CI para poder recuperar tu contraseña si la olvidas") que desaparece en cuanto los reemplaza. Ningún usuario puede asignarse los valores reservados (7.3).
- **Dado** el Administrador inicial (`admin`), **cuando** decide hacerlo, **entonces** puede cambiar su contraseña manualmente desde su perfil (US-05) en cualquier momento; el sistema no lo obliga a cambiarla en el primer inicio de sesión.
- **Riesgo aceptado (decisión de producto):** las credenciales fijas `admin` / `Admin123!` no obligan a un cambio en el primer inicio. Se acepta por ser una aplicación local, de un solo dispositivo y sin exposición a red.

#### AU-07 — Sin límite de intentos de acceso *(nuevo — no está en los RF originales)*

**Qué:** define explícitamente que no hay protección contra fuerza bruta, para no dejarlo ambiguo.

- **Dado** un usuario que ingresa credenciales incorrectas repetidamente (en login o en recuperación de contraseña), **cuando** lo intenta las veces que quiera, **entonces** el sistema no aplica bloqueos temporales ni retrasos — no hay límite de intentos.
- Justificación: aplicación local, de un solo usuario activo a la vez, sin exposición a red; el riesgo de fuerza bruta remoto no aplica.

---

### Módulo: Usuarios

#### US-01 — Crear nuevo usuario (RF-006)

**Qué:** el Administrador registra un nuevo usuario.

- **Dado** el Administrador autenticado, **cuando** accede al formulario de creación de usuario y completa Nombre de Usuario, Rol, Teléfono, CI, Contraseña y Confirmar contraseña con datos válidos y únicos, **entonces** el usuario se crea. *("Confirmar contraseña" es una comprobación de la interfaz contra errores de tecleo: no se envía ni se guarda.)*
- **Dado** el Contador autenticado, **cuando** intenta acceder a la creación de usuarios, **entonces** no tiene esa opción disponible en la interfaz (pestaña oculta para este rol).
- **Dado** el teléfono o el CI ya pertenecen a otro usuario existente, **cuando** el Administrador intenta crear el usuario, **entonces** el sistema deniega la creación con un mensaje que indica cuál dato ya está registrado (p. ej. "El teléfono ya está registrado").
- **Dado** el nombre de usuario ya existe (comparación **sin distinguir mayúsculas/minúsculas** — ver 7.1), **cuando** se intenta crear otro con el mismo nombre, **entonces** el sistema deniega la creación indicando que el nombre de usuario ya está registrado.
- **Dado** una contraseña que no cumple los requisitos mínimos (7.2), **cuando** se intenta crear el usuario, **entonces** el sistema rechaza el formulario y explica el requisito faltante.
- **Dado** el teléfono o el CI ingresados son los valores reservados del bootstrap (7.3), **cuando** el Administrador intenta crear el usuario, **entonces** el sistema deniega la creación con un mensaje claro.

#### US-02 — Ver listado de usuarios (RF-007)

- **Dado** el Administrador autenticado, **cuando** accede a la pestaña "Usuarios", **entonces** ve la lista de todos los usuarios con: Nombre de Usuario, Rol, Teléfono y CI.
- **Dado** el Contador autenticado, **entonces** la pestaña "Usuarios" no está visible ni accesible.
- En el listado, la fila del propio usuario autenticado se identifica y **no ofrece** las acciones de editar ni eliminar: sus propios datos se editan desde Perfil (US-04) y no puede eliminarse a sí mismo (US-07). Es solo UX: el backend rechaza igualmente ambas acciones sobre uno mismo (7.4).

#### US-03 — Ver perfil propio (RF-008)

- **Dado** cualquier usuario autenticado (Administrador o Contador), **cuando** accede a la sección "Perfil", **entonces** ve sus propios datos: Nombre de Usuario, Rol, Teléfono y CI.

#### US-04 — Editar perfil propio *(extensión de RF-008/009, decidida en esta ronda)*

**Qué:** cualquier usuario puede actualizar sus propios datos de contacto sin pasar por el Administrador.

- **Dado** cualquier usuario autenticado, **cuando** edita su Nombre de Usuario, Teléfono y/o CI desde su Perfil con datos válidos y únicos, **entonces** el sistema guarda los cambios **sin pedir la contraseña actual**.
- **Dado** el nuevo Nombre de Usuario, Teléfono o CI ya pertenece a otro usuario, **cuando** intenta guardar, **entonces** el sistema deniega el cambio con un mensaje de error (mismas reglas de unicidad que US-01, incluido el mensaje que indica qué dato ya está registrado).
- **Dado** cualquier usuario autenticado, **cuando** guarda su perfil sin modificar ninguno de sus datos, o modificando solo uno de los tres campos (Nombre de Usuario, Teléfono, CI) y dejando los otros dos iguales, **entonces** el sistema guarda el cambio sin rechazarlo por "duplicado" — la validación de unicidad **debe excluir la propia fila** del usuario que se está editando; un valor sigue siendo único aunque coincida con el valor ya guardado del mismo usuario.
- **Dado** cualquier usuario (incluido el Administrador), **cuando** está en la pantalla de Perfil propio, **entonces** el campo Rol **no es editable** en ningún caso — el rol solo puede cambiarlo un Administrador sobre otro usuario, nunca sobre sí mismo (evita auto-degradación/auto-promoción; ver 7.4).
- **Dado** cualquier usuario, **cuando** intenta guardar como teléfono o CI los valores reservados del bootstrap (7.3), **entonces** el sistema rechaza el cambio con un mensaje claro. El `admin` inicial, que ya los conserva, puede guardar otros campos (p. ej. su Nombre de Usuario) sin cambiarlos.
- El cambio de contraseña se mantiene como una acción separada dentro de Perfil, que sí exige la contraseña actual (US-05 / RF-009).

#### US-05 — Cambiar contraseña desde perfil (RF-009)

- **Dado** cualquier usuario autenticado, **cuando** ingresa su contraseña actual correctamente junto con una nueva contraseña válida (7.2) y su confirmación, **entonces** el sistema actualiza la contraseña y muestra confirmación. *(Igual que en US-01, la confirmación es una comprobación de la interfaz.)*
- **Dado** la contraseña actual ingresada es incorrecta, **cuando** intenta cambiarla, **entonces** el sistema rechaza el cambio con un mensaje de error.
- **Dado** la nueva contraseña no cumple los requisitos mínimos, **entonces** el sistema rechaza el cambio y explica el requisito faltante.

#### US-06 — Editar usuario (Administrador sobre otro usuario) *(nuevo — no está en los RF originales)*

**Qué:** el Administrador puede modificar los datos de cualquier usuario del sistema.

- **Dado** el Administrador autenticado, **cuando** edita el Nombre de Usuario, Rol, Teléfono y/o CI de otro usuario con datos válidos y únicos, **entonces** el sistema guarda los cambios sin restricciones adicionales, salvo que ni el teléfono ni el CI pueden ser los valores reservados del bootstrap (7.3). La interfaz **no ofrece** editar (ni eliminar) la fila del propio Administrador — sus datos se editan desde Perfil —, y el backend además rechaza la edición sobre uno mismo (7.4).
- **Dado** el Administrador intenta cambiar el rol de otro usuario, **cuando** ese usuario no es el último Administrador del sistema, **entonces** el cambio se permite libremente (de Contador a Administrador o viceversa).
- Reglas de unicidad idénticas a US-01/US-04 — igual que en US-04, la validación **excluye la propia fila** del usuario que se está editando: guardar sin cambiar sus datos, o cambiando solo uno de los tres campos, no debe rechazarse como "duplicado".
- **Dado** el Administrador autenticado, **cuando** establece una nueva contraseña para otro usuario desde la pantalla de edición (sin necesidad de conocer ni ingresar la contraseña actual de ese usuario), **entonces** el sistema la actualiza siempre que cumpla los requisitos mínimos (7.2) — es una acción administrativa, independiente de US-05 (que es la que usa el propio usuario sobre sí mismo) y de AU-02 (recuperación por el propio usuario).
- **Dado** el Administrador restablece **su propia** contraseña por esta vía, **cuando** lo hace, **entonces** debe indicar también su contraseña actual (igual que en US-05).
- **Dado** la pantalla de edición combina el guardado de los datos (`updateUser`) con el establecimiento de una nueva contraseña (`resetUserPassword`) — son dos operaciones independientes, no una transacción —, **cuando** el guardado de los datos tiene éxito pero el establecimiento de la contraseña falla, **entonces** la interfaz muestra un error indicando que los datos se guardaron pero la contraseña no cambió, y **no** revierte los datos ya guardados. Si es el guardado de los datos el que falla, no se intenta establecer la contraseña.
- *Verificación defensiva (no es un flujo alcanzable desde la interfaz):* **dado** el usuario editado es el último Administrador del sistema y el cambio lo dejaría como Contador, **entonces** el sistema lo bloquea (7.4). En la práctica no puede ocurrir por la vía normal, porque quien edita es un Administrador distinto del editado.

#### US-07 — Eliminar usuario (Administrador) *(nuevo — no está en los RF originales)*

**Qué:** el Administrador puede eliminar cuentas de usuario.

- **Dado** el Administrador autenticado, **cuando** elimina a otro usuario (Administrador o Contador) que no sea el último Administrador del sistema, **entonces** la cuenta se elimina definitivamente, previa confirmación del Administrador (mensaje de confirmación antes de borrar).
- **Dado** el Administrador autenticado, **cuando** intenta eliminar su propia cuenta, **entonces** el sistema no lo permite (no puede eliminarse a sí mismo): la interfaz no ofrece la acción sobre la fila propia y el backend además la rechaza.
- *Verificación defensiva (no es un flujo alcanzable desde la interfaz):* **dado** el usuario a eliminar es el único Administrador restante del sistema, **cuando** se intenta eliminar, **entonces** el sistema lo impide y muestra un mensaje explicando que no puede quedar el sistema sin ningún Administrador. En la práctica quien elimina es un Administrador distinto del eliminado, por lo que siempre existen al menos dos.
- **Dado** el usuario eliminado tenía una sesión persistida ("Mantener sesión activa") con la app cerrada, **entonces** aplica el comportamiento de AU-05 (invalidación silenciosa al reabrir). *Aclaración: el proyecto es 100% local y de una sola instalación (`Constitution.md` §1) — no existe "otra instalación" ni sincronización entre dispositivos; la sesión persistida referida aquí es siempre la misma (y única, §7.5) base de datos local. Dado el invariante de sesión única y que un Administrador no puede eliminarse a sí mismo, en el flujo normal de la UI la sesión del usuario que se va a eliminar ya tuvo que cerrarse antes de que el Administrador pudiera llegar al login — por eso este escenario es, en la práctica, una verificación defensiva de la robustez del sistema ante estados inconsistentes, no un flujo de usuario típico.*
- La eliminación es **física** (el registro del usuario se borra por completo de la base de datos, no un borrado lógico). Si otros módulos referencian a ese usuario en historiales (ej. RF-091 "Usuario que realizó el movimiento"), el manejo de esa referencia se define en la spec del módulo correspondiente, no en esta.

---

## 7. Reglas de negocio transversales

### 7.1 Nombre de Usuario

- Único en todo el sistema.
- Comparación de unicidad **e inicio de sesión ambos sin distinguir mayúsculas de minúsculas** (`Admin` y `admin` se consideran el mismo usuario).
- 3 a 50 caracteres alfanuméricos. El formato lo define esta spec: lo imponen la validación del backend y el `CHECK` del esquema (`plan.md` §2).

### 7.2 Contraseña

- Entre **8 y 72 caracteres**, al menos una mayúscula y al menos un carácter especial.
- Solo **ASCII imprimible sin espacios**. Mayúscula = `A`–`Z`. Carácter especial = símbolo o puntuación ASCII: ``! " # $ % & ' ( ) * + , - . / : ; < = > ? @ [ \ ] ^ _ ` { | } ~``. Se rechazan acentos, `ñ` y espacios con un mensaje claro.
- Se almacena de forma no reversible, nunca en texto plano.
- El inicio de sesión no aplica estas reglas: solo compara la contraseña ingresada, y cualquier entrada que no coincida da el mensaje genérico de credenciales incorrectas.

### 7.3 Teléfono y Carnet de Identidad (CI)

- **Teléfono:** único en el sistema, formato cubano `+53` seguido de 8 dígitos.
- **CI:** único en el sistema, exactamente 11 dígitos numéricos. **No** se valida la lógica interna real de un carnet de identidad cubano (fecha de nacimiento embebida, dígito verificador, etc.) — solo se valida el formato de 11 dígitos. Esto queda explícitamente fuera de alcance.
- **Valores reservados de bootstrap** (`+5300000000` y `00000000000`, AU-06): ningún usuario puede asignárselos al crear un usuario (US-01) ni al editar un perfil propio o ajeno (US-04, US-06); se rechazan con un mensaje claro. Un usuario que ya los conserva (el `admin` inicial) puede seguir guardando otros campos sin cambiarlos. Motivo: un usuario con valores reservados no puede recuperar su contraseña (AU-02) y, sin otro Administrador que la restablezca (US-06), quedaría sin salida.

### 7.4 Integridad de Administradores

- El sistema **nunca** puede quedar sin ningún usuario con rol Administrador.
- Un Administrador puede eliminar a otros Administradores, pero **no puede eliminarse a sí mismo** en ningún caso.
- Antes de eliminar un Administrador, o de cambiar el rol de un Administrador a Contador, el sistema **debe verificar** si ese usuario es el último Administrador del sistema. Si lo es, la acción se bloquea y se muestra una notificación explicando que no es posible porque es el último Administrador.
- Un Administrador no puede cambiar su propio rol desde ningún punto de la interfaz (ni Perfil propio, ni ninguna otra pantalla) — evita la auto-degradación accidental o intencional.
- Dado que solo existe una sesión activa a la vez, y quien actúa es siempre un Administrador que no puede eliminarse ni degradarse a sí mismo, el sistema siempre tiene al menos dos Administradores cuando uno actúa sobre otro: la regla del último Administrador **no es alcanzable desde la interfaz**. Se mantiene igualmente como **defensa en profundidad**, para que un estado inconsistente o una llamada directa nunca deje el sistema sin Administradores.

### 7.5 Sesión única

- Solo puede haber una sesión activa (o persistida) a la vez en todo el sistema.
- Si hay una sesión persistida de un usuario con la app cerrada y otro usuario necesita iniciar sesión, primero debe cerrarse esa sesión (AU-03) antes de permitir el nuevo inicio de sesión.

### 7.6 Cantidad de usuarios

- No existe límite en la cantidad de usuarios que pueden registrarse, ni por rol ni en total. El Administrador puede crear tantos Administradores y Contadores como el negocio necesite.

## 8. Manejo de errores (aplicable a todo este módulo)

Por `Constitution.md` §2.VI y `Agents.md` §5, sin exponer nunca detalles técnicos ni trazas. Los mensajes visibles se clasifican así, siempre en español:

- **Genéricos** en autenticación (inicio de sesión y recuperación de contraseña, AU-01/AU-02): no indican si falló el usuario, la contraseña, el teléfono o el CI (ej. "El nombre de usuario o la contraseña son incorrectos", "Los datos no coinciden").
- **Específicos** en validación y reglas de negocio, porque el usuario ya está autenticado y necesita saber qué corregir: qué dato está duplicado (usuario, teléfono o CI), qué requisito de contraseña falta, o que no puede quedar el sistema sin Administradores.
- **Genérico** ante cualquier error interno (ej. "Ocurrió un error inesperado"), sin detalle técnico.
- **No se puede abrir la base de datos al iniciar:** mensaje genérico ("No se pudo abrir la base de datos") y cierre de la aplicación.
- Todo error **inesperado o de sistema** se registra en el log local con módulo, operación, causa y timestamp, para depuración, sin exponerse al usuario final. **No** se registran los resultados esperados de una acción del usuario: credenciales incorrectas (AU-01) y datos ingresados que no pasan la validación (`validationFailed`) — al no haber límite de intentos (AU-07), llenarían el log de ruido. Ningún registro contiene contraseñas, hashes ni datos personales (nombre de usuario, teléfono, CI). Los registros de más de 90 días se eliminan al iniciar la aplicación.

## 9. Fuera de alcance (recapitulación)

- Multifactor, biometría o SSO.
- Bloqueo por intentos fallidos (decidido explícitamente: no existe, ver AU-07).
- Validación de la lógica interna real del CI cubano.
- Cualquier módulo fuera de Autenticación y Usuarios.

## 10. Desviaciones respecto de `Requisitos_del_sistema.docx`

La verificación de trazabilidad contra `Requisitos_del_sistema.docx` (RF-001 a RF-009) encontró tres diferencias puntuales, aceptadas y registradas aquí para que no se confundan con huecos de cobertura:

- **RF-002 (Restaurar contraseña):** el requisito original describe un flujo secuencial (primero el nombre de usuario y, en un segundo paso, el sistema pide teléfono y CI). Esta spec (AU-02) pide los tres datos juntos en un solo paso. Se decide así para no revelar, a través de la forma del flujo, si el nombre de usuario ingresado existe.
- **RF-004 (Cierre de sesión por inactividad):** el requisito pide que, cumplidas las 3 horas, el sistema cierre la sesión y redirija por sí solo. Esta spec (AU-04) no lo hace de forma proactiva: el cierre se aplica en la siguiente interacción del usuario, no mediante un temporizador de fondo. Se acepta porque evita una complejidad adicional en el frontend y `validate_session` sigue impidiendo cualquier acción sobre una sesión ya vencida, sin importar cuándo se detecte.
- **RF-005 (Mantener sesión activa):** el requisito no fija ningún límite de tiempo para la persistencia. Esta spec (AU-05) añade un máximo de 30 días.

## 11. Estado

Todas las decisiones abiertas de las rondas de auditoría (seguridad, contrato frontend-backend, orden y cobertura de tests, higiene documental) y de la verificación de trazabilidad contra `Requisitos_del_sistema.docx` están cerradas y confirmadas por el dueño del proyecto. `plan.md` y `tasks.md` están alineados con esta spec. Por `Agents.md` §2, se cuenta con la confirmación necesaria para empezar a implementar.