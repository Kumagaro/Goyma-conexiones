# UI Spec — Sistema de diseño de Goyma Conexiones

- **Estado:** en vigor. Toda pantalla nueva o modificada sigue estas reglas.
- **Origen:** auditoría UX/UI del 2026-11-06 y decisiones del dueño.
- **Código:** `src/css/design-system.css` (fuentes y variables) y `src/css/components.css` (componentes y utilidades).

## 1. Principios

1. **Comodidad antes que decoración.** Es una herramienta de trabajo de muchas horas: legibilidad, orden y calma.
2. **Una sola fuente de verdad.** Los colores, tamaños, radios y duraciones salen de las variables; no se escriben valores sueltos.
3. **Accesible por defecto.** WCAG 2.1 AA: contraste, teclado y movimiento reducido.
4. **Sin internet.** Las fuentes van dentro de la aplicación.

## 2. Tipografía

| Uso | Fuente | Variable |
|---|---|---|
| Títulos (`h1` a `h4`, títulos de tarjeta) | Poppins 500, 600, 700 | `--font-heading` |
| Texto, tablas, formularios, números | Inter (variable) | `--font-body` |

| Variable | Tamaño | Uso |
|---|---|---|
| `--text-xs` | 12px | Mínimo absoluto: etiquetas, notas, insignias |
| `--text-sm` | 13px | Texto secundario |
| `--text-base` | 14px | Texto de trabajo: tablas, formularios, botones |
| `--text-md` | 16px | Texto destacado |
| `--text-lg` | 18px | Subtítulos |
| `--text-xl` | 20px | Títulos de tarjeta |
| `--text-2xl` | 24px | Título de página |
| `--text-3xl` | 28px | Cifras destacadas |
| `--text-4xl` | 32px | Cifras principales |
| `--text-5xl` | 40px | Cifras de portada |

**Nunca** menos de 12px.

## 3. Color

Azul principal: **`#2563eb`** (decisión del dueño).

| Variable | Valor | Uso |
|---|---|---|
| `--color-primary` / `-hover` | `#2563eb` / `#1d4ed8` | Acción principal, enlaces |
| `--color-primary-soft`, `-tint`, `-ring`, `-border` | Azul con transparencia | Fondos, foco, bordes |
| `--color-ink` | `#020f1e` | Texto principal, barra lateral |
| `--color-text-muted` | `#56606e` | Texto secundario (6:1 sobre blanco) |
| `--color-line`, `--color-line-strong`, `--color-ink-soft` | Tinta con transparencia | Bordes y fondos neutros |
| `--color-bg` / `--color-white` / `--color-surface-2` | `#eff3f6` / `#ffffff` / `#f7f9fb` | Fondo azulado, tarjetas y encabezados de tabla |
| `--color-danger` | `#dc2626` | Destructivo y errores |
| `--color-success` / `-text` | `#16a34a` / `#15803d` | El de relleno y el apto para texto |
| `--color-warning` / `-text` | `#f48430` / `#b45309` | El de relleno y el apto para texto |

- Texto normal: contraste mínimo **4,5:1**. Por eso verde y naranja tienen versión `-text`.
- El estado nunca se comunica solo con color: va con texto o icono.

## 4. Forma, espacio y movimiento

- **Radios:** `--radius-sm` 6px (botones, campos), `--radius-md` 10px, `--radius-lg` 14px (tarjetas), `--radius-xl` 20px, `--radius-pill`.
- **Espaciado:** múltiplos de 4 (`--space-1` a `--space-8`).
- **Superficies:** tarjetas blancas con `--shadow-card` sobre el fondo azulado. Sin difuminados: son caros en equipos modestos.
- **Movimiento:** corto, discreto y con `transform` y `opacity` (más el trazo de los minigráficos). Nunca `transition: all` ni bucles infinitos de adorno. Con `prefers-reduced-motion` no hay animación.

| Patrón | Efecto | Duración |
|---|---|---|
| Cambio de pantalla | Fundido con subida de 4px | 180ms |
| Diálogo | Escala de 96 % a 100 % con fundido | 200ms |
| Filas de tabla | Fundido | 150ms |
| Tarjetas del dashboard | Suben en cascada, 30ms entre una y otra | 220ms |
| Minigráficos y barras | La línea se dibuja; la barra crece desde la izquierda | 400ms |
| Pulsar un botón | Se reduce a 97 % | 100ms |
| Campo con error | Temblor corto y mensaje que aparece | 200ms |
| Notificación nueva | La campana se agita | 500ms |

