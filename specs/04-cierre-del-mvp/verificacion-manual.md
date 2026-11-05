# Verificación manual — Cierre del MVP

Guía de `task-04.md` §5.6 para `spec-01`, `spec-03` y `spec-04`. Marca cada casilla al comprobarla y anota lo que falle (pantalla, pasos, qué esperabas y qué pasó).

## Preparación

1. **Haz una copia de la carpeta de datos** antes de empezar. Está en `%APPDATA%`, en la carpeta de la aplicación (contiene `goyma.sqlite3`).
2. Arranca la app:

```bash
npm run tauri:dev
```

3. Ten dos usuarios: uno **Administrador** y uno **Contador**. Repite con ambos lo marcado con **(ambos)**.
4. Al final repite lo esencial con la versión instalada:

```bash
npm run tauri:build
```

## 1. Arranque y sesión (`spec-01`)

- [ ] La app abre sin errores y la base existente se actualiza sola (no pierde datos).
- [ ] Iniciar sesión, cerrar sesión y "recordar sesión" funcionan. **(ambos)**
- [ ] Una contraseña incorrecta da un mensaje claro, sin detalles técnicos.
- [ ] Recuperar la contraseña funciona.
- [ ] El Contador no ve Usuarios ni Registro de Actividad.
- [ ] Editar el perfil y cambiar la contraseña funcionan.

## 2. Toasts

- [ ] Guardar algo muestra un toast arriba a la derecha, con el nombre de lo afectado ("Cliente «Ana Pérez» creado.").
- [ ] Éxito e información se cierran a los ~4 s; advertencia a los ~7 s; error se queda hasta cerrarlo.
- [ ] Pasar el ratón por encima pausa el cierre.
- [ ] Un campo mal rellenado se marca junto al campo, además del toast.

## 3. Contactos y facturas (`spec-03`)

- [ ] Crear, editar, ver y eliminar un cliente y un proveedor. Eliminar muestra su toast.
- [ ] Compra con un producto existente y con un **producto nuevo**.
- [ ] Compra con **conversión** (por ejemplo, en Caja): el stock sube en la unidad del producto.
- [ ] Adjuntar un PDF a la compra con el diálogo y **arrastrándolo**. Descargarlo.
- [ ] Venta, servicio, garantía, insumos y pérdida: crear, ver, cambiar estado y eliminar.
- [ ] Venta con otra unidad: el detalle muestra lo escrito ("2 Caja") y el historial lo movido ("24 u").
- [ ] Cancelar una venta devuelve el stock y deja la Reversión en el historial.
- [ ] Vender más de lo que hay se rechaza con un mensaje claro.

## 4. Notificaciones y reservas

- [ ] Bajar un producto de 5 unidades da "stock bajo"; llegar a 0 da "agotado". No se repiten al seguir operando.
- [ ] Reponer el stock y volver a bajarlo avisa de nuevo.
- [ ] El contador de no leídas sube y baja al instante.
- [ ] Clic en una notificación la marca leída y abre el producto o la factura.
- [ ] "Marcar todas" y "Eliminar todas" funcionan.
- [ ] Reservar una venta: la fecha aparece en el detalle, en el listado y en la impresión.
- [ ] El filtro "Reservas activas" la encuentra y el detalle del producto muestra la cantidad reservada.
- [ ] **Vencimiento:** crea una reserva, cambia la fecha del equipo a después de la fecha límite (o edita la fecha) y reinicia. Debe quedar Cancelada, devolver el stock y avisar. Restaura la fecha del equipo.

## 5. Configuración

- [ ] Elegir monedas visibles: los equivalentes salen en el detalle del producto y en el dashboard.
- [ ] Guardar tasas: quedan en el historial con usuario y fecha.
- [ ] Datos de la empresa y logo PNG o JPG. Un archivo que no sea imagen se rechaza.
- [ ] Cambiar los umbrales reevalúa los avisos.
- [ ] El tamaño de página se conserva al reiniciar y es distinto por usuario. **(ambos)**

## 6. Impresión y PDF

