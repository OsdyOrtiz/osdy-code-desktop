//! Spanish text of Settings (agents, models, devices, integrations, notifications, extensions).

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Agents.
        "Agents" => "Agentes",
        "Permissions (new sessions)" => "Permisos (sesiones nuevas)",
        "Background subagents" => "Subagentes en segundo plano",
        "Send helpers to the background" => "Enviar subagentes a segundo plano",
        "Off" => "Desactivado",
        "On" => "Activado",
        "off" => "desactivado",
        "on" => "activado",
        "project file" => "archivo del proyecto",
        "global file" => "archivo global",
        "default" => "predeterminado",
        "(malformed)" => "(formato no válido)",
        "Turn on" => "Activar",
        "Turn off" => "Desactivar",
        "Cancel" => "Cancelar",
        "On: el Gentleman launches helpers in the background and keeps working; each answer arrives when it finishes. Applied by Gentle Shell (/gentle:background-subagents), so the terminal sees the same setting." => {
            "Al activarlo, el Gentleman lanza subagentes en segundo plano y sigue trabajando. Cada subagente responde cuando termina. Gentle Shell aplica el mismo ajuste en la terminal (/gentle:background-subagents)."
        }
        "{file} decides for this project, so the switch (global) won't change it here." => {
            "{file} define el ajuste para este proyecto; el interruptor global no lo cambia aquí."
        }
        "Gentle Shell applied it. Helpers launched from now on use it." => {
            "Gentle Shell aplicó el ajuste. Los subagentes nuevos lo usarán."
        }
        "Gentle Shell applied it; what decides here did not change." => {
            "Gentle Shell aplicó el ajuste, pero la fuente que lo define aquí no cambió."
        }
        "Gentle Shell applied it; its report is in 🔔. Helpers launched from now on use it." => {
            "Gentle Shell aplicó el ajuste. Puedes ver el informe en 🔔. Los subagentes nuevos lo usarán."
        }
        "Not changed: {error}" => "No se aplicó el cambio: {error}",
        "Agent UI" => "Interfaz del agente",
        "Shows UIs" => "Muestra interfaces",
        "Proactive" => "Proactivamente",
        "When asked" => "Si lo pides",
        "The agent shows a diagram, table, form, checklist, chart or review on its own when one clearly beats a text answer: a system with several parts, a plan of four or more steps, options to compare, several questions at once, risky edits. Never for short answers, and at most one new UI per reply." => {
            "El agente muestra por iniciativa propia un diagrama, una tabla, un formulario, una lista, un gráfico o una revisión si es más útil que el texto: para un sistema con varias partes, un plan de cuatro pasos o más, opciones para comparar, varias preguntas a la vez o cambios arriesgados. Nunca lo hace en respuestas breves y muestra como máximo una interfaz nueva por respuesta."
        }
        "Only when you ask for one (\"draw it\", \"as a table\", \"let me review the changes first\"), or to update one already shown." => {
            "Solo si lo pides («dibújalo», «en una tabla», «déjame revisar los cambios primero») o si actualiza una interfaz que ya mostró."
        }
        "The agent has no gentle_ui tool. UIs already in a conversation stay and your edits still reach it, but it can't change them; Insert UI template still works." => {
            "El agente no tiene la herramienta gentle_ui. Las interfaces que ya están en la conversación se conservan y tus cambios le llegan, pero el agente no puede modificarlas. Aún puedes insertar plantillas de interfaz."
        }
        "Every agent tab follows it from its next turn. Saved in the app's settings.json (genuiMode)." => {
            "El ajuste se aplica en cada pestaña de agente a partir de su próximo turno. Se guarda en el settings.json de la app (genuiMode)."
        }

        // Models and Claude bridge.
        "Model defaults" => "Modelos predeterminados",
        "Default model" => "Modelo predeterminado",
        "first signed-in model" => "primer modelo de un proveedor con sesión iniciada",
        "Default effort" => "Esfuerzo predeterminado",
        "Scoped models" => "Modelos habilitados",
        "all" => "todos",
        "Profiles & providers" => "Perfiles y proveedores",
        "Active profile" => "Perfil activo",
        "Profiles" => "Perfiles",
        "Sign-ins" => "Sesiones iniciadas",
        "none" => "ninguno",
        "Custom providers" => "Proveedores personalizados",
        "Model and sign-in details are read-only here. Use Profiles to edit agent routing and defaults." => {
            "Aquí solo puedes consultar los modelos y las sesiones iniciadas. Usa Perfiles para editar la asignación de agentes y los valores predeterminados."
        }
        "Manage profiles" => "Administrar perfiles",
        "Claude bridge" => "Puente de Claude",
        "Package" => "Paquete",
        "pi-claude-bridge installed" => "pi-claude-bridge instalado",
        "checking…" => "Comprobando…",
        "pro (default; set max for Opus 1M)" => "pro (predeterminado; elige max para Opus 1M)",
        "Claude bridge plan set to {plan}. New sessions use it." => {
            "El puente de Claude ahora usa el plan {plan}. Se aplica a las sesiones nuevas."
        }
        "Could not write claude-bridge.json: {error}" => {
            "No se pudo escribir claude-bridge.json: {error}"
        }

        // Devices and browser.
        "Configure Maestro UI testing" => "Configurar pruebas de interfaz con Maestro",
        "Devices" => "Dispositivos",
        "iOS simulators and Android emulators inside the app: live, touchable, docked at the right or floating. Agents open them here too, never in a separate Simulator or emulator window." => {
            "Usa simuladores de iOS y emuladores de Android interactivos dentro de la app. Puedes acoplarlos a la derecha o dejarlos flotantes. Los agentes también los abren aquí, sin usar una ventana externa de Simulator o del emulador."
        }
        "Looking at this machine…" => "Comprobando este equipo…",
        "{package} (installs when you turn this on)" => {
            "{package} (se instala al activar la opción)"
        }
        "Node.js {version}: 22.12 or newer is needed" => {
            "Node.js {version}: necesitas la versión 22.12 o posterior"
        }
        "Not found: Setup & requirements installs Node.js" => {
            "No se encontró Node.js. Puedes instalarlo en Configuración inicial y requisitos"
        }
        "Ready (Xcode)" => "Listo (Xcode)",
        "Not found: install Android Studio and create a virtual device" => {
            "No se encontró: instala Android Studio y crea un dispositivo virtual"
        }
        "Use devices" => "Usar dispositivos",
        "Open simulators and emulators in the device panel. {package}" => {
            "Abre simuladores y emuladores en el panel de dispositivos. {package}"
        }
        "Agents can use devices" => "Los agentes pueden usar dispositivos",
        "New agent sessions can open and drive these devices (gentle-device, agent-device). {package}" => {
            "Las sesiones nuevas de agentes pueden abrir y controlar estos dispositivos (gentle-device, agent-device). {package}"
        }
        "iOS simulators" => "Simuladores de iOS",
        "Android SDK" => "SDK de Android",
        "Not found: Android emulator video needs it" => {
            "No se encontró: el video del emulador de Android lo necesita"
        }
        "Install ffmpeg ({command})" => "Instalar ffmpeg ({command})",
        "Installing the device hub" => "Instalando el concentrador de dispositivos",
        "Installing agent-device" => "Instalando agent-device",
        "New agent sessions use it; open tabs pick it up after /reload." => {
            "El ajuste se aplica a las sesiones nuevas. En las pestañas abiertas, usa /reload."
        }
        "Browser" => "Navegador",
        "Open pages" => "Abrir páginas",
        "Inside the app" => "Dentro de la app",
        "In my browser" => "En mi navegador",
        "Where the pages agents open go: a dev server, a preview, the design canvas. Pages agents drive (gentle-browser) always stay inside the app." => {
            "Dónde se abren las páginas que muestran los agentes: un servidor de desarrollo, una vista previa o el lienzo de diseño. Las páginas que controlan mediante gentle-browser siempre quedan dentro de la app."
        }
        "Choose In my browser to pick which browser opens them; ↗ on a page opens your system's default browser." => {
            "Elige «En mi navegador» para decidir en qué navegador se abren. El botón ↗ de una página la abre en el navegador predeterminado del sistema."
        }
        "Which browser · it comes to the front with each page; ↗ uses it too" => {
            "Navegador · pasa al frente con cada página; ↗ también lo usa"
        }
        "Looking for browsers…" => "Buscando navegadores…",
        "System default" => "Predeterminado del sistema",
        "{browser} (not found)" => "{browser} (no encontrado)",

        // Integrations.
        "MCP servers" => "Servidores MCP",
        "Configured servers" => "Servidores configurados",
        "Detected from Gentle Shell's MCP configuration; edit the server definitions in the terminal configuration." => {
            "Estos servidores vienen de la configuración MCP de Gentle Shell. Puedes editarlos en la configuración de la terminal."
        }
        "Configure devices" => "Configurar dispositivos",
        "UI testing · Maestro" => "Pruebas de interfaz · Maestro",
        "Lets the agent tap, type, swipe, check and take screenshots on Android emulators, iOS simulators and web pages (Chromium, beta), and run Maestro flows. Everything installs into this app's own folder, without your password." => {
            "Permite al agente tocar, escribir, deslizar, verificar y tomar capturas en emuladores de Android, simuladores de iOS y páginas web (Chromium, beta), además de ejecutar flujos de Maestro. Todo se instala en la carpeta de la app, sin pedirte la contraseña."
        }
        "Looking for Maestro and Java…" => "Buscando Maestro y Java…",
        "Ready: the agent has the maestro tools" => {
            "Listo: el agente tiene las herramientas de Maestro"
        }
        "Installed, not connected to the agent" => "Instalado, sin conexión al agente",
        "Connected, but the CLI is missing" => "Conectado, pero falta la CLI",
        "Not installed" => "Sin instalar",
        "Maestro CLI" => "CLI de Maestro",
        "Not installed (about 350 MB)" => "Sin instalar (aprox. 350 MB)",
        "Not found: installs Temurin 21 (about 45 MB) with Maestro" => {
            "No se encontró Java. Maestro instala Temurin 21 (aprox. 45 MB)"
        }
        "Agent tools" => "Herramientas del agente",
        "Connected (open tabs reload when idle)" => {
            "Conectado (las pestañas se recargan cuando quedan inactivas)"
        }
        "Not connected" => "Sin conexión",
        "Connect to the agent" => "Conectar al agente",
        "Install Maestro" => "Instalar Maestro",
        "Install" => "Instalar",
        "Test connection" => "Probar conexión",
        "Testing the connection" => "Probando la conexión",
        "Disconnect" => "Desconectar",
        "Remove" => "Eliminar",
        "Disconnect and delete the Maestro and Java this app installed" => {
            "Desconecta y elimina las copias de Maestro y Java que instaló la app"
        }
        "Docs" => "Documentación",
        "Check again" => "Volver a comprobar",
        "{action} done." => "{action}: listo.",
        "Music · gentle-stream" => "Música · gentle-stream",
        "Endless music composed live while you work, steered by a mood you describe (\"rainy night in a small café\"). The same player as /gentle:music, from the ♪ button in the title bar. It installs into this app's own folder." => {
            "Música continua compuesta en vivo mientras trabajas, a partir del ambiente que describas («noche lluviosa en un pequeño café»). El botón ♪ de la barra de título abre el mismo reproductor que /gentle:music. Se instala en la carpeta de la app."
        }
        "gentle-stream runs on macOS and Linux (and WSL)." => {
            "gentle-stream funciona en macOS, Linux y WSL."
        }
        "Looking for gentle-stream…" => "Buscando gentle-stream…",
        "Ready: ♪ is in the title bar" => "Listo: el botón ♪ está en la barra de título",
        "Found, but too old for the app (no ctl --json): install the current one" => {
            "La versión instalada es demasiado antigua (no admite ctl --json). Instala la versión actual"
        }
        "Player" => "Reproductor",
        "Not installed (Python 3.13 and the player, about 150 MB)" => {
            "Sin instalar (Python 3.13 y el reproductor, aprox. 150 MB)"
        }
        "Instruments" => "Instrumentos",
        "Sampled (FluidR3_GM soundfont)" => "Con muestras (banco de sonidos FluidR3_GM)",
        "Synthesized; sampled ones are an optional 120 MB download" => {
            "Sintetizados. Descarga opcional de muestras: 120 MB"
        }
        "Install gentle-stream" => "Instalar gentle-stream",
        "Sampled instruments" => "Instrumentos con muestras",
        "Download the FluidR3_GM soundfont (about 120 MB, once)" => {
            "Descarga el banco de sonidos FluidR3_GM una sola vez (aprox. 120 MB)"
        }
        "Stop the music and delete what this app installed" => {
            "Detiene la música y elimina lo que instaló la app"
        }
        "Every open session as a floor of its project's building: tasks, talk, artifacts, cost and permissions. Gentle Shell 4.9.0 was published without Tower's web app; Enable downloads that version's source from GitHub and builds it into your install (needs a few minutes and an internet connection)." => {
            "Cada sesión abierta ocupa un piso en el edificio de su proyecto: tareas, conversaciones, archivos generados, costos y permisos. Gentle Shell 4.9.0 salió sin la app web de Tower. Al activarla, se descarga el código de esa versión desde GitHub y se compila en tu instalación. Necesitas unos minutos y conexión a internet."
        }
        "Gentle Tower is ready: open it from the tab bar's + or with /gentle:tower (a Tower tab showing the error: Try again)." => {
            "Gentle Tower está listo. Ábrelo con + en la barra de pestañas o con /gentle:tower. Si una pestaña de Tower muestra el error, vuelve a intentarlo."
        }
        "Looking for Gentle Shell…" => "Buscando Gentle Shell…",
        "Gentle Shell not found" => "No se encontró Gentle Shell",
        "Ready" => "Listo",
        "Web app missing in this Gentle Shell" => {
            "Esta instalación de Gentle Shell no tiene la app web"
        }
        "Enable Gentle Tower" => "Activar Gentle Tower",
        "Building Gentle Tower…" => "Compilando Gentle Tower…",

        // Notifications.
        "Notifications" => "Notificaciones",
        "File" => "Archivo",
        "Context threshold" => "Umbral de contexto",
        "{percent} % (fixed by Gentle Shell)" => "{percent} % (fijado por Gentle Shell)",
        "Sounds" => "Sonidos",
        "Master switch, off by default. Off also stops the Dock bounce / taskbar flash while the app is in the background." => {
            "Interruptor general, desactivado de forma predeterminada. Al desactivarlo, el icono del Dock no rebota y la barra de tareas no parpadea cuando la app está en segundo plano."
        }
        "Turn finished" => "Turno terminado",
        "Needs you" => "Requiere tu atención",
        "Context high" => "Contexto alto",
        "When a turn finishes (agent.completed)." => "Cuando termina un turno (agent.completed).",
        "When el Gentleman asks you something (agent.attention)." => {
            "Cuando el Gentleman te pregunta algo (agent.attention)."
        }
        "When the context reaches the threshold (context.high)." => {
            "Cuando el contexto alcanza el umbral (context.high)."
        }
        "notifications.json isn't valid for Gentle Shell, so sounds are off. A change here replaces it with the default preset." => {
            "Gentle Shell no reconoce el formato de notifications.json, así que los sonidos están desactivados. Si haces un cambio aquí, se reemplazará el archivo por la configuración predeterminada."
        }
        "notifications.json can't be read, so sounds are off. A change here rewrites it with the default preset." => {
            "No se puede leer notifications.json, así que los sonidos están desactivados. Si haces un cambio aquí, se reescribirá el archivo con la configuración predeterminada."
        }
        "Same file as /gentle:customize in the terminal. Gentle Shell plays nothing for app tabs (RPC), so the app plays the same tones itself; other events (failures, subagents) keep their terminal settings." => {
            "Es el mismo archivo que usa /gentle:customize en la terminal. Gentle Shell no reproduce sonidos en las pestañas de la app (RPC), así que la app reproduce los mismos tonos. Otros eventos (errores, subagentes) conservan los ajustes de la terminal."
        }
        "Gentle Shell answered something else." => "Gentle Shell dio una respuesta inesperada.",
        "Not saved: notifications changed in Gentle Shell meanwhile. Showing the latest; choose again." => {
            "No se guardó el cambio: las notificaciones cambiaron en Gentle Shell. Ya tienes los ajustes más recientes; vuelve a elegir."
        }
        "Not saved: {message}" => "No se guardó: {message}",
        "Could not write notifications.json: {error}" => {
            "No se pudo escribir notifications.json: {error}"
        }
        "Gentle Shell offered no way to save this." => {
            "Gentle Shell no permite guardar este ajuste."
        }

        // Extensions.
        "Packages" => "Paquetes",
        "1 package has an update" => "Hay una actualización para 1 paquete",
        "{count} packages have updates" => "Hay actualizaciones para {count} paquetes",
        "Update packages" => "Actualizar paquetes",
        "Extension filters" => "Filtros de extensiones",
        "Gentle Shell couldn't read Pi's package list, so updates weren't checked" => {
            "Gentle Shell no pudo leer la lista de paquetes de Pi y no comprobó si hay actualizaciones"
        }
        "Pi's check reported no package updates" => {
            "Pi no informó de actualizaciones para los paquetes"
        }

        // Reading desk.
        "Reading desk" => "Mesa de lectura",
        "Previewing · click or Enter to apply" => {
            "Vista previa · haz clic o pulsa Enter para aplicar"
        }
        "Live preview" => "Vista previa en vivo",
        "Interface" => "Interfaz",
        "Code" => "Código",
        "Terminal" => "Terminal",
        "Review the latest changes" => "Revisa los últimos cambios",
        "Résumé ready · 3 files · café, acción, 0123" => {
            "Resumen listo · 3 archivos · café, acción, 0123"
        }
        "View changes" => "Ver cambios",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};

    #[test]
    fn i18n_translates_settings_area_b_but_preserves_english() {
        for (english, spanish) in [
            ("Shows UIs", "Muestra interfaces"),
            ("Model defaults", "Modelos predeterminados"),
            (
                "Agents can use devices",
                "Los agentes pueden usar dispositivos",
            ),
            ("Configured servers", "Servidores configurados"),
            ("Context threshold", "Umbral de contexto"),
            ("Update packages", "Actualizar paquetes"),
        ] {
            assert_eq!(tr(Language::Spanish, english), spanish, "{english}");
            assert_eq!(tr(Language::English, english), english);
        }
    }
}
