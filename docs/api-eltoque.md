# API de tasas de elTOQUE

Referencia para solicitar la clave y para la spec de tasas automáticas (`PEN-23`). Servicio autorizado por `Constitution.md` 2.0.0, §2.IV.

Datos tomados de la documentación pública (`https://tasas.eltoque.com/docs/`, versión `1.0.beta`) el 2026-11-11. El formulario de solicitud y los términos completos están detrás de una verificación de Cloudflare, así que no se pudieron leer automáticamente.

---

## 1. Solicitar la clave

- **Formulario:** `https://tasas-token.eltoque.com/`.
- La API pide **una clave por cada aplicación** que se integra.
- Goyma Conexiones es una aplicación de escritorio: **no tiene web propia** y la llamada sale del backend en Rust, no de un navegador.

### Qué poner en el formulario

| Campo | Qué poner | Por qué |
|---|---|---|
| **Nombre de la aplicación** (si lo pide) | `Goyma Conexiones` | El nombre del producto. |
| **URL de la aplicación** | `https://github.com/Kumagaro/Goyma-conexiones` | El repositorio del proyecto, público desde el 2026-11-11. Identifica la aplicación sin necesidad de una web propia. |
| **URL de la aplicación origen** | `https://github.com` | Es el origen (esquema y dominio) de la URL anterior. Goyma llama a la API desde Rust, que no envía origen, así que este campo no limita nada. |
| **Descripción o uso** (si lo pide) | "Aplicación de escritorio para la gestión de un negocio en Cuba (inventario, ventas y facturas). Consulta la tasa del mercado informal una vez al día como máximo, desde su backend, para mostrar equivalencias de precios. Cita a elTOQUE como fuente." | Deja claro que es un uso ligero, de solo lectura y con atribución. |

### 1.1 Página propia (opcional)

`docs/pagina-publica/index.html` es una página de presentación por si más adelante se prefiere una URL propia: se publica con GitHub Pages en un repositorio público `Kumagaro.github.io` y queda en `https://kumagaro.github.io/`. La página no lleva datos del negocio ni de contacto personal.

No pongas `http://tauri.localhost` (el origen interno de la app en Windows): solo serviría si la llamada se hiciera desde el frontend, y la constitución lo prohíbe.

---

## 2. Cómo se usa

- **Endpoint:** `GET https://tasas.eltoque.com/v1/trmi`.
- **Autenticación:** cabecera `Authorization: Bearer <clave>` (formato JWT).
- **Qué devuelve:** las medianas de las ofertas de compra y venta de divisas en el mercado informal de las últimas 24 horas.
- **Parámetros opcionales:** `date_from` y `date_to`, con formato `AAAA-MM-DD HH:MM:SS` (por ejemplo `2022-10-27 00:00:01`), para la tasa de un día concreto. Un rango mayor de 24 horas da error.
- **Respuestas de error documentadas:** `400`, `401` (clave inválida), `422` y `429` (límite superado).
- **Formato de la respuesta 200:** la documentación no lo describe. Hay que verlo con una clave real antes de escribir la spec.

---

## 3. Límites

- Se aplican **por clave**, no por IP: por defecto **60 peticiones por minuto**, con un máximo de **10 por segundo**. Una clave puede tener otra cuota.
- Al superarlos responde `429` con un JSON `{"error": "...", "limit": "60 per 1 minute"}` y la cabecera `Retry-After`, en segundos.
- Todas las respuestas traen `X-RateLimit-Limit`, `X-RateLimit-Remaining` y `X-RateLimit-Reset`.

Goyma necesita como mucho unas pocas peticiones al día.

---

## 4. Términos de uso (resumen de la documentación)

- **Citar a elTOQUE** como fuente de los datos obtenidos con la API.
- **No dar la clave a terceros.** Por eso la clave no va en el código ni en el instalador: la introduce el Administrador de cada instalación (`Constitution.md` §2.IV). Si la aplicación se instala en otros negocios, cada uno necesita su propia clave.
- Las tasas son **referenciales**; elTOQUE no responde por el uso que se haga de ellas.
- **No modificar los datos**, salvo para actualizarlos.
- No usar la API con contenido malicioso, de odio, engañoso o discriminatorio.
- Términos completos: `https://eltoque.com/api-tasas-de-eltoque-terminos-y-condiciones-de-uso`. Conviene leerlos en el navegador antes de la spec.

---

## 5. Pendiente para la spec (`spec-07`)

- Ver el formato real de la respuesta con la clave.
- Decidir qué monedas de CF-02 se actualizan, y cada cuánto: al abrir la app, una vez al día o con un botón.
- Decidir si la tasa traída se guarda en el historial de tasas como un cambio más, firmado por "elTOQUE".
- Decidir dónde se introduce y se guarda la clave. Tener en cuenta que los respaldos la incluirían.
- Añadir un cliente HTTP en Rust. Es una dependencia nueva y hay que justificarla en el plan.
