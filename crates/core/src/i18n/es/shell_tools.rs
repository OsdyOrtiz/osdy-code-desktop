//! Spanish text of the shell tools area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        "New Tab" => "Nueva pestaña",
        "New Terminal Tab" => "Nueva pestaña de terminal",
        "New Canvas Tab" => "Nueva pestaña de lienzo",
        "New Pane (Tiled)" => "Nuevo panel (en mosaico)",
        "Split Right" => "Dividir hacia la derecha",
        "Split Down" => "Dividir hacia abajo",
        "Close Pane" => "Cerrar panel",
        "Close Tab" => "Cerrar pestaña",
        "Reopen Closed Pane" => "Reabrir panel cerrado",
        "Toggle Sidebar" => "Mostrar u ocultar la barra lateral",
        "Configuration…" => "Configuración…",
        "Zoom In" => "Acercar",
        "Zoom Out" => "Alejar",
        "Actual Size" => "Tamaño real",
        "Quit Osdy Code Desktop" => "Salir de Osdy Code Desktop",
        "Press {chord} again to close" => "Pulsa {chord} otra vez para cerrar",
        "Profile name" => "Nombre del perfil",
        "Design · {slug}" => "Diseño · {slug}",
        "Take over" => "Tomar el control",
        "An agent is driving this page · take it back" => {
            "Un agente controla esta página · recupera el control"
        }
        "Where this page keeps its cookies and sign-ins" => {
            "Dónde se guardan las cookies y sesiones de esta página"
        }
        "Reload" => "Recargar",
        "Open in your browser" => "Abrir en tu navegador",
        "Float" => "Separar",
        "Dock to the right" => "Acoplar a la derecha",
        "Restore" => "Restaurar",
        "Restore · Esc" => "Restaurar · Esc",
        "Maximize" => "Maximizar",
        "Hide · it stays in the top bar" => "Ocultar · sigue en la barra superior",
        "Close" => "Cerrar",
        "Pages with the same profile share cookies and sign-ins. Agents open theirs in Agent." => {
            "Las páginas del mismo perfil comparten cookies y sesiones. Los agentes abren las suyas en el perfil Agente."
        }
        "New profile…" => "Nuevo perfil…",
        "Clear website data…" => "Borrar datos de sitios web…",
        "Name the new profile; press Return to use it." => {
            "Ponle un nombre al perfil y pulsa Intro para usarlo."
        }
        "Clear website data in {name}?" => "¿Borrar los datos de sitios web de {name}?",
        "Sites using it sign you out and forget their settings. Pages using it reload." => {
            "Los sitios de este perfil cerrarán tu sesión y olvidarán sus ajustes. Las páginas se recargarán."
        }
        "Clear" => "Borrar",
        "Shared" => "Compartido",
        "Agent" => "Agente",
        "Private" => "Privado",
        "Session" => "Sesión",
        "Profile" => "Perfil",
        "Close tab · the device keeps running" => "Cerrar pestaña · el dispositivo seguirá activo",
        "Open a simulator or emulator" => "Abrir un simulador o emulador",
        "Shut down {name}" => "Apagar {name}",
        "Hide · devices keep running" => "Ocultar · los dispositivos seguirán activos",
        "No simulators or emulators yet: install Xcode (iOS) or Android Studio and create a virtual device." => {
            "Aún no hay simuladores ni emuladores. Instala Xcode (iOS) o Android Studio y crea un dispositivo virtual."
        }
        "Booting" => "Iniciando",
        "Running" => "En ejecución",
        "{name} is off." => "{name} está apagado.",
        "Start" => "Iniciar",
        "Starting {name}…" => "Iniciando {name}…",
        "Starting the device hub…" => "Iniciando el centro de dispositivos…",
        "Turn on Use devices to see simulators and emulators here, live and touchable." => {
            "Activa Usar dispositivos para ver y manejar simuladores y emuladores en vivo aquí."
        }
        "Configuration → Devices" => "Configuración → Dispositivos",
        "now" => "ahora",
        "session no longer open" => "sesión ya no disponible",
        "The session that sent this is no longer open: its pane shows another one now." => {
            "La sesión que envió esta notificación ya no está abierta: su panel muestra otra sesión."
        }
        "pane closed" => "panel cerrado",
        "The pane that sent this notification was closed." => {
            "El panel que envió esta notificación se cerró."
        }
        "{count} new" => "{count} sin leer",
        "Mark all read" => "Marcar todas como leídas",
        "No notifications" => "No hay notificaciones",
        "Time for some water" => "Hora de tomar agua",
        "A glass now, and another in {interval}." => "Un vaso ahora y otro en {interval}.",
        "a while" => "un rato",
        "Search memory (Enter)" => "Buscar en la memoria (Intro)",
        "Nothing remembered for this project yet. el Gentleman saves decisions, bugs and discoveries here as you work." => {
            "Aún no hay recuerdos de este proyecto. el Gentleman guarda aquí decisiones, errores y hallazgos mientras trabajas."
        }
        "No memories match." => "No hay recuerdos que coincidan.",
        "{count} revision" => "{count} revisión",
        "{count} revisions" => "{count} revisiones",
        "Memory" => "Memoria",
        "Engram · all projects" => "Engram · todos los proyectos",
        "This project" => "Este proyecto",
        "All projects" => "Todos los proyectos",
        "Describe a mood…" => "Describe un ambiente…",
        "Music" => "Música",
        "Composed live by gentle-stream" => "Compuesta en vivo por gentle-stream",
        "Stop the music" => "Detener la música",
        "Start music" => "Iniciar la música",
        "Pause" => "Pausar",
        "Play" => "Reproducir",
        "Next piece, same mood" => "Siguiente pieza, mismo ambiente",
        "Music volume" => "Volumen de la música",
        "Starting the player…" => "Iniciando el reproductor…",
        "Composing “{mood}”…" => "Componiendo «{mood}»…",
        "Install Gentle Shell first: it reads the mood." => {
            "Instala Gentle Shell primero: interpreta el ambiente que describes."
        }
        "Now playing {name}" => "Reproduciendo {name}",
        "Any language; Gentle Shell's model reads it." => {
            "Escribe en cualquier idioma; el modelo de Gentle Shell lo interpreta."
        }
        "Design canvas" => "Lienzo de diseño",
        "The browser could not find the app's window." => {
            "El navegador no pudo encontrar la ventana de la app."
        }
        "The browser could not start: {error}" => "El navegador no pudo iniciarse: {error}",
        "The browser could not start: {error}. It needs the Microsoft Edge WebView2 Runtime." => {
            "El navegador no pudo iniciarse: {error}. Necesita Microsoft Edge WebView2 Runtime."
        }
        "The browser could not start GTK: {error}" => "El navegador no pudo iniciar GTK: {error}",
        "The browser could not start: {error}. It needs WebKitGTK 4.1." => {
            "El navegador no pudo iniciarse: {error}. Necesita WebKitGTK 4.1."
        }
        "The in-app browser needs an X11 session; on Wayland the canvas opens in your browser." => {
            "El navegador integrado necesita una sesión de X11. En Wayland, el lienzo se abre en tu navegador."
        }
        "Open an agent to insert paths into its prompt." => {
            "Abre un agente para insertar rutas en su mensaje."
        }
        "That isn't a theme file: it needs at least a \"name\"." => {
            "No es un archivo de tema: necesita al menos un campo \"name\"."
        }
        "There's no data folder to keep themes in." => {
            "No hay una carpeta de datos donde guardar los temas."
        }
        "This theme has no file." => "Este tema no tiene un archivo.",
        "Engram is not running. Gentle Shell starts it with the first session." => {
            "Engram no está en ejecución. Gentle Shell lo inicia con la primera sesión."
        }
        "Engram answered {status} without JSON" => "Engram respondió con {status} sin JSON",
        "Engram answered {status}" => "Engram respondió con {status}",
        "Turn on Use devices in Configuration → Devices first." => {
            "Primero activa Usar dispositivos en Configuración → Dispositivos."
        }
        "No free port for the device hub: {error}" => {
            "No hay un puerto libre para el centro de dispositivos: {error}"
        }
        "This system has no app data folder." => {
            "Este sistema no tiene una carpeta de datos para la app."
        }
        "Couldn't create {path}: {error}" => "No se pudo crear {path}: {error}",
        "Couldn't open the hub's log: {error}" => {
            "No se pudo abrir el registro del centro de dispositivos: {error}"
        }
        "The device hub didn't start: {error}" => "El centro de dispositivos no se inició: {error}",
        "The device hub stopped ({status}). {last}" => {
            "El centro de dispositivos se detuvo ({status}). {last}"
        }
        "The device hub didn't answer in time; see devices/hub.log." => {
            "El centro de dispositivos no respondió a tiempo. Consulta devices/hub.log."
        }
        "The device hub didn't answer: {error}" => {
            "El centro de dispositivos no respondió: {error}"
        }
        "The device hub's answer is unreadable: {error}" => {
            "No se puede leer la respuesta del centro de dispositivos: {error}"
        }
        "The device hub's list is unreadable: {error}" => {
            "No se puede leer la lista del centro de dispositivos: {error}"
        }
        "The device hub refused." => "El centro de dispositivos rechazó la solicitud.",
        "This system has no home or app data folder." => {
            "Este sistema no tiene una carpeta personal ni de datos de aplicaciones."
        }
        "{path} already exists and isn't Osdy Code Desktop's; remove it first." => {
            "{path} ya existe y no pertenece a Osdy Code Desktop; elimínalo primero."
        }
        "Could not write {path}: {error}" => "No se pudo escribir {path}: {error}",
        "Could not run PowerShell: {error}" => "No se pudo ejecutar PowerShell: {error}",
        "Could not add {path} to your PATH: {error}" => {
            "No se pudo agregar {path} a tu PATH: {error}"
        }
        "ocd is installed. Type ocd in any folder to open it here." => {
            "ocd está instalado. Escribe ocd en cualquier carpeta para abrirla aquí."
        }
        "ocd is installed. Open a new terminal and type ocd in any folder." => {
            "ocd está instalado. Abre una terminal nueva y escribe ocd en cualquier carpeta."
        }
        "ocd is installed in {path}, which isn't on your PATH yet: add it in your shell profile." => {
            "ocd está instalado en {path}, que aún no está en tu PATH. Agrégalo en el perfil de tu shell."
        }
        "gentle-stream didn't start: {error}" => "gentle-stream no se inició: {error}",
        "gentle-stream didn't answer in time." => "gentle-stream no respondió a tiempo.",
        "gentle-stream didn't answer" => "gentle-stream no respondió",
        "gentle-stream failed" => "gentle-stream falló",
        "Asks before risky actions" => "Pide permiso antes de acciones de riesgo",
        "A classifier decides with the conversation's context" => {
            "Un clasificador decide según el contexto de la conversación"
        }
        "Ordinary work runs freely; destructive confirmations remain" => {
            "El trabajo habitual se ejecuta sin pedir permiso; las acciones destructivas aún requieren confirmación"
        }
        "No permission prompts; private paths stay blocked" => {
            "No se pide permiso; las rutas privadas siguen bloqueadas"
        }
        "job {id}" => "tarea {id}",
        "exit {code}" => "salida {code}",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};

    #[test]
    fn shell_tools_translate_menu_and_browser_text_without_changing_english() {
        for (english, spanish) in [
            ("Reopen Closed Pane", "Reabrir panel cerrado"),
            ("Profile name", "Nombre del perfil"),
            (
                "The browser could not start: {error}. It needs the Microsoft Edge WebView2 Runtime.",
                "El navegador no pudo iniciarse: {error}. Necesita Microsoft Edge WebView2 Runtime.",
            ),
        ] {
            assert_eq!(tr(Language::English, english), english);
            assert_eq!(tr(Language::Spanish, english), spanish);
        }
    }
}
