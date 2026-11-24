## Context

`menu_draw` compone en `cv` (256x224) y `present` lo sube como textura. Todas las constantes de disposición (`PX`, `PY`, `PW`, `ROW_Y`, `ROW_H`, `VAL_R`, `CTL_*`...) están en esos píxeles. La entrada táctil y el ratón convierten la posición de la ventana a ese lienzo (`menu_canvas_pt`, `mouse_click`). `ui_*` y `text_*` en `play_ui.h` dibujan con la fuente de 5x7 engrosada a 1x.

## Goals / Non-Goals

**Goals:** texto legible y proporcionado en cualquier tamaño de ventana; más aire entre filas; Controles con lista, mando y detalle a la vez; el mismo código en Windows, Linux y Android; las pruebas sin ventana siguen siendo deterministas.

**Non-Goals:** cambiar qué opciones hay en cada pestaña; temas de color; animaciones nuevas.

## Decisions

1. **Lienzo = tamaño de la ventana (o de `--present-size` sin ventana).** Se reserva un búfer RGBA del tamaño de la superficie y se rehace al cambiar el tamaño. Alternativa descartada: un lienzo fijo de 1920x1080 reescalado, porque vuelve a mezclar píxeles de interfaz con píxeles de pantalla.
2. **Escala de interfaz entera `ui_scale = max(1, alto_ventana / 270)`.** La disposición se escribe en unidades y se multiplica por `ui_scale`; el texto se dibuja con la fuente a esa escala entera. Así las posiciones tienen precisión de `ui_scale` píxeles y la fuente de píxeles no se emborrona. Alternativa descartada: escala fraccionaria con filtrado, porque emborrona la fuente.
3. **Fuente: se mantiene la de píxeles, a escala entera, en esta fase.** Una fuente de contorno (por ejemplo `stb_truetype` más una fuente con licencia abierta incrustada) da texto fino, pero añade una dependencia, una licencia, un atlas de glifos y trabajo en Android. Se decide tras ver la fase 1 con la fuente actual; el contrato de `text_w`/`text_draw` no cambia, así que cambiar la fuente después es local.
4. **Disposición por secciones con rectángulos**: cada pestaña pide su zona (cabecera, cuerpo, pie) en vez de coordenadas sueltas; las filas se reparten por la altura disponible y la lista se desplaza (rueda del ratón y arrastre) si no cabe.
5. **Entrada en coordenadas de ventana**: `menu_tap_canvas` pasa a recibir píxeles de ventana y las zonas pulsables salen del mismo cálculo que el dibujo (una lista de «zonas» que `menu_draw` rellena y el ratón consulta; una sola fuente de verdad).
6. **Pruebas**: `--menu-shot` con `--present-size WxH` fijo (por defecto 1280x720), hashes regenerados, y una prueba que comprueba que en 640x360, 1280x720 y 1920x1080 ningún texto se sale de su zona.

## Risks / Trade-offs

- [Es un cambio grande que toca todo el menú] → por fases: lienzo y escala (1), Controles (2), el resto de pestañas (3), selector de ranuras y menú del móvil (4); cada fase deja el menú usable.
- [El menú del móvil y el de escritorio divergen] → comparten zonas y escala desde la fase 1.
- [Rendimiento: dibujar ~2 Mpíxeles por fotograma con el menú abierto] → solo mientras está abierto, y solo se redibuja al cambiar (cursor, valor, parpadeo cada 10 fotogramas).
