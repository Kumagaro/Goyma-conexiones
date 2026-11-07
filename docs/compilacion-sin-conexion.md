# Compilar sin conexión

Guía para generar el instalador sin internet. Hay que **preparar el equipo una sola vez con conexión**. Después, `npm run tauri:build` funciona sin internet, y el cliente instala sin internet.

## Qué hace falta

| Pieza | Dónde queda | Se descarga |
|---|---|---|
| Dependencias de Rust | `%USERPROFILE%\.cargo\registry` | Ya están en este equipo |
| Dependencias de npm | `node_modules` | Ya están en este equipo |
| WebView2 (copia fija) | `src-tauri\webview2-runtime` | Paso 1, a mano |
| NSIS (crea el `.exe`) | `%LOCALAPPDATA%\tauri\NSIS` | Paso 2, automático |

`webview2-runtime` pesa unos 200 MB y no se guarda en Git. En un equipo nuevo hay que repetir esta guía.

## Paso 1: descargar WebView2

1. Abre <https://developer.microsoft.com/microsoft-edge/webview2/> y ve a **Fixed Version**.
2. Elige la versión más reciente, arquitectura **x64**, y descarga el `.cab`. Microsoft puede bloquear el acceso desde Cuba: si la página no abre, usa una VPN o pide el archivo a alguien fuera.
3. Copia el `.cab` a la carpeta del proyecto y descomprímelo desde PowerShell, cambiando `<versión>` por la del archivo:

```powershell
expand .\Microsoft.WebView2.FixedVersionRuntime.<versión>.x64.cab -F:* .\src-tauri
```

4. Renombra la carpeta creada para que la ruta no cambie con cada versión:

```powershell
Rename-Item .\src-tauri\Microsoft.WebView2.FixedVersionRuntime.<versión>.x64 webview2-runtime
```

5. Comprueba que existe `src-tauri\webview2-runtime\msedgewebview2.exe` y borra el `.cab`.

## Paso 2: primera compilación, con internet

```bash
npm run tauri:build
```

Tauri descarga NSIS de GitHub y lo guarda en `%LOCALAPPDATA%\tauri\NSIS`. El instalador queda en `src-tauri\target\release\bundle\nsis`.

**Si GitHub no responde,** descarga estos dos archivos con una VPN y colócalos a mano:

- <https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip>: descomprime su contenido en `%LOCALAPPDATA%\tauri\NSIS`, de modo que exista `%LOCALAPPDATA%\tauri\NSIS\makensis.exe`.
- <https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll>: cópialo a `%LOCALAPPDATA%\tauri\NSIS\Plugins\x86-unicode\additional\`.

Estas versiones son las que pide la CLI actual de Tauri. Si actualizas Tauri, pueden cambiar.

## Paso 3: comprobar

Desconecta internet y vuelve a ejecutar `npm run tauri:build`. Si termina, el equipo está listo.

## Uso diario

- `npm run tauri:build`: instalador, sin internet.
- `npm run tauri:dev`, `cargo test`: no necesitan WebView2 fijo ni NSIS.

## Cuándo hace falta internet otra vez

- **Añadir o actualizar una dependencia de Rust.** `src-tauri/.cargo/config.toml` deja Cargo sin conexión. Usa, por ejemplo:

```bash
cargo --config net.offline=false update --manifest-path src-tauri/Cargo.toml
```

- **Añadir una dependencia de npm**, o reinstalar `node_modules`.
- **Actualizar WebView2.** La copia fija no se actualiza sola: repite el paso 1 de vez en cuando y compila de nuevo.
- **Actualizar Tauri**, porque puede pedir otra versión de NSIS.

## Por qué una copia fija

Con `offlineInstaller`, Tauri consulta a Microsoft en cada compilación, así que no sirve sin internet. La copia fija está en el disco, se empaqueta con la app y el cliente no instala WebView2 aparte.

Solo se usa en `tauri:build`, mediante `src-tauri/tauri.release.conf.json`. Si estuviera en `tauri.conf.json`, cada `cargo test` y `tauri:dev` copiaría 200 MB, y fallaría si falta la carpeta.