- [ ] "Imprimir" abre la vista previa A4 en los seis tipos, con logo y datos de la empresa y **sin** el estado.
- [ ] Imprimir en papel (o "Guardar como PDF" del sistema) sale bien: márgenes, una sola factura, sin menús.
- [ ] "Exportar a PDF" guarda el archivo y se ve igual, con el logo real.
- [ ] Cancelar el diálogo no hace nada.

## 7. Dashboard

- [ ] Las tarjetas muestran cifras reales y coinciden con lo registrado (compruébalo a mano con una venta).
- [ ] Las siete opciones de período cambian las cifras. El rango pide las dos fechas.
- [ ] La gráfica dibuja bien en Hoy (un punto), Mes, 3 Meses y 1 Año; la leyenda oculta series.
- [ ] Notas, reservas y lento movimiento (con su paginador) **se ven bien**: es diseño nuevo sin probar.
- [ ] Al guardar algo y volver al dashboard, las cifras cambian.
- [ ] "Ver Agotados" y "Ver Descontinuados" abren su pestaña.

## 8. Borradores

- [ ] Escribe en un formulario, espera 2 s y **cierra la app a la fuerza** (Administrador de tareas). Al abrir sale "Tienes 1 formulario incompleto".
- [ ] Al abrir el formulario pregunta. **Continuar** recupera lo escrito; **Descartar** lo borra.
- [ ] Recupera una factura con líneas y contacto. Elimina antes uno de sus productos: avisa y lo quita.
- [ ] Guardar o Cancelar borra el borrador.
- [ ] La sección Borradores lista, abre y elimina. Un usuario no ve los del otro. **(ambos)**
- [ ] Ninguna contraseña aparece al recuperar el formulario de usuario.
- [ ] Tasas de cambio: escribir sin guardar y recuperar.

## 9. Registro de actividad

- [ ] Como Administrador: eliminar, cancelar, editar una factura y cambiar una tasa dejan su entrada.
- [ ] Filtros de fecha, usuario y acción, búsqueda y orden por fecha funcionan.
- [ ] La reserva vencida del punto 4 aparece con el usuario "Sistema".

## 10. CSV con Excel

- [ ] Exportar desde productos, categorías, unidades, clientes, proveedores y cada lista de facturas, con un filtro puesto: solo salen esas filas.
- [ ] **Abrir en Excel en español:** columnas separadas, tildes bien, decimales con coma, fechas `dd/mm/aaaa`.
- [ ] Un teléfono `+53…` no se convierte en fórmula.
- [ ] Guardar en Excel como **"CSV UTF-8"** e importarlo: los duplicados se omiten con su fila y motivo.
- [ ] Guardar como "CSV" normal e importarlo: se rechaza con un mensaje claro.
- [ ] Importar productos con una categoría y una unidad nuevas: se crean. Revisa la abreviatura de la unidad.
- [ ] Una importación grande (cerca de 5 000 filas): anota cuánto tarda y si la app se queda bloqueada.

## 11. Respaldo y restauración

- [ ] "Crear copia de seguridad" guarda `goyma-AAAA-MM-DD.sqlite3` y muestra el aviso de que no está cifrada.
- [ ] Al arrancar se crea la copia del día en `backups` y aparece en la lista.
- [ ] Restaurar con una contraseña incorrecta no toca nada.
- [ ] Elegir un archivo cualquiera se rechaza sin tocar nada.
- [ ] **Restaurar en otra carpeta de datos:** con una copia hecha antes de un cambio, restaura; vuelves al login y están los datos de la copia.
- [ ] La copia `goyma-previa-*` aparece en la lista y permite volver atrás.
- [ ] Recuerda: los PDF adjuntos y el logo no van en la copia.

## 12. Versión instalada (`tauri:build`)

- [ ] Instala y abre sin conexión a internet.
- [ ] Repite: iniciar sesión, una venta, imprimir, exportar PDF, CSV y crear una copia.
- [ ] Sin errores de CSP: el logo, la gráfica y los diálogos funcionan.

## Resultado

Cuando acabes, pásame lo que falló. Si todo está bien, marco la verificación en `task-04.md` y `Agents.md` y el MVP queda cerrado.
