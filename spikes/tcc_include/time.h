/* time.h minima para TinyCC en un dispositivo sin cabeceras del sistema: solo time() (time_t es long en arm64 y x86_64). Codigo propio. */
#ifndef SNES_TCC_TIME_H
#define SNES_TCC_TIME_H
typedef long time_t;
time_t time(time_t *t);
#endif
