## Why

La pestaña Controles del menú F1 ofrece siempre «Jugador 1» y «Jugador 2», aunque el juego sea de un solo jugador (ALttP lo es). El jugador 2 no hace nada y confunde. El motor no sabe cuántos jugadores tiene un juego: hay que decirlo en el perfil.

## What Changes

- El perfil acepta `[input] players = 1` (por defecto 2, como hoy, para no cambiar otros juegos).
- Con `players = 1` la pestaña Controles no muestra «Jugador N» ni el selector de jugador, el segundo mando y el segundo teclado no se asignan a ningún jugador, y `config.ini` no escribe `p2.*`.
- El generador emite `snes_game_players` en el C generado y el front lo lee.

## Impact

- `src/config.rs` (clave nueva y prueba), `src/codegen.rs` (constante), `runtime/play.c` (menú y entrada), el perfil de `alttp-recomp` (`[input] players = 1`), README y `CLAUDE.md`.
- Los juegos sin la clave se comportan como hasta ahora.
