**IBRÆ** · A different layer of your desktop.

## Descargas

| Sistema | Archivo |
| --- | --- |
| Windows 10/11 | `IBRAE_{{VERSION}}_x64-setup.exe`: recomendado; instala por usuario, sin permisos de administrador, y se actualiza sola |
| Windows 10/11 (portable) | `IBRAE_{{VERSION}}_x64-portable.zip`: **si tu empresa bloquea las descargas de .exe**; descomprime y abre `ibrae.exe`, sin instalar ni pedir administrador |
| Windows 10/11 (MSI) | `IBRAE_{{VERSION}}_x64.msi`: para todo el equipo; pide administrador, sirve para Intune / GPO |
| Linux (cualquier distro) | `ibrae_{{VERSION}}_amd64.AppImage`: recomendado; sin instalar y **se actualiza sola** desde la isla. Dale permiso de ejecución (`chmod +x`) o ábrelo con Gear Lever |
| Debian / Ubuntu | `ibrae_{{VERSION}}_amd64.deb`: `sudo apt install ./ibrae_{{VERSION}}_amd64.deb` |
| Fedora | `ibrae-{{VERSION}}-1.x86_64.rpm`: `sudo dnf install ./ibrae-{{VERSION}}-1.x86_64.rpm` |

Los instaladores se compilan automáticamente en GitHub y aparecen aquí unos minutos después de publicar la versión.

- **Windows:** usa solo una de las tres opciones. No están firmados; si SmartScreen avisa: "Más información" → "Ejecutar de todas formas". Con el instalador `.exe` o el `.msi`, las siguientes versiones se instalan desde la isla (Ajustes → Actualizaciones).
- **Linux:** la isla corre por X11/XWayland. Con el `.AppImage` las siguientes versiones se instalan desde la isla (Ajustes → Actualizaciones); con el `.deb` / `.rpm` te avisa y abre la descarga. Usa solo uno de los tres. Antes de desinstalar, corre `ibrae hooks uninstall` si conectaste Claude Code.
- **¿Venías de "Dynamic Island"?** Es la misma app con nombre nuevo. En Windows, al abrir IBRÆ por primera vez se quita la versión anterior y se conservan tus notas, ajustes, calendarios, la conexión con Claude Code y el inicio automático. En Linux, quita el paquete viejo: `sudo apt remove dynamic-island` o `sudo dnf remove dynamic-island`.

Para conectar Claude Code: en la isla, sección Claude → Conectar.

♥ IBRÆ es gratis y de código abierto. Si te sirve, puedes apoyarlo en [GitHub Sponsors](https://github.com/sponsors/ibrarmz).
