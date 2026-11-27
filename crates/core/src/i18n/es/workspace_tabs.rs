//! Spanish text of the workspace tabs area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Workspace chrome and panes.
        "Switch workspace · an agent in another one needs you" => {
            "Cambiar de espacio de trabajo · un agente te necesita en otro"
        }
        "Switch workspace · agents are working in another one" => {
            "Cambiar de espacio de trabajo · hay agentes trabajando en otro"
        }
        "Switch workspace · subagents are running in another one" => {
            "Cambiar de espacio de trabajo · hay subagentes trabajando en otro"
        }
        "Switch workspace · jobs are running in another one" => {
            "Cambiar de espacio de trabajo · hay tareas en curso en otro"
        }
        "Switch workspace" => "Cambiar de espacio de trabajo",
        "Panes: tile, split or close the focused one" => {
            "Paneles: colocar en mosaico, dividir o cerrar el panel activo"
        }
        "Close “{target}”" => "Cerrar «{target}»",
        "New pane, tiled" => "Nuevo panel en mosaico",
        "Same kind, beside the focused one" => "Del mismo tipo, junto al panel activo",
        "Split right" => "Dividir a la derecha",
        "Same kind, to the right" => "Del mismo tipo, a la derecha",
        "Split down" => "Dividir hacia abajo",
        "Same kind, below" => "Del mismo tipo, debajo",
        "Configuration, profiles, memory, sessions, providers ({shortcut})" => {
            "Configuración, perfiles, memoria, sesiones y proveedores ({shortcut})"
        }
        "Theme, models, integrations, updates" => "Tema, modelos, integraciones y actualizaciones",
        "Model and effort per agent" => "Modelo y esfuerzo por agente",
        "Memory" => "Memoria",
        "Engram observations" => "Observaciones de Engram",
        "Sessions" => "Sesiones",
        "Resume a past session" => "Retomar una sesión anterior",
        "Providers" => "Proveedores",
        "Sign-in and API keys" => "Inicio de sesión y claves de API",
        "No agents in this workspace yet" => "Todavía no hay agentes en este espacio de trabajo",
        "Open agent" => "Abrir agente",
        "Agent tab" => "Pestaña de agente",
        "Gentle Shell in this folder" => "Gentle Shell en esta carpeta",
        "Terminal tab" => "Pestaña de terminal",
        "A shell in this folder" => "Una terminal en esta carpeta",
        "Canvas tab" => "Pestaña de lienzo",
        "A whiteboard: ink, shapes, notes" => {
            "Una pizarra para dibujar, agregar formas y tomar notas"
        }
        "Tower tab" => "Pestaña de Gentle Tower",
        "Gentle Tower: every session as a floor" => "Gentle Tower: cada sesión es un piso",
        "New agent" => "Nuevo agente",
        "New terminal" => "Nueva terminal",
        "Rename" => "Cambiar nombre",
        "Badge…" => "Distintivo…",
        "New agent tab" => "Nueva pestaña de agente",
        "New terminal tab" => "Nueva pestaña de terminal",
        "New canvas tab" => "Nueva pestaña de lienzo",
        "New tower tab" => "Nueva pestaña de Gentle Tower",
        "New workspace" => "Nuevo espacio de trabajo",
        "Delete" => "Eliminar",
        "Search workspaces…" => "Buscar espacios de trabajo…",
        "Search tabs" => "Buscar pestañas",
        "Initials" => "Iniciales",
        "From the name" => "Según el nombre",
        "Colors follow the theme" => "Los colores siguen el tema",
        "Icon" => "Icono",
        "Folder" => "Carpeta",
        "Globe" => "Globo",
        "Bot" => "Robot",
        "Brain" => "Cerebro",
        "Rocket" => "Cohete",
        "Zap" => "Rayo",
        "Flame" => "Llama",
        "Leaf" => "Hoja",
        "Database" => "Base de datos",
        "Server" => "Servidor",
        "Cloud" => "Nube",
        "Smartphone" => "Teléfono",
        "Palette" => "Paleta",
        "Pen" => "Pluma",
        "Book" => "Libro",
        "Music" => "Música",
        "Shield" => "Escudo",
        "Sparkles" => "Destellos",
        "Star" => "Estrella",
        "Heart" => "Corazón",
        "Gem" => "Gema",
        "Crown" => "Corona",
        "Coffee" => "Café",
        "Gamepad" => "Mando",
        "Flask" => "Matraz",
        "Atom" => "Átomo",
        "Bug" => "Insecto",
        "Puzzle" => "Rompecabezas",
        "Cat" => "Gato",
        "Color" => "Color",
        "Accent" => "Acento",
        "Blue" => "Azul",
        "Teal" => "Verde azulado",
        "Green" => "Verde",
        "Gold" => "Dorado",
        "Amber" => "Ámbar",
        "Red" => "Rojo",
        "Purple" => "Morado",
        "{count} panes" => "{count} paneles",
        "Closed “{title}”. Click to reopen ({shortcut})." => {
            "Se cerró «{title}». Haz clic para volver a abrir ({shortcut})."
        }
        "Horizontal tabs" => "Pestañas horizontales",
        "Vertical tabs" => "Pestañas verticales",
        "Compact sidebar" => "Barra lateral compacta",
        "Pin tab" => "Fijar pestaña",
        "Unpin tab" => "Desfijar pestaña",
        "Move up" => "Mover arriba",
        "Move down" => "Mover abajo",
        "Close tab" => "Cerrar pestaña",
        "Pin {count} tabs" => "Fijar {count} pestañas",
        "Unpin {count} tabs" => "Desfijar {count} pestañas",
        "Close {count} tabs" => "Cerrar {count} pestañas",
        "Pages" => "Páginas",
        "Hide all" => "Ocultar todo",
        "Close" => "Cerrar",
        "Pages agents opened" => "Páginas abiertas por los agentes",
        "Drag to resize · double-click to reset" => {
            "Arrastra para cambiar el tamaño · haz doble clic para restablecerlo"
        }
        "Hide workspace sidebar" => "Ocultar la barra de espacios de trabajo",
        "Show workspace sidebar" => "Mostrar la barra de espacios de trabajo",
        "Commands ({shortcut})" => "Comandos ({shortcut})",
        "Hide Files sidebar ({shortcut})" => "Ocultar la barra de archivos ({shortcut})",
        "Show Files sidebar ({shortcut})" => "Mostrar la barra de archivos ({shortcut})",
        "Devices: simulators and emulators" => "Dispositivos: simuladores y emuladores",
        "{activity} in background" => "{activity} en segundo plano",
        " and " => " y ",
        "An agent here needs you" => "Un agente aquí te necesita",
        "Agents are working here" => "Hay agentes trabajando aquí",
        "Agent" => "Agente",
        "Canvas" => "Lienzo",
        "Tower" => "Gentle Tower",
        "New tab" => "Nueva pestaña",
        "{count} running" => "{count} en curso",
        "{count} waiting" => "{count} en espera",
        "New agent or terminal in this workspace" => {
            "Nuevo agente o terminal en este espacio de trabajo"
        }
        "New agent or terminal" => "Nuevo agente o terminal",
        "Expand workspace sidebar" => "Expandir la barra de espacios de trabajo",
        "No sessions yet" => "Todavía no hay sesiones",
        "No matching workspaces or sessions" => {
            "No hay espacios de trabajo ni sesiones que coincidan"
        }
        "{kind} panes are not available in this build." => {
            "Los paneles de tipo {kind} no están disponibles en esta versión."
        }
        "Stop {names} and close it?" => "¿Detener y cerrar {names}?",
        "Stop {count} agents and close them?" => "¿Detener y cerrar {count} agentes?",
        "Close {names} without saving?" => "¿Cerrar {names} sin guardar?",
        "Close {count} files without saving?" => "¿Cerrar {count} archivos sin guardar?",
        "Close unsaved files and working agents?" => {
            "¿Cerrar los archivos sin guardar y los agentes que están trabajando?"
        }
        "Unsaved edits in {names} are lost. {shortcut} saves first." => {
            "Se perderán los cambios sin guardar en {names}. Usa {shortcut} para guardarlos primero."
        }
        "{names} is working or waiting for your answer; closing stops it. The conversation is kept: {shortcut} reopens it." => {
            "{names} está trabajando o esperando tu respuesta. Si lo cierras, se detendrá. La conversación se conserva: puedes reabrirla con {shortcut}."
        }
        "{names} are working or waiting for your answer; closing stops them. The conversation is kept: {shortcut} reopens it." => {
            "{names} están trabajando o esperando tu respuesta. Si los cierras, se detendrán. La conversación se conserva: puedes reabrirla con {shortcut}."
        }
        "Close without saving" => "Cerrar sin guardar",
        "Stop and close" => "Detener y cerrar",
        "Create workspace" => "Crear espacio de trabajo",
        "Open folder" => "Abrir carpeta",
        "Move this agent to “{folder}”?" => "¿Mover este agente a «{folder}»?",
        "Its conversation ends and a new session starts in that folder. The previous conversation stays on disk." => {
            "La conversación termina y empieza una sesión nueva en esa carpeta. La conversación anterior se conserva en el disco."
        }
        "Move" => "Mover",
        "Delete workspace “{name}”?" => "¿Eliminar el espacio de trabajo «{name}»?",
        "Its 1 tab close and any running agent in them stops. Files and conversations stay on disk." => {
            "Se cerrará su única pestaña y se detendrán los agentes que estén trabajando en ella. Los archivos y las conversaciones se conservan en el disco."
        }
        "Its {count} tabs close and any running agent in them stops. Files and conversations stay on disk." => {
            "Se cerrarán sus {count} pestañas y se detendrán los agentes que estén trabajando en ellas. Los archivos y las conversaciones se conservan en el disco."
        }
        "Switch to “{branch}”?" => "¿Cambiar a «{branch}»?",
        "Runs {command} in {folder}. Uncommitted changes that conflict make git refuse." => {
            "Ejecuta {command} en {folder}. Git rechazará el cambio si hay cambios sin confirmar que entren en conflicto."
        }
        "Switch" => "Cambiar",
        "Switched to {branch}" => "Se cambió a {branch}",
        "Profile “{profile}” applied to this session's orchestrator; helpers keep the routing this session started with." => {
            "Se aplicó el perfil «{profile}» al orquestador de esta sesión. Los agentes auxiliares conservan la configuración inicial de la sesión."
        }
        "Open an agent tab first: Gentle Shell applies this setting." => {
            "Abre primero una pestaña de agente: Gentle Shell aplica este ajuste."
        }
        "The setup command is on your clipboard: run it in a terminal." => {
            "El comando de configuración está en el portapapeles. Ejecútalo en una terminal."
        }
        "The command is typed in this terminal: press Enter to run it. Reopen Setup & requirements from the command palette when it's done." => {
            "El comando ya está escrito en esta terminal. Presiona Enter para ejecutarlo. Cuando termine, vuelve a abrir Configuración inicial y requisitos desde la paleta de comandos."
        }
        "The setup command is on your clipboard: paste it here and press Enter. Reopen Setup & requirements from the command palette when it's done." => {
            "El comando de configuración está en el portapapeles. Pégalo aquí y presiona Enter. Cuando termine, vuelve a abrir Configuración inicial y requisitos desde la paleta de comandos."
        }
        // Command palette.
        "All" => "Todo",
        "Commands" => "Comandos",
        "Workspaces" => "Espacios de trabajo",
        "Branches" => "Ramas",
        "Search commands, workspaces, files…" => {
            "Buscar comandos, espacios de trabajo, archivos…"
        }
        "Type a command or screen…" => "Escribe un comando o el nombre de una pantalla…",
        "Find or create a workspace…" => "Buscar o crear un espacio de trabajo…",
        "Find a file…" => "Buscar un archivo…",
        "Find a branch…" => "Buscar una rama…",
        "Delete workspace" => "Eliminar espacio de trabajo",
        "Indexing files…" => "Indexando archivos…",
        "Reading branches…" => "Leyendo ramas…",
        "No local branches here" => "No hay ramas locales",
        "No matches" => "No hay coincidencias",
        "navigate" => "navegar",
        "run" => "ejecutar",
        "category" => "categoría",
        "switch workspaces" => "cambiar de espacio de trabajo",
        "close" => "cerrar",
        "1 tab" => "1 pestaña",
        "{count} tabs" => "{count} pestañas",
        " · current" => " · actual",
        "Create workspace “{name}”" => "Crear el espacio de trabajo «{name}»",
        "in {folder}" => "en {folder}",
        "Browse folders…" => "Explorar carpetas…",
        "Open Osdy Code Desktop in any folder ({shortcut})" => {
            "Abrir Osdy Code Desktop en cualquier carpeta ({shortcut})"
        }
        "Create workspace from folder…" => "Crear espacio de trabajo desde una carpeta…",
        "Named after the folder" => "Con el nombre de la carpeta",
        "Named “{name}”" => "Con el nombre «{name}»",
        "current" => "actual",
        "needs you" => "te necesita",
        "{count} need you" => "{count} te necesitan",
        "working" => "trabajando",
        "{count} working" => "{count} trabajando",
        "1 subagent" => "1 subagente",
        "{count} subagents" => "{count} subagentes",
        "1 job" => "1 tarea",
        "{count} jobs" => "{count} tareas",
        "Helpers working for this session ({shortcut})" => {
            "Agentes auxiliares que trabajan en esta sesión ({shortcut})"
        }
        "Model and effort per agent; profiles as tabs" => {
            "Modelo y esfuerzo por agente; perfiles como pestañas"
        }
        "Resume a previous conversation" => "Retomar una conversación anterior",
        "Browse and search Engram, the persistent memory" => {
            "Explorar y buscar en Engram, la memoria persistente"
        }
        "Architecture docs" => "Documentación de arquitectura",
        "ADRs, C4, data, states and flows, written and kept current by the agent" => {
            "ADR, C4, datos, estados y flujos que el agente redacta y mantiene al día"
        }
        "Commits as a graph, branches, tags, pull, push, cherry-pick, revert and reset" => {
            "Gráfico de commits, ramas, etiquetas, pull, push, cherry-pick, revert y reset"
        }
        "Launch an Android emulator or iOS simulator and watch it" => {
            "Iniciar y ver un emulador de Android o un simulador de iOS"
        }
        "Usage & stats" => "Uso y estadísticas",
        "Subscription windows and local token/cost stats" => {
            "Períodos de suscripción y estadísticas locales de tokens y costos"
        }
        "Sign in with a subscription or an API key" => {
            "Iniciar sesión con una suscripción o una clave de API"
        }
        "Settings" => "Configuración",
        "Osdy Code Desktop and Gentle Shell settings" => {
            "Ajustes de Osdy Code Desktop y Gentle Shell"
        }
        "Check Node, Gentle Shell, providers and extras; install what's missing" => {
            "Revisa Node, Gentle Shell, los proveedores y los complementos; instala lo que falte"
        }
        "Welcome screen" => "Pantalla de bienvenida",
        "The first-run screen: the intro, then the install sequence" => {
            "Pantalla del primer inicio: introducción seguida de la instalación"
        }
        "Browse folders" => "Explorar carpetas",
        "Open Osdy Code Desktop in any folder, oil-style ({shortcut})" => {
            "Abrir Osdy Code Desktop en cualquier carpeta, al estilo de oil ({shortcut})"
        }
        "Install `ocd` command" => "Instalar el comando `ocd`",
        "Type ocd in any terminal folder to open it here as a workspace" => {
            "Escribe ocd en cualquier carpeta desde la terminal para abrirla aquí como espacio de trabajo"
        }
        "New session" => "Nueva sesión",
        "Start a fresh conversation in this pane ({shortcut})" => {
            "Iniciar una conversación nueva en este panel ({shortcut})"
        }
        "Another agent in the same folder ({shortcut})" => {
            "Otro agente en la misma carpeta ({shortcut})"
        }
        "A shell in the same folder" => "Una terminal en la misma carpeta",
        "A whiteboard for ink, shapes, notes and images ({shortcut})" => {
            "Una pizarra para dibujos, formas, notas e imágenes ({shortcut})"
        }
        "Every session as a floor of its project's building: tasks, talk, artifacts, cost" => {
            "Cada sesión es un piso en el edificio de su proyecto: tareas, conversaciones, artefactos y costos"
        }
        "New pane (tiled)" => "Nuevo panel (en mosaico)",
        "Same kind of pane, tiled beside the focused one ({shortcut})" => {
            "Panel del mismo tipo, en mosaico junto al panel activo ({shortcut})"
        }
        "Same kind of pane to the right of this one" => "Panel del mismo tipo a la derecha de este",
        "Same kind of pane below this one ({shortcut})" => {
            "Panel del mismo tipo debajo de este ({shortcut})"
        }
        "New agent pane" => "Nuevo panel de agente",
        "Split with an agent beside the focused pane" => {
            "Dividir y abrir un agente junto al panel activo"
        }
        "New terminal pane" => "Nuevo panel de terminal",
        "Split with a terminal beside the focused pane" => {
            "Dividir y abrir una terminal junto al panel activo"
        }
        "Close pane" => "Cerrar panel",
        "The last pane closes its tab ({shortcut})" => {
            "Al cerrar el último panel se cierra su pestaña ({shortcut})"
        }
        "Reopen closed pane" => "Reabrir panel cerrado",
        "The last pane or tab closed, where it was ({shortcut})" => {
            "Reabre el último panel o pestaña cerrados en su lugar original ({shortcut})"
        }
        "View: Show tabs vertically" => "Vista: mostrar pestañas en vertical",
        "Move tabs from the top bar into the left sidebar" => {
            "Mover las pestañas de la barra superior a la barra lateral izquierda"
        }
        "View: Show tabs horizontally" => "Vista: mostrar pestañas en horizontal",
        "Move tabs from the sidebar back into the top bar" => {
            "Devolver las pestañas de la barra lateral a la barra superior"
        }
        "Toggle Files sidebar" => "Mostrar u ocultar la barra de archivos",
        "Show or hide Files / Changes / Branches on the right ({shortcut})" => {
            "Mostrar u ocultar Archivos / Cambios / Ramas a la derecha ({shortcut})"
        }
        "Browser card" => "Tarjeta del navegador",
        "Restore browser card" => "Restaurar tarjeta del navegador",
        "{title} back to its size and place (Esc)" => "{title} vuelve a su tamaño y posición (Esc)",
        "Maximize browser card" => "Maximizar tarjeta del navegador",
        "{title} fills the window (or double-click its header)" => {
            "{title} ocupa toda la ventana (también puedes hacer doble clic en su encabezado)"
        }
        "Hide browser card" => "Ocultar tarjeta del navegador",
        "{title} waits in the instance tray" => "{title} queda en la bandeja de instancias",
        "Close browser card" => "Cerrar tarjeta del navegador",
        "{title} closes, page and all" => "Se cierra {title} junto con su página",
        "Skills" => "Habilidades",
        "Gentle commands" => "Comandos de Gentle",
        // Session browser and terminal.
        "Search by name, first message or path" => "Buscar por nombre, primer mensaje o ruta",
        "An extension cancelled the switch; this tab keeps its session." => {
            "Una extensión canceló el cambio. Esta pestaña conserva su sesión."
        }
        "That session belongs to another project, and its file was not found to open it in a new tab." => {
            "Esa sesión pertenece a otro proyecto y no se encontró su archivo para abrirla en una pestaña nueva."
        }
        "{count} msg" => "{count} mensaje",
        "{count} msgs" => "{count} mensajes",
        "new tab" => "pestaña nueva",
        "This Gentle Shell does not offer that over Desktop." => {
            "Esta versión de Gentle Shell no ofrece esa función en Desktop."
        }
        "Gentle Shell did not answer in time." => "Gentle Shell no respondió a tiempo.",
        "Gentle Shell restarted before it answered." => {
            "Gentle Shell se reinició antes de responder."
        }
        "The request failed ({error})." => "La solicitud falló ({error}).",
        "Reading sessions…" => "Leyendo sesiones…",
        "No sessions for this project yet." => "Todavía no hay sesiones para este proyecto.",
        "No sessions found." => "No se encontraron sesiones.",
        "Load older sessions" => "Cargar sesiones anteriores",
        "No loaded session matches; older ones may." => {
            "Ninguna sesión cargada coincide; puede haber coincidencias en las anteriores."
        }
        "No sessions match." => "Ninguna sesión coincide.",
        "This project" => "Este proyecto",
        "All projects" => "Todos los proyectos",
        "{count} more — refine the search" => "{count} más — acota la búsqueda",
        "Reload" => "Recargar",
        "Every project" => "Todos los proyectos",
        "open first match" => "abrir la primera coincidencia",
        "(no messages)" => "(sin mensajes)",
        "just now" => "ahora mismo",
        "{count}m ago" => "hace {count} min",
        "{count}h ago" => "hace {count} h",
        "{count}d ago" => "hace {count} d",
        "{count}w ago" => "hace {count} sem",
        "1mo ago" => "hace 1 mes",
        "{count}mo ago" => "hace {count} meses",
        "1y ago" => "hace 1 año",
        "{count}y ago" => "hace {count} años",
        "Could not start the shell: {error}" => "No se pudo iniciar la terminal: {error}",
        "Process exited" => "El proceso terminó",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};
    use crate::sessions::relative_time_in;

    #[test]
    fn translates_workspace_tabs_ui_without_changing_english() {
        for (en, es) in [
            (
                "No agents in this workspace yet",
                "Todavía no hay agentes en este espacio de trabajo",
            ),
            ("No local branches here", "No hay ramas locales"),
            ("Process exited", "El proceso terminó"),
            ("(no messages)", "(sin mensajes)"),
        ] {
            assert_eq!(tr(Language::Spanish, en), es);
            assert_eq!(tr(Language::English, en), en);
        }
        assert_eq!(
            relative_time_in(Language::Spanish, 300_000, 0),
            "hace 5 min"
        );
        assert_eq!(relative_time_in(Language::English, 300_000, 0), "5m ago");
    }
}
