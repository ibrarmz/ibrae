<h1 align="center">IBRÆ</h1>
<p align="center"><em>A different layer of your desktop.</em><br><sub>DESIGNED BY IBRA · EST. 2026</sub></p>

<p align="center"><b>Español</b> · <a href="README.en.md">English</a></p>

<p align="center">
  <a href="https://github.com/ibrarmz/ibrae/releases/latest"><img alt="Descargar" src="https://img.shields.io/github/v/release/ibrarmz/ibrae?style=for-the-badge&label=Descargar&color=e07a5f"></a>
  <a href="https://github.com/sponsors/ibrarmz"><img alt="Apoyar en GitHub Sponsors" src="https://img.shields.io/badge/Apoyar-GitHub%20Sponsors-ea4aaa?style=for-the-badge&logo=githubsponsors&logoColor=white"></a>
  <a href="https://github.com/ibrarmz/ibrae/stargazers"><img alt="Estrellas" src="https://img.shields.io/github/stars/ibrarmz/ibrae?style=for-the-badge&color=f5c84b"></a>
  <a href="LICENSE"><img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-8fd6a0?style=for-the-badge"></a>
</p>

https://github.com/user-attachments/assets/b2391a25-c06f-46eb-bb2a-e74e7af3be1c

Una isla dinámica para **Windows y Linux** que vive arriba de tu pantalla: lo que suena, lo que hace Claude Code, tu próxima reunión y tus herramientas del día, a un vistazo. Gratis y de código abierto.

