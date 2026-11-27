## 1. Lienzo y escala

- [x] 1.1 (puente) el menú es una capa de la ventana, dibujada tras la imagen y sus efectos, con la capa de 256x224 ampliada por un factor entero `min(alto/270, ancho/256)` y el juego atenuado alrededor; en GL y en el renderizador. Pendiente: el lienzo con más unidades que 256x224 (fase 3, para el aire)
- [ ] 1.2 `text_*`/`ui_*` con escala entera; medidas en unidades
- [x] 1.3 Ratón y toque en coordenadas de ventana (`layer_pt`); pendiente: lista de zonas pulsables compartida con el dibujo
- [x] 1.4 `--present-shot` con `--present-size` ya muestra la capa; `--menu-in-picture` conserva el menú dentro de la imagen para las pruebas de los efectos

## 2. Controles (primera pestaña en el lienzo nuevo)

- [x] 2.1 Lista, mando SNES, detalle y selector Teclado/Mando con el espacio nuevo (capa de 256x270, filas de 13)
- [x] 2.2 Número de jugadores del perfil (`profile-players`): sin selector de jugador en juegos de uno

## 3. Resto de pestañas

- [x] 3.1 (filas de 14, descripción de la fila elegida sobre el pie; el pie con las teclas ya existía) Imagen, Audio, Juego, Táctil: filas con aire, valores alineados, descripción de la fila elegida, pie con teclas
- [x] 3.2 Desplazamiento de listas largas (escritorio: 13 filas, sigue al cursor, rueda del ratón, flechas al borde; el táctil ya lo tenía)

## 4. Selector de ranuras y menú del móvil

- [ ] 4.1 `slot_draw` en el lienzo nuevo
- [ ] 4.2 `mm_*` (Android) sobre las mismas zonas y escala

## 5. Cierre

- [ ] 5.1 Prueba de que nada se sale de su zona en 640x360, 1280x720 y 1920x1080
- [ ] 5.2 README y `CLAUDE.md` (descripción del menú); actualizar `menu-ui`