## 5. Componentes

- **Botones:** `.btn-primary` (una acción principal por pantalla), `.btn-secondary`, `.btn-danger`. Misma altura: 40px.
- **Acciones de fila:** `.action-link`, altura mínima 32px; `.danger` para destructivas.
- **Títulos de página:** cada página tiene un único `<h1 class="page-title">`.
- **Foco:** todo lo interactivo muestra el anillo `:focus-visible`. Quitar `outline` exige un sustituto visible.
- **Campos:** todos los `input`, `select` y `textarea` miden 46px, con el mismo borde, radio y foco, también en diálogos; dentro de las líneas de factura, 40px. El navegador no añade controles propios: el ojo de contraseña es solo el de la aplicación.
- **Sin excepciones oscuras:** filtros, fechas y buscadores de lista parten de la misma regla base de `components.css`; solo el buscador en píldora y las líneas de factura tienen variante propia.
- **Solo icono:** `<button>` real con `aria-label`. Un `<div>` nunca es clicable.
- **Tablas:** `.list-table`: tarjeta con encabezado en mayúscula pequeña y filas con línea fina. Si no caben, se desplazan dentro de su contenedor, nunca se cortan.
- **Orden y encabezados:** pulsar un encabezado ordena la columna (`tableSort.js`), salvo acciones y fechas con orden propio. El encabezado queda fijo al desplazar la tabla.
- **Acciones de producto:** Ver y Editar con texto; Descontinuar y Eliminar solo con icono, `title` y texto `.sr-only`.
- **Atajos:** Ctrl+S guarda el formulario abierto y Ctrl+K lleva a la búsqueda de la página.
- **Importes:** siempre con separador de miles desde 4 cifras (`1.234,50 USD`), en pantalla, PDF y exportaciones.
- **Menú:** agrupado con `.nav-label` en Operación, Catálogos y Sistema.
- **Contenedores:** nada sobresale de su tarjeta o columna. Los textos largos saltan de línea o se cortan con puntos suspensivos.
- **Estados:** píldora `.status-pill` con fondo suave y texto oscuro: pendiente, completado o cancelado.
- **Estados vacíos:** dicen qué falta y ofrecen la acción para crearlo. Con filtros activos dicen que no hay coincidencias.

## 6. Formularios y textos

- Toda entrada tiene etiqueta visible; `placeholder` solo para formato.
- El formato se valida **al salir del campo** y el error desaparece al corregir. Los campos vacíos se avisan al guardar.
- Los errores van junto al campo, con el estilo de la aplicación, nunca el globo del navegador.
- Botones con verbo concreto: "Guardar cliente", no "Aceptar".
- Toda acción destructiva confirma nombrando el registro, con un verbo destructivo ("Eliminar", "Restaurar").
- Los mensajes no culpan al usuario y dicen cómo seguir.

## 7. Ventana

La ventana abre **maximizada y no se puede reducir** (decisión del dueño), conservando la barra de título para minimizar y cerrar. En pantallas pequeñas el diseño debe funcionar desde **1024×640**. El diseño de escritorio es el principal.

## 8. Tareas

- [x] Auditoría UX/UI.
- [x] 1. Diálogo de eliminar unidad y conversión.
- [x] 2. Fuentes Poppins e Inter incluidas.
- [x] 3. Tamaño de ventana.
- [x] 4. Contraste y tamaño mínimo de texto.
- [x] 5. Teclado y foco.
- [x] 6. Variables en lugar de valores sueltos, y estilos en línea a clases.
- [x] 7. Botones de la misma escala.
- [x] 8. Un título por página.
- [x] 9. Confirmaciones que nombran el registro.
- [x] 10. Validación al salir del campo.
- [x] 11. Estados vacíos con acción.
- [x] 12. Tablas con desplazamiento propio.
- [x] 13. Movimiento.
- [x] 14. Revisión visual de las pantallas principales a 1280 y 1024, con datos de ejemplo.

Hallazgos de la revisión visual, también corregidos: Clientes y Proveedores no cargaban sus datos al arrancar; Borradores y Registro de Actividad abrían "Próximamente"; los botones de las filas de facturas no tenían estilo; el buscador de productos de las facturas no parecía un campo.

**Pendiente:** comprobar en la app real con `tauri:dev` (`verificacion-manual.md`).