**[Descargar la última versión](https://github.com/ibrarmz/ibrae/releases/latest)** · Windows 10/11, Debian/Ubuntu, Fedora · [♥ Apoyar](https://github.com/sponsors/ibrarmz)

## Qué hace

- **Música** de cualquier reproductor (Spotify, YouTube en el navegador, Apple Music, VLC…): carátula, controles y barra de progreso. Opcional: carátulas en HD.
- **Claude Code** en vivo: qué fase lleva (leer, editar, probar), el diff que está escribiendo, sus subagentes, y se asoma cuando termina. Cuando **pide permiso**, contestas desde la isla: **Permitir**, **Siempre** o **Denegar**, sin volver a la terminal.
- **Volumen y brillo** estilo Mac: al subir o bajar el volumen, silenciar, cambiar el brillo o conectar audífonos, la píldora lo muestra. En Sistema → *Sonido y pantalla* hay barras de volumen y brillo y cambias la salida de audio con un clic. En Linux funciona con PipeWire o PulseAudio (`pactl`), y el brillo solo con la pantalla de una laptop.
- **Calendario** de Google, Outlook o iCloud (link iCal, sin iniciar sesión): tu agenda del día, «Daily en 12 min» en la píldora, aviso un minuto antes y botón **Unirse** a Meet, Zoom o Teams.
- **Focus** (pomodoro), **notas rápidas** y **sistema** (CPU, memoria, disco, batería).
- **Varios monitores:** en la pantalla que elijas, o siguiendo al cursor de un monitor a otro.
- **No estorba:** sin actividad es una línea de 64 px; si el cursor solo pasa, se vuelve transparente y los clics atraviesan; se esconde con ventanas maximizadas.
- **Ligera y privada:** ~150 MB de RAM, casi 0 % de CPU en reposo, sin cuentas ni servidores propios ([qué se conecta a internet](#privacidad-qué-se-conecta-a-internet)).
- **Se actualiza sola** con firma verificada.

<p align="center"><img src="docs/demo.gif" alt="La isla: música en la píldora, Claude Code trabajando y pidiendo permiso, aviso de reunión y los paneles de música, Claude y calendario" width="760"></p>

Hecha con **Tauri 2 + Rust + Svelte 5**. El nombre se escribe IBRÆ y se busca como *ibrae*.

## Instalar

Descarga el instalador de la [última versión](https://github.com/ibrarmz/ibrae/releases/latest).

| Sistema | Archivo | Notas |
| --- | --- | --- |
| Windows 10/11 | `IBRAE_<versión>_x64-setup.exe` | Recomendado. Instala por usuario, sin permisos de administrador. |
| Windows 10/11 | `IBRAE_<versión>_x64-portable.zip` | Si tu red bloquea descargas de `.exe`/`.msi`. Descomprime y abre `ibrae.exe`; no instala ni pide administrador. Instrucciones en `LEEME.txt`. |
| Windows 10/11 | `IBRAE_<versión>_x64.msi` | Para todo el equipo, en Program Files. Pide administrador; útil para desplegar con Intune / GPO (`msiexec /i … /qn`). |
| Linux (cualquier distro) | `ibrae_<versión>_amd64.AppImage` | Recomendado en Linux, y el único que se actualiza solo. Sin instalar: `chmod +x` y ábrelo, o intégralo al menú con Gear Lever (viene en Bazzite). |
| Debian / Ubuntu | `ibrae_<versión>_amd64.deb` | `sudo apt install ./ibrae_<versión>_amd64.deb` |
| Fedora | `ibrae-<versión>-1.x86_64.rpm` | `sudo dnf install ./ibrae-<versión>-1.x86_64.rpm` |

**Actualizaciones:** la isla revisa GitHub al arrancar y cada 12 horas. Con el instalador `.exe`, el `.msi` o el `.AppImage` se actualiza sola desde **Ajustes → Actualizaciones** (verifica la firma de la descarga antes de instalar; el AppImage se reemplaza y la isla se reinicia); con el `.zip` portable, el `.deb` o el `.rpm` te avisa y abre la página de descarga. Se puede apagar en Ajustes.

Usa solo una de las opciones de Windows. Los dos, al desinstalar, quitan los hooks de Claude y el inicio automático (`ibrae uninstall-cleanup`). No están firmados: si SmartScreen avisa, "Más información" → "Ejecutar de todas formas".

**¿Venías de "Dynamic Island"?** Es la misma app con nombre nuevo. En Windows, al abrir IBRÆ por primera vez (o al actualizar desde la isla) se quita la versión anterior y se conservan tus notas, ajustes, calendarios, la conexión con Claude Code y el inicio automático. En Linux quita el paquete viejo con `sudo apt remove dynamic-island` / `sudo dnf remove dynamic-island`.

Con el AppImage, si conectaste Claude Code, la isla copia `island-hook` a `~/.local/share/dev.dynamicisland.island/` (el AppImage cambia de ruta cada vez que se abre). Usa un solo formato en Linux: si cambias del `.rpm`/`.deb` al AppImage, desinstala el paquete primero (en Bazzite: `rpm-ostree uninstall ibrae`).

En Linux, antes de desinstalar, corre `ibrae hooks uninstall` si conectaste Claude Code y apaga "Iniciar con el sistema" en Ajustes: el paquete se desinstala como root y no puede tocar la configuración de tu usuario.

## Compilar

Requisitos: Node 20+ y Rust (toolchain MSVC en Windows).

```sh
npm install
npm run tauri dev                    # desarrollo con recarga en caliente
npx tauri build                      # instalador: src-tauri/target/release/bundle/nsis/*.exe (Windows)
```

`island-hook` se compila primero (`npm run build:hook`, lo hace solo `tauri build`) y el instalador lo incluye junto a la app.

**Linux** necesita las librerías de WebKitGTK:

```sh
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev pkg-config build-essential
# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel openssl-devel pkgconf-pkg-config
```

O sin instalar nada, los `.deb` y `.rpm` desde Docker (también desde Windows): ver `ci/linux-bundle.Dockerfile`.

La isla vive arriba de la pantalla principal; el ícono de bandeja tiene **Configuración**, **Mostrar / ocultar** y **Salir**. Solo corre una instancia: si ya hay una, la segunda se cierra sola.

## Publicar una versión

```sh
npm run release -- 0.1.2
```

Pone la versión en `package.json`, `tauri.conf.json` y `Cargo.toml`, hace commit, crea el tag `v0.1.2` y lo sube. GitHub Actions (`.github/workflows/release.yml`) compila el instalador de Windows y los paquetes `.deb`/`.rpm` y los publica en el release, en unos 15–20 minutos.

Para regrabar el GIF de arriba con datos de ejemplo (sin tu música, calendario ni sesiones): `scripts/record-demo.mjs`; las instrucciones están al inicio del archivo.

## Ajustes

Ícono de engrane en el dock, o bandeja → **Configuración**. Se guardan en `settings.json` dentro de la carpeta de configuración de la app.

- **Iniciar con el sistema** (registro de Windows / `~/.config/autostart` en Linux)
- **Posición:** izquierda, centro o derecha
- **Pantalla** (con más de un monitor): la principal, una fija (se recuerda por su nombre aunque reconectes cables; si la desconectas, la isla espera en la principal) o **donde esté el cursor**: se muda al monitor donde dejes el cursor un segundo. En Linux con Wayland, "donde esté el cursor" solo ve el cursor sobre apps X11.
- **Espera para abrir** al pasar el cursor, sin y con actividad (100 ms en 100 ms)
- **Modo fantasma** y **esconder con ventanas maximizadas**
- **Focus:** minutos de focus y de descanso (la regla del panel Focus también los guarda)
- **Secciones del dock** visibles
- **Volumen y brillo:** qué avisos salen en la píldora (volumen, brillo, salida de audio) y cuánto duran
- **Claude Code:** conectar / desconectar, si la isla se abre cuando pide permiso y si puedes contestar permisos desde la isla
- **Música:** carátulas en HD desde el catálogo de iTunes (apagado por defecto)
- **Calendario:** conectar calendarios por link iCal y cuándo avisar
- **Actualizaciones:** versión, buscar e instalar, y buscar automáticamente

### Conectar un calendario

Sección **Calendario** (o Ajustes → Calendario) → **Google Calendar** u **Outlook** abre la página donde está el link iCal. Cópialo, vuelve a la isla y aparece **Conectar**; para iCloud u otros, **Pegar link**. Es de solo lectura y sin iniciar sesión: el link se guarda solo en tu PC y se descarga cada 5 minutos.

- **Google:** «Dirección secreta en formato iCal», no la pública (esa solo funciona si el calendario es público). En cuentas de trabajo el administrador puede ocultar la secreta; si solo puedes publicar la disponibilidad, la isla muestra los eventos como «Ocupado».
- **Outlook:** «Publicar un calendario» → link ICS.

### Conectar Claude Code

En la isla, sección **Claude** → **Conectar**. Eso agrega hooks a `~/.claude/settings.json` (guarda antes un respaldo `settings.json.bak-dynamic-island`) y aplica a las sesiones nuevas de Claude Code. También desde terminal: `ibrae hooks install` / `hooks uninstall`.

**Permisos desde la isla.** Cuando Claude pide permiso (un comando, editar un archivo, abrir una web…), la isla muestra qué quiere hacer y tres botones:
- **Permitir:** solo esta vez.
- **Siempre:** agrega la misma regla que el "no volver a preguntar" de la terminal (por ejemplo `Bash(npm test:*)`). Sale solo cuando Claude Code la ofrece.
- **Denegar:** Claude no lo hace y sigue sin eso.

La terminal te pregunta al mismo tiempo; vale la primera respuesta. Si contestas allá, la tarjeta desaparece sola. Las preguntas de opciones y la revisión del plan se contestan solo en la terminal. Si un subagente en segundo plano pide permiso, la isla espera 30 s y luego deja que pregunte la terminal. Se apaga en Ajustes → "Responder permisos desde la isla".

Los hooks llaman a `island-hook`, un ejecutable de 200 KB que solo reenvía el evento por un canal local (named pipe en Windows, socket Unix en Linux; nada de puertos de red). Van con `"async": true`, así que Claude nunca espera a la isla.

### Cómo se aparta

- **Sin actividad** es solo una línea de 64×5 px. Con música, focus o Claude trabajando muestra la píldora.
- **Modo fantasma** (Windows): si el cursor solo pasa por encima, la isla se desvanece al 15 % y los clics van a lo que hay debajo. Se abre si dejas el cursor quieto 0.4 s (0.6 s si hay algo activo) o si mantienes `Ctrl`. En Linux se abre igual con la espera o `Ctrl`, pero la isla misma no deja pasar clics.
- **Ventana maximizada o pantalla completa** en ese monitor: la isla sube y deja 2 px visibles. Se asoma sola cuando termina un focus o cuando Claude pide permiso.

## Privacidad: qué se conecta a internet

Nada sale de tu PC hacia un servidor del proyecto: no hay servidor. Las únicas conexiones son:

| Qué | A dónde | Cuándo |
| --- | --- | --- |
| Buscar actualizaciones | `github.com` (el `latest.json` del último release) | Al arrancar y cada 12 h; se apaga en Ajustes |
| Calendarios | El link iCal que tú agregaste | Cada 5 min, solo si agregaste alguno |
| Carátulas en HD | `itunes.apple.com` (artista y título de la canción) | Solo si lo activas en Ajustes |

Los hooks de Claude Code usan un canal local (named pipe / socket Unix), nunca la red.

## Estado

| Pieza | Windows | Linux |
| --- | --- | --- |
| Ventana transparente, siempre encima, clic a través | Probado | Compila; ventana creada en su lugar (Xvfb) |
| Píldora, expansión, modo fantasma, se esconde con ventanas maximizadas | Probado | Compila; sin probar en escritorio real |
| Inicio, Focus | Probado | Compila |
| Música | SMTC, probado | MPRIS, compila; sin probar |
| Claude: fases, diff, permisos, terminó | Probado con eventos simulados | Canal y `island-hook` probados en contenedor |
| Notas: nota rápida con guardado automático, botón **+** del dock | Probado (render) | Compila |
| Sistema: CPU con historial, memoria, disco, batería, tiempo encendido, RAM de la isla | Probado | Compila |
| Volumen, silencio, salida de audio y brillo (avisos y controles) | Probado | Volumen, silencio y salida probados en contenedor (PulseAudio); brillo sin probar |
| Ajustes, posición, autoarranque | Probado en la app instalada | Compila |
| Calendario iCal: agenda, avisos, Unirse, conectar copiando el link | Probado con Google (calendario real) | Compila |
| Actualización automática | Probada de 0.1.13 a 0.1.14 con el instalador `.exe` | Avisa y abre la descarga |
| Instalador / desinstalador | NSIS por usuario, probado instalar → usar → desinstalar | `.deb` / `.rpm` generados en Docker, sin instalar en un escritorio real; AppImage se actualiza solo (probado en Docker de 0.4.90 a 0.4.91) |

Las notas se guardan en `notes.md` dentro de la carpeta de datos de la app (`%APPDATA%\dev.dynamicisland.island` en Windows, `~/.local/share/dev.dynamicisland.island` en Linux). Mientras escribes, la isla no se cierra al sacar el cursor; se cierra al hacer clic en otra app, y en Windows le devuelve el teclado a la app donde estabas.

En Linux la isla corre por X11/XWayland en todos los escritorios (GNOME no permite a una app de Wayland colocarse sola ni quedarse encima). En una sesión Wayland, "ventana maximizada" solo detecta apps X11.

## Rendimiento medido (Windows 11, build release)

| Métrica | Valor |
| --- | --- |
| Binarios | 3.9 MB + 0.2 MB (`island-hook`) |
| RAM privada, árbol completo (app + WebView2) | ~140–170 MB (con todos los paneles; el renderer crece con el uso y baja al cerrar) |
| — de eso, el núcleo en Rust | ~10 MB |
| CPU en reposo | 0.3–1.3 % de un núcleo (el núcleo en Rust, ~0.05 %) |

La meta inicial de < 60 MB no se alcanza con WebView2: su piso son ~100 MB repartidos en 6 procesos. Lo aplicado para bajar desde 165 MB:

- Sin proceso de GPU (`--disable-gpu`): −55 MB. La isla es pequeña, el render por software basta.
- Un solo renderer, sin extensiones, sin crashpad, V8 en *lite mode*.
- El cursor se lee con `GetCursorPos` directo y el reloj solo marca segundos cuando algo los muestra.
- La música solo avisa a la UI cuando algo cambia de verdad.

Si se quiere bajar de 60 MB, el plan B sigue vigente: dibujar la píldora compacta en nativo y crear el WebView solo al expandir.

## Estructura

```
src/                       UI (Svelte 5)
  App.svelte               isla, expansión, dock, tinte del vidrio, hover en Linux
  lib/store.svelte.js      estado compartido; Rust empuja eventos
  lib/Pill|Home|Claude|Music|Focus|Notes|System|Settings.svelte
src-tauri/src/
  main.rs                  arranque, bandeja, CLI (hook / hooks install)
  hit.rs                   posición, clic a través, modo fantasma, ventanas maximizadas
  focus.rs                 temporizador
  claude/                  sesiones de Claude Code, canal local, instalador de hooks
  media/                   música: windows.rs (SMTC), linux.rs (MPRIS)
  notes.rs                 guardar / cargar la nota
  settings.rs              ajustes, inicio automático
src-tauri/hook/            island-hook: cliente mínimo que llama Claude Code
src-tauri/windows/hooks.nsh  limpieza al desinstalar (Windows)
  sys.rs                   estadísticas del sistema, RAM de la isla
ci/                        compilación y prueba de humo de Linux en Docker
```

`ISLAND_PEEK=home,claude,music,focus,notes,sys` abre cada panel al iniciar (útil para capturas).

## Apoyar

IBRÆ es gratis, sin anuncios y sin cuentas. Si te sirve y quieres que siga creciendo, puedes apoyarlo en [GitHub Sponsors](https://github.com/sponsors/ibrarmz) (una vez o cada mes). También ayuda mucho darle ⭐ al repo y contarle a alguien. En la isla está en Ajustes → **♥ Apoyar**.

## Licencia

[MIT](LICENSE): úsalo, cópialo, modifícalo y compártelo libremente, también en proyectos comerciales, conservando el aviso de copyright.
