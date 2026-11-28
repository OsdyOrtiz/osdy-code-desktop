//! Spanish text of Settings (rail, pages, appearance, general, updates, about).

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        "Appearance" => "Apariencia",
        "Reading your Gentle Shell configuration…" => "Leyendo la configuración de Gentle Shell…",
        "Configuration" => "Configuración",
        "Manage Gentle Shell and the desktop experience" => {
            "Configura Gentle Shell y la app de escritorio"
        }
        "Reload and check for updates" => "Recargar y buscar actualizaciones",
        "Find a setting…" => "Buscar un ajuste…",
        "General" => "General",
        "Agents" => "Agentes",
        "Models" => "Modelos",
        "Devices" => "Dispositivos",
        "Integrations" => "Integraciones",
        "Notifications" => "Notificaciones",
        "Updates" => "Actualizaciones",
        "Extensions" => "Extensiones",
        "About" => "Acerca de",
        "Environment and setup" => "Entorno y configuración",
        "Theme and window appearance" => "Tema y apariencia de la ventana",
        "Agent behavior and background work" => {
            "Comportamiento de los agentes y tareas en segundo plano"
        }
        "Model and profile defaults" => "Modelos y perfiles predeterminados",
        "Development devices and the browser" => "Dispositivos de desarrollo y navegador",
        "Companions and external tools" => "Complementos y herramientas externas",
        "Alerts and notification preferences" => "Alertas y preferencias de notificación",
        "Application and tool updates" => "Actualizaciones de la app y las herramientas",
        "Packages and extension management" => "Administración de paquetes y extensiones",
        "Version, license and credits" => "Versión, licencia y créditos",
        "Clear settings search" => "Borrar la búsqueda de ajustes",
        "No matching settings. Try another term." => {
            "No se encontraron ajustes. Prueba otro término."
        }
        "Categories" => "Categorías",
        "Choose a configuration category" => "Elige una categoría de configuración",
        "Category: {category}" => "Categoría: {category}",
        "Setup & requirements" => "Configuración inicial y requisitos",
        "Home" => "Directorio principal",
        "Agent dir" => "Carpeta del agente",
        "Config dir" => "Carpeta de configuración",
        "Launcher" => "Ejecutable",
        "not found" => "No se encontró",
        "Open agent dir" => "Abrir carpeta del agente",
        "Open config dir" => "Abrir carpeta de configuración",
        "Status" => "Estado",
        "Needs attention" => "Requiere atención",
        "Run /gentle:doctor in a session to repair it." => {
            "Ejecuta /gentle:doctor en una sesión para repararlo."
        }
        "Terminal appearance" => "Apariencia de la terminal",
        "Theme (terminal)" => "Tema de la terminal",
        "default" => "predeterminado",
        "Banner" => "Encabezado",
        "pink · rose on · text on" => "pink · rosa activada · texto activado",
        "These values reflect Gentle Shell's terminal configuration. Desktop themes and terminal themes are stored separately." => {
            "Estos valores reflejan la configuración de la terminal de Gentle Shell. Los temas de escritorio y de terminal se guardan por separado."
        }
        "Gentle Shell and Engram" => "Gentle Shell y Engram",
        "Versions" => "Versiones",
        "Waiting for the agent tab to start…" => {
            "Esperando a que se inicie la pestaña del agente…"
        }
        "Checking Gentle Shell for updates…" => "Buscando actualizaciones de Gentle Shell…",
        "Check for updates" => "Buscar actualizaciones",
        "Version" => "Versión",
        "License" => "Licencia",
        "Fork of" => "Derivado de",
        "Repository" => "Repositorio",
        "Issues" => "Incidencias",
        "Credits" => "Créditos",
        "Third-party licenses" => "Licencias de terceros",
        "The animated scenes and the Rust crates built into the app keep their own licenses." => {
            "Las escenas animadas y las bibliotecas de Rust incluidas en la app conservan sus propias licencias."
        }
        "Scenes license" => "Licencia de las escenas",
        "Dependencies" => "Dependencias",
        "@{login} on GitHub" => "@{login} en GitHub",
        "Contributors" => "Colaboradores",
        "Panels & tabs" => "Paneles y pestañas",
        "Tabs" => "Pestañas",
        "Horizontal" => "Horizontal",
        "Vertical" => "Vertical",
        "Navigator" => "Navegador",
        "Show" => "Mostrar",
        "Hide" => "Ocultar",
        "Navigator style" => "Estilo del navegador",
        "Expanded" => "Expandido",
        "Icon rail" => "Barra de iconos",
        "Files" => "Archivos",
        "Tabs appear at the top or on the left. Files always opens on the right." => {
            "Las pestañas aparecen arriba o a la izquierda. El panel de archivos siempre se abre a la derecha."
        }
        "Composer" => "Redacción de mensajes",
        "Input style" => "Estilo del cuadro de texto",
        "Card" => "Tarjeta",
        "Bar" => "Barra",
        "Prompt" => "Indicador",
        "Pill" => "Cápsula",
        "Soft" => "Redondeado",
        "A floating card with its controls inside; folder and branch below." => {
            "Una tarjeta flotante con los controles dentro y la carpeta y la rama debajo."
        }
        "Docked across the pane: one row of chips, a ↵ key and Stop · esc." => {
            "Anclado de lado a lado: una fila de controles, una tecla ↵ y Detener · esc."
        }
        "A framed ❯ line in the code font over a terminal status strip." => {
            "Una línea con marco y el símbolo ❯ en la fuente de código, sobre una barra de estado de terminal."
        }
        "One rounded pill: attach, text and send on the same line." => {
            "Una cápsula redondeada: adjuntos, texto y envío en la misma línea."
        }
        "A soft rounded box: the text on top, quiet controls and a filled send." => {
            "Un cuadro redondeado con el texto arriba, controles discretos y un botón de envío relleno."
        }
        "Water reminder" => "Recordatorio para beber agua",
        "Reminder" => "Recordatorio",
        "Off" => "Desactivado",
        "On" => "Activado",
        "Every" => "Cada",
        "Show it now" => "Mostrar ahora",
        "1 hour" => "1 hora",
        "2 hours" => "2 horas",
        "Drops fall over the window and fill a glass. The clock starts when the app opens; changing how often keeps counting from then." => {
            "Caen gotas por la ventana y llenan un vaso. El tiempo se cuenta desde que abres la app, aunque cambies la frecuencia."
        }
        "No recordings yet." => "Todavía no hay grabaciones.",
        "Stop and show file" => "Detener y mostrar el archivo",
        "Start recording" => "Iniciar grabación",
        "Diagnostics" => "Diagnóstico",
        "Show last recording" => "Mostrar la última grabación",
        "Recording to {name}" => "Grabando en {name}",
        "Last recording: {file}" => "Última grabación: {file}",
        "If the app uses too much CPU or memory, start a recording, do what makes it happen for a few minutes, then stop. It saves only numbers and program names (never prompts, conversations or paths) and nothing leaves this computer: attach the file to your issue on GitHub." => {
            "Si la app usa demasiada CPU o memoria, inicia una grabación, reproduce el problema durante unos minutos y detén la grabación. Solo se guardan cifras y nombres de programas (nunca instrucciones, conversaciones ni rutas). Nada sale de este equipo: adjunta el archivo a tu incidencia en GitHub."
        }
        "This computer" => "Este equipo",
        "Command" => "Comando",
        "Where agents run" => "Dónde se ejecutan los agentes",
        "Agents run on" => "Ejecutar agentes en",
        "Distro" => "Distribución",
        "Looking for WSL distros…" => "Buscando distribuciones de WSL…",
        "No WSL distros found. Install one with wsl --install." => {
            "No se encontraron distribuciones de WSL. Instala una con wsl --install."
        }
        "default · gentle-shell found" => "predeterminada · gentle-shell disponible",
        "gentle-shell found" => "gentle-shell disponible",
        "default · no gentle-shell" => "predeterminada · sin gentle-shell",
        "no gentle-shell" => "sin gentle-shell",
        "A command that runs a program in that environment, as argv (OrbStack: orb -m dev --, Lima: limactl shell dev, a container: docker exec -i dev). Press Enter to use it." => {
            "Un comando que ejecuta un programa en ese entorno, como argv (OrbStack: orb -m dev --, Lima: limactl shell dev, un contenedor: docker exec -i dev). Pulsa Enter para usarlo."
        }
        "Agents, terminals, git and Setup run here; open agent tabs stay where they started. A project opened from \\\\wsl.localhost\\<distro>\\ always runs in that distro." => {
            "Los agentes, las terminales, git y la configuración inicial se ejecutan aquí. Las pestañas de agentes abiertas mantienen su entorno. Un proyecto abierto desde \\\\wsl.localhost\\<distro>\\ siempre se ejecuta en esa distribución."
        }
        "Checking {label}…" => "Comprobando {label}…",
        "New agent tabs run on {label}." => {
            "Las nuevas pestañas de agentes se ejecutan en {label}."
        }
        "gentle-shell wasn't found on {label}. Install it there first." => {
            "No se encontró gentle-shell en {label}. Instálalo allí primero."
        }
        "Terminal font now follows the code font. Click to undo." => {
            "La terminal ahora usa la fuente de código. Haz clic para deshacer."
        }
        "Search fonts…" => "Buscar fuentes…",
        "Interface font" => "Fuente de la interfaz",
        "Code font" => "Fuente de código",
        "Terminal font" => "Fuente de la terminal",
        "Same as code font" => "Igual que la fuente de código",
        "Default" => "Predeterminada",
        "No installed font matches." => "No se encontró ninguna fuente instalada que coincida.",
        "{slot} changed to {family}. Click to undo." => {
            "{slot}: se cambió a {family}. Haz clic para deshacer."
        }
        "{slot} reset to default. Click to undo." => {
            "{slot}: se restauró la fuente predeterminada. Haz clic para deshacer."
        }
        "{slot} size changed to {size} px. Click to undo." => {
            "{slot}: el tamaño cambió a {size} px. Haz clic para deshacer."
        }
        "Paste a theme link (https://…/name.gentletheme)" => {
            "Pega un enlace a un tema (https://…/name.gentletheme)"
        }
        "Import theme" => "Importar tema",
        "A theme link starts with https://" => "El enlace a un tema empieza con https://",
        "Animated backgrounds" => "Fondos animados",
        "Your Pi themes" => "Tus temas de Pi",
        "Popular" => "Populares",
        "This theme has no background picture to blur." => {
            "Este tema no tiene una imagen de fondo que desenfocar."
        }
        "Softens the theme's animated scene (scenes are faded, not blurred)." => {
            "Atenúa la escena animada del tema (las escenas se atenúan, no se desenfocan)."
        }
        "Blurs the theme's picture; its animated scene is faded, not blurred." => {
            "Desenfoca la imagen del tema. La escena animada se atenúa, pero no se desenfoca."
        }
        "Blurs the theme's background picture." => "Desenfoca la imagen de fondo del tema.",
        "Theme" => "Tema",
        "Window & interface" => "Ventana e interfaz",
        "Transparency" => "Transparencia",
        "How much the desktop or the theme's background shows through the window. Past 80%, text trades readability for glass." => {
            "Cuánto se ve el escritorio o el fondo del tema a través de la ventana. Pasado el 80 %, el texto cede legibilidad a cambio de transparencia."
        }
        "Blur" => "Desenfoque",
        "Opacity" => "Opacidad",
        "How solid the areas with text are. Up to 80% transparency, text always stays readable (WCAG AA)." => {
            "Qué tan opacas son las zonas con texto. Hasta el 80 % de transparencia, el texto siempre se puede leer (WCAG AA)."
        }
        "Fonts" => "Fuentes",
        "Scene motion" => "Movimiento de las escenas",
        "Typing paws & notes" => "Huellas y notas al escribir",
        "OLED Black stays solid black at any transparency, so an OLED screen keeps those pixels off." => {
            "OLED Black se mantiene en negro puro con cualquier transparencia, para que las pantallas OLED mantengan esos píxeles apagados."
        }
        "Theme library" => "Biblioteca de temas",
        "Downloading…" => "Descargando…",
        "Import…" => "Importar…",
        "Open the themes folder" => "Abrir la carpeta de temas",
        "Interface size" => "Tamaño de la interfaz",
        "Smaller" => "Reducir",
        "Bigger" => "Ampliar",
        "Reset" => "Restablecer",
        "Share: save it as one file" => "Compartir: guardar en un solo archivo",
        "Added “{name}” and switched to it." => "Se agregó y activó «{name}».",
        "Couldn't read" => "No se pudo leer",
        "Couldn't download the theme: {error}" => "No se pudo descargar el tema: {error}",
        "Couldn't share it: {error}" => "No se pudo compartir: {error}",
        "Couldn't save it: {error}" => "No se pudo guardar: {error}",
        "This theme's scene stopped: {error}" => "La escena de este tema se detuvo: {error}",
        "Couldn't download the theme: HTTP {status}" => {
            "No se pudo descargar el tema: HTTP {status}"
        }
        "Saved {path}: send it to anyone with the app, they import it here." => {
            "Se guardó {path}. Envíalo a quien use la app para que lo importe aquí."
        }
        "{bigger} bigger · {smaller} smaller · {reset} resets" => {
            "{bigger} amplía · {smaller} reduce · {reset} restablece"
        }
        "Scenes only move while this window is in front.\nOn battery, they move at Calm at most.\nWhile saving power, they hold still." => {
            "Las escenas solo se mueven con la ventana al frente.\nCon batería, como máximo usan el modo Suave.\nEn modo de ahorro de energía, se quedan quietas."
        }
        "Ask an agent for one (“a theme with koi in a pond, in my colors”): it writes the theme and its scene to the app's themes folder and it shows up here. Share any of them with the share button on its card; import one you got with a link, Import…, or by dropping its file here." => {
            "Pídele un tema a un agente («un tema con carpas koi en un estanque, en mis colores»): guardará el tema y su escena en la carpeta de temas de la app y aparecerá aquí. Comparte un tema desde su tarjeta. Para importar uno, usa un enlace, selecciona Importar… o arrastra el archivo aquí."
        }
        "Smooth" => "Fluido",
        "Calm" => "Suave",
        "Still" => "Fijo",
        "Full motion, up to 24 fps." => "Movimiento completo, hasta 24 fps.",
        "Half the rate. Easier on battery." => "A la mitad de velocidad. Consume menos batería.",
        "No motion. Costs nothing." => "Sin movimiento ni consumo adicional.",
        "Saving power: scenes hold still." => "Ahorro de energía: las escenas no se mueven.",
        "On battery: scenes move at Calm." => "Con batería: las escenas se mueven en modo Suave.",
        "Auto" => "Automático",
        "Paws and notes float up as you type, unless the system reduces motion." => {
            "Las huellas y las notas flotan al escribir, salvo si el sistema reduce el movimiento."
        }
        "No paws or notes while typing." => "Sin huellas ni notas al escribir.",
        "GitHub releases" => "Publicaciones en GitHub",
        "Development build: automatic update checks are off." => {
            "Versión de desarrollo: la búsqueda automática de actualizaciones está desactivada."
        }
        "Checking GitHub for a newer release…" => "Buscando una versión más reciente en GitHub…",
        "You're on the latest release." => "Tienes la versión más reciente.",
        "Downloads the release for this computer, checks it against the published SHA-256 and replaces the app. The running app keeps working until you restart." => {
            "Descarga la versión para este equipo, verifica el SHA-256 publicado y reemplaza la app. La app sigue funcionando hasta que la reinicies."
        }
        "Try again" => "Reintentar",
        "Download & install" => "Descargar e instalar",
        "Release notes" => "Notas de la versión",
        "Skip this version" => "Omitir esta versión",
        "Restart to use it. Workspaces are saved first and reopen as you left them." => {
            "Reinicia para usarla. Los espacios de trabajo se guardan y se vuelven a abrir tal como los dejaste."
        }
        "Restart now" => "Reiniciar ahora",
        "App" => "Aplicación",
        "Skipping {version}. Check for updates to see it again." => {
            "Omitiste la versión {version}. Busca actualizaciones para volver a verla."
        }
        "{version} (development build)" => "{version} (versión de desarrollo)",
        "Osdy Code Desktop {version} is available" => "Osdy Code Desktop {version} está disponible",
        "you have {version}" => "tienes la versión {version}",
        "Downloading Osdy Code Desktop {version}…" => "Descargando Osdy Code Desktop {version}…",
        "Verified. Installing Osdy Code Desktop {version}…" => {
            "Verificación completa. Instalando Osdy Code Desktop {version}…"
        }
        "Osdy Code Desktop {version} is installed" => "Osdy Code Desktop {version} está instalado",
        "{done} of {total}" => "{done} de {total}",
        "Updating…" => "Actualizando…",
        "Runs gentle-shell upgrade. Open agent tabs then restart on the new version and keep their conversations; a tab mid-turn switches when the turn ends." => {
            "Ejecuta gentle-shell upgrade. Las pestañas de agentes abiertas se reinician con la nueva versión y conservan sus conversaciones. Si hay un turno en curso, esperan a que termine."
        }
        "Runs gentle-shell upgrade --channel main. Open agent tabs then restart on the new build and keep their conversations; a tab mid-turn switches when the turn ends." => {
            "Ejecuta gentle-shell upgrade --channel main. Las pestañas de agentes abiertas se reinician con la nueva compilación y conservan sus conversaciones. Si hay un turno en curso, esperan a que termine."
        }
        "The Pi update didn't finish." => "La actualización de Pi no se completó.",
        "The Pi update finished." => "La actualización de Pi se completó.",
        "Updating Pi…" => "Actualizando Pi…",
        "Update Pi" => "Actualizar Pi",
        "This can take several minutes." => "Esto puede tardar varios minutos.",
        "Updates Engram the way it was installed (Homebrew, or the release the app downloaded). Your memories stay." => {
            "Actualiza Engram con el mismo método de instalación (Homebrew o la versión que descargó la app). Tus recuerdos se conservan."
        }
        "Update now" => "Actualizar ahora",
        "This can take several minutes. Open agent tabs restart when it ends." => {
            "Esto puede tardar varios minutos. Las pestañas de agentes abiertas se reinician al terminar."
        }
        "Open agent tabs restarted on it and kept their conversations. Nothing else to do." => {
            "Las pestañas de agentes abiertas se reiniciaron con esta versión y conservaron sus conversaciones. No necesitas hacer nada más."
        }
        "Open agent tabs restarted on it. Gentle Shell's output below says what is left to do." => {
            "Las pestañas de agentes abiertas se reiniciaron con esta versión. La salida de Gentle Shell indica qué falta por hacer."
        }
        "The update didn't finish" => "La actualización no se completó",
        "Gentle Shell's output is below; the version you had keeps running." => {
            "La salida de Gentle Shell aparece abajo. Tu versión anterior sigue en uso."
        }
        "Could not remember the skip: {error}" => "No se pudo guardar la versión omitida: {error}",
        "Gentle Shell main has a newer commit {commit}" => {
            "Gentle Shell main tiene un commit más reciente: {commit}"
        }
        "Gentle Shell {version} is installed" => "Gentle Shell {version} está instalado",
        "Gentle Shell {version} is installed, but the update didn't finish" => {
            "Gentle Shell {version} está instalado, pero la actualización no se completó"
        }
        "Engram {version} is installed. An Engram server that's already running keeps the old version until it restarts." => {
            "Engram {version} está instalado. Si un servidor de Engram ya está en ejecución, seguirá usando la versión anterior hasta que se reinicie."
        }
        "{launcher} comes first. Remove the old package with `npm uninstall -g gentle-pi`, then open Configuration again." => {
            "{launcher} tiene prioridad. Elimina el paquete anterior con `npm uninstall -g gentle-pi` y vuelve a abrir Configuración."
        }
        "{tool} {version} is available" => "{tool} {version} está disponible",
        "Gentle Shell {version} installed, but the app still runs {running}" => {
            "Gentle Shell {version} está instalado, pero la app sigue ejecutando {running}"
        }
        "not overridden" => "sin sustitución",
        "override in use" => "sustitución en uso",
        "override declared but unusable" => "sustitución declarada, pero no disponible",
        "present" => "presente",
        "missing" => "ausente",
        "valid" => "válido",
        "invalid" => "no válido",
        "not saved" => "sin guardar",
        "Persona" => "Persona",
        "Dev binary" => "Ejecutable de desarrollo",
        "Delegation assets" => "Recursos de delegación",
        "Review assets" => "Recursos de revisión",
        "up to date" => "al día",
        "override in use (sha256 {prefix}…)" => "sustitución en uso (sha256 {prefix}…)",
        "models.json {config} · routing {routing}" => {
            "models.json {config} · enrutamiento {routing}"
        }
        "{count} stale (/gentle:install-{owner} --force)" => {
            "{count} sin actualizar (/gentle:install-{owner} --force)"
        }
        "{count} replaced by you" => "{count} con cambios tuyos",
        "{count} shadowed by your agents" => "{count} sustituidos por tus agentes",
        "off" => "desactivado",
        "on" => "activado",
        "{color} · {rose} · text {text}" => "{color} · {rose} · texto {text}",
        "rose on" => "rosa activada",
        "rose off" => "rosa desactivada",
        "not found in {label} (only Windows' {path})" => {
            "No se encontró en {label} (solo se encontró {path} de Windows)"
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};

    #[test]
    fn settings_a_translates_navigation_controls_and_status_templates() {
        for (english, spanish) in [
            ("Environment and setup", "Entorno y configuración"),
            ("Category: {category}", "Categoría: {category}"),
            ("Where agents run", "Dónde se ejecutan los agentes"),
            ("Check for updates", "Buscar actualizaciones"),
            (
                "Gentle Shell {version} is installed",
                "Gentle Shell {version} está instalado",
            ),
            ("Search fonts…", "Buscar fuentes…"),
        ] {
            assert_eq!(tr(Language::Spanish, english), spanish, "{english}");
            assert_eq!(tr(Language::English, english), english);
        }
    }
}
