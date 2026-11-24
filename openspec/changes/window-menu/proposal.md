## Why

El menú F1 se dibuja hoy en un lienzo de 256x224 píxeles (`fb_menu`, `Canvas cv`) que luego se amplía a la ventana con la imagen del juego de fondo. Con una ventana de 1900x1000 cada píxel del menú mide 7 píxeles de pantalla: el texto se ve enorme, las filas van apretadas (11 px de alto), las pestañas con muchas opciones no caben (Controles tiene 13 filas, un mando dibujado y una caja de detalle en 236 px) y nada se puede colocar con holgura. Un menú actual (como el de configuración de botones de un juego moderno: lista a la izquierda, ilustración y detalle a la derecha, pie con las teclas) necesita espacio real.

## What Changes

- El menú se dibuja a la **resolución de la ventana** (no a 256x224): el lienzo tiene el tamaño de la superficie de presentación y se redibuja al cambiar de tamaño.
- La disposición se calcula en unidades de pantalla con una **escala de interfaz** que sale de la altura de la ventana (por ejemplo, altura/270, mínimo 1), de modo que el texto mide lo mismo en una ventana pequeña que en pantalla completa y deja de ser un bloque de 7 px.
- La fuente conserva su aspecto (la de píxeles del proyecto) pero se dibuja a escala entera elegida por la interfaz (2x, 3x, 4x...) con una tabla de métricas propia para colocar el texto con medidas finas; queda abierto cambiarla por una fuente de contorno (vectorial) en una fase posterior (decisión 3 de `design.md`).
- Cada pestaña se recoloca con la holgura nueva: más espacio entre filas, una columna de valores alineada, una descripción de una línea para la fila elegida y un pie con las teclas o botones.
- El ratón y el toque trabajan en coordenadas de ventana (se acaba convertir a 256x224 y de vuelta).
- La imagen del juego sigue de fondo oscurecido. El menú del móvil (`mm_*`, Android) se revisa con el mismo lienzo.

## Capabilities

### Modified Capabilities

- `menu-ui`: la tipografía deja de ser «una fuente de píxeles de caja fija a 1x sobre 256x224»; pasa a escala entera sobre el lienzo de la ventana. Los requisitos de pestañas, filas con valor y recorte con «...» se mantienen, en unidades de pantalla.

## Impact

- `runtime/play.c` (`menu_draw`, `menu_row`, `ctl_*`, `menu_tap_canvas`, `mouse_click`, `slot_*`), `runtime/play_ui.h` (`Canvas`, `text_*`, `ui_*`), `runtime/play_touch.h` (placas), `runtime/play_state.h` (selector de ranuras `slot_draw`), y las pruebas `--menu-shot` / `--menu-keys tap@X:Y` (las coordenadas pasan a ser de ventana; el tamaño de la captura se fija con `--present-size`).
- Los hashes de `--menu-shot` cambian en todas las pestañas: las pruebas con hash se regeneran en este cambio.
- Android, Linux y Windows comparten el código del menú; el menú del móvil (`mm_*`) se adapta a la vez.
