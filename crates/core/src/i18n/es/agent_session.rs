//! Spanish text of the agent session area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        " at {commit}" => " en {commit}",
        "+1 line" => "+1 línea",
        "+{count} lines" => "+{count} líneas",
        "/{name} isn't available in the app yet." => "/{name} aún no está disponible en la app.",
        "/{name} only works in Gentle Shell's terminal." => {
            "/{name} solo funciona en la terminal de Gentle Shell."
        }
        "1 changed file · {repo}" => "1 archivo modificado · {repo}",
        "A quick glance" => "Un vistazo rápido",
        "Activity" => "Actividad",
        "Add what to keep with /compact <instructions>." => {
            "Indica qué conservar con /compact <instructions>."
        }
        "Agent answer" => "Respuesta del agente",
        "Agent" => "Agente",
        "Agents can write, update and check the docs." => {
            "Los agentes pueden redactar, actualizar y comprobar la documentación."
        }
        "All in" => "Máximo esfuerzo",
        "All repositories" => "Todos los repositorios",
        "An ADR from what was decided: why, the alternatives and what it costs." => {
            "Un ADR que recoge la decisión, el motivo, las alternativas y sus costos."
        }
        "Answer in Gentle Shell's terminal: {text}" => {
            "Responde en la terminal de Gentle Shell: {text}"
        }
        "Answers right away: fastest, lightest on limits" => {
            "Responde al instante: la opción más rápida y la que menos consume"
        }
        "Applies now to this conversation and to new ones." => {
            "Se aplica desde ahora a esta conversación y a las nuevas."
        }
        "Architecture docs" => "Documentación de arquitectura",
        "Ask Gentle Shell to cancel this helper over the Desktop channel" => {
            "Pide a Gentle Shell que cancele este subagente por el canal Desktop"
        }
        "Ask the agent" => "Pedir al agente",
        "Ask to stop" => "Pedir que se detenga",
        "Ask" => "Pedir",
        "Asked the agent to add it to the docs." => {
            "Se le pidió al agente que lo agregue a la documentación."
        }
        "Attach files or images (or paste / drop them)" => {
            "Adjuntar archivos o imágenes (también puedes pegarlos o arrastrarlos)"
        }
        "Attach" => "Adjuntar",
        "Back to agents" => "Volver a agentes",
        "Background job" => "Tarea en segundo plano",
        "Background jobs" => "Tareas en segundo plano",
        "Balanced" => "Equilibrado",
        "Barely thinks before answering" => "Apenas razona antes de responder",
        "Better on hard problems; uses limits faster" => {
            "Mejor para problemas difíciles; consume más rápido la cuota"
        }
        "Brings the documents whose code changed up to date." => {
            "Actualiza los documentos relacionados con el código que cambió."
        }
        "Cache read" => "Lectura de caché",
        "Cancel this job" => "Cancelar esta tarea",
        "Check the documents again" => "Volver a comprobar los documentos",
        "Choose model" => "Elegir modelo",
        "Choose" => "Elige",
        "Close (Esc)" => "Cerrar (Esc)",
        "Close" => "Cerrar",
        "Commands" => "Comandos",
        "Compact automatically when nearly full" => {
            "Compactar automáticamente cuando esté casi lleno"
        }
        "Compact now" => "Compactar ahora",
        "Compact the conversation context (optional: what to keep)" => {
            "Compactar el contexto de la conversación (opcional: indicar qué conservar)"
        }
        "Compacting summarizes the conversation to free context." => {
            "Compactar resume la conversación para liberar espacio en el contexto."
        }
        "Compacting…" => "Compactando…",
        "Consider compacting or starting a new session." => {
            "Considera compactar o iniciar una sesión nueva."
        }
        "Context nearly full. Compact or start a new session." => {
            "El contexto está casi lleno. Compáctalo o inicia una sesión nueva."
        }
        "Context usage and compaction" => "Uso y compactación del contexto",
        "Context window" => "Ventana de contexto",
        "Context" => "Contexto",
        "Copied to the clipboard." => "Se copió al portapapeles.",
        "Copy answer" => "Copiar respuesta",
        "Copy message" => "Copiar mensaje",
        "Copy path" => "Copiar ruta",
        "Copy the last answer to the clipboard" => "Copiar la última respuesta al portapapeles",
        "Copy" => "Copiar",
        "Cost" => "Costo",
        "Could not read {name}: {error}" => "No se pudo leer {name}: {error}",
        "Could not save the permission mode: {error}" => {
            "No se pudo guardar el modo de permisos: {error}"
        }
        "Could not start gentle-shell" => "No se pudo iniciar gentle-shell",
        "Couldn't install it: {error}" => "No se pudo instalar: {error}",
        "Couldn't run the docs check: {error}" => {
            "No se pudieron comprobar los documentos: {error}"
        }
        "Deep thinking" => "Razonamiento profundo",
        "Delegated {count} task" => "Delegó {count} tarea",
        "Delegated {count} tasks" => "Delegó {count} tareas",
        "Diagram renderer" => "Renderizador de diagramas",
        "Docs skill" => "Skill de documentación",
        "Document a feature" => "Documentar una función",
        "Document this project" => "Documentar este proyecto",
        "Documents" => "Documentos",
        "Done" => "Listo",
        "Draws the docs' diagrams in the preview. Optional: a one-time download of about 20 MB, not part of the app." => {
            "Muestra los diagramas de la documentación en la vista previa. Es opcional: se descarga una sola vez, pesa unos 20 MB y no viene con la app."
        }
        "Drop files here" => "Suelta los archivos aquí",
        "ERROR" => "ERROR",
        "Each opens an agent tab in this folder, so you can follow along and answer its questions." => {
            "Cada opción abre una pestaña de agente en esta carpeta para que puedas seguir el progreso y responder sus preguntas."
        }
        "Edit" => "Editar",
        "Edited {count} file" => "Editó {count} archivo",
        "Edited {count} files" => "Editó {count} archivos",
        "Effort" => "Esfuerzo",
        "Effort: {current}. How long the model thinks" => {
            "Esfuerzo: {current}. Cuánto razona el modelo"
        }
        "Exit code {code}." => "Código de salida {code}.",
        "Exploration budget" => "Presupuesto de exploración",
        "Export the conversation as an HTML file" => "Exportar la conversación a un archivo HTML",
        "Exported the conversation to {path}" => "Se exportó la conversación a {path}",
        "Extension request" => "Solicitud de una extensión",
        "Extra deep" => "Razonamiento muy profundo",
        "Failed" => "Falló",
        "Feature name, e.g. payments" => "Nombre de la función, p. ej., pagos",
        "Fetched {count} page" => "Consultó {count} página",
        "Fetched {count} pages" => "Consultó {count} páginas",
        "Files and images · folders aren’t attached" => {
            "Archivos e imágenes · no se adjuntan carpetas"
        }
        "Filter: active" => "Filtro: activos",
        "Filter: all" => "Filtro: todos",
        "Filter: finished" => "Filtro: finalizados",
        "Find" => "Buscar",
        "Gentle Desktop did not open a design canvas outside this machine." => {
            "Gentle Desktop no abrió un lienzo de diseño fuera de este equipo."
        }
        "Gentle Desktop events were lost; re-reading the configuration." => {
            "Se perdieron eventos de Gentle Desktop. Volviendo a leer la configuración."
        }
        "Gentle Desktop events were lost; re-reading the review mode." => {
            "Se perdieron eventos de Gentle Desktop. Volviendo a leer el modo de revisión."
        }
        "Gentle Desktop events were lost; resyncing helpers and jobs." => {
            "Se perdieron eventos de Gentle Desktop. Resincronizando los subagentes y las tareas."
        }
        "Gentle Desktop events were lost; resyncing subscription usage." => {
            "Se perdieron eventos de Gentle Desktop. Resincronizando el uso de la suscripción."
        }
        "Gentle Desktop events were lost; resyncing the channel." => {
            "Se perdieron eventos de Gentle Desktop. Resincronizando el canal."
        }
        "Gentle Desktop events were lost; resyncing the design canvas." => {
            "Se perdieron eventos de Gentle Desktop. Resincronizando el lienzo de diseño."
        }
        "Gentle Desktop is still loading; try again in a moment." => {
            "Gentle Desktop aún está cargando. Inténtalo de nuevo en un momento."
        }
        "Gentle Desktop is still loading; try the permission change again in a moment." => {
            "Gentle Desktop aún está cargando. Intenta cambiar los permisos de nuevo en un momento."
        }
        "Gentle Desktop's capabilities could not be read ({error}); its channel stays off in this tab." => {
            "No se pudieron leer las funciones de Gentle Desktop ({error}). El canal seguirá desactivado en esta pestaña."
        }
        "Gentle Desktop's snapshot could not be read ({error}); its channel stays off in this tab." => {
            "No se pudo leer el estado de Gentle Desktop ({error}). El canal seguirá desactivado en esta pestaña."
        }
        "Gentle Shell asks you to confirm before anything changes." => {
            "Gentle Shell te pide confirmación antes de hacer cambios."
        }
        "Gentle Shell can't read helper activity right now." => {
            "Gentle Shell no puede leer la actividad del subagente en este momento."
        }
        "Gentle Shell can't read job output right now." => {
            "Gentle Shell no puede leer la salida de la tarea en este momento."
        }
        "Gentle Shell could not read the review mode; nothing is assumed." => {
            "Gentle Shell no pudo leer el modo de revisión; no se da por hecho ningún ajuste."
        }
        "Gentle Shell did not run {PROMPT_COMMAND}. Is gentle-pi up to date?" => {
            "Gentle Shell no ejecutó {PROMPT_COMMAND}. ¿Está actualizado gentle-pi?"
        }
        "Gentle Shell did not run {text} (disposition {disposition}). Is gentle-pi up to date?" => {
            "Gentle Shell no ejecutó {text} (resultado: {disposition}). ¿Está actualizado gentle-pi?"
        }
        "Gentle Shell has not published an activity thread for this helper." => {
            "Gentle Shell no ha publicado un hilo de actividad para este subagente."
        }
        "Gentle Shell is still starting in this tab; try again in a moment." => {
            "Gentle Shell aún se está iniciando en esta pestaña. Inténtalo de nuevo en un momento."
        }
        "Gentle Shell no longer keeps this helper's activity." => {
            "Gentle Shell ya no conserva la actividad de este subagente."
        }
        "Gentle Shell reloads in this tab when the current turn ends." => {
            "Gentle Shell se volverá a cargar en esta pestaña al terminar el turno actual."
        }
        "Gentle Shell stops helpers from its agent: this asks it to run subagent_cancel" => {
            "Pide al agente de Gentle Shell que ejecute subagent_cancel para detener este subagente"
        }
        "Gentle Shell was updated; this tab switches to it when the current turn ends." => {
            "Gentle Shell se actualizó. Esta pestaña usará la nueva versión al terminar el turno actual."
        }
        "Helpers" => "Subagentes",
        "Hide tasks" => "Ocultar tareas",
        "How long the model reasons before answering" => {
            "Cuánto razona el modelo antes de responder"
        }
        "Id" => "ID",
        "Input" => "Entrada",
        "Insert UI template… (saved generative UI): /ui-template [name]" => {
            "Insertar una plantilla de interfaz generativa guardada: /ui-template [name]"
        }
        "Install the docs skill to ask for these." => {
            "Instala la skill de documentación para usar estas opciones."
        }
        "Install the docs skill to see which documents are current." => {
            "Instala la skill de documentación para ver qué documentos están al día."
        }
        "Installed. Agent tabs started from now on have it." => {
            "Instalado. Estará disponible en las pestañas de agente que abras a partir de ahora."
        }
        "Installing into Gentle Shell…" => "Instalando en Gentle Shell…",
        "Installing…" => "Instalando…",
        "Its components, data model, states and flows." => {
            "Sus componentes, modelo de datos, estados y flujos."
        }
        "Jobs are running in background" => "Hay tareas en segundo plano",
        "Jobs {total}" => "Tareas: {total}",
        "Jobs" => "Tareas",
        "Last step" => "Último paso",
        "Latest output" => "Salida más reciente",
        "Light thinking" => "Razonamiento ligero",
        "List" => "Lista",
        "Loading models…" => "Cargando modelos…",
        "Long reasoning; consumes usage limits faster" => {
            "Razonamiento prolongado; consume más rápido la cuota"
        }
        "Maps the code and writes the structure, the index and every document it can ground in it. Shows the plan first." => {
            "Analiza el código y redacta la estructura, el índice y los documentos que pueda respaldar con él. Primero muestra el plan."
        }
        "Mermaid diagrams are drawn in the Markdown preview." => {
            "La vista previa de Markdown muestra los diagramas de Mermaid."
        }
        "Message" => "Mensaje",
        "Model" => "Modelo",
        "Monitor" => "Monitor",
        "New sign-in saved; this tab reloads its models when the current turn ends." => {
            "Se guardó el inicio de sesión. Esta pestaña volverá a cargar los modelos al terminar el turno actual."
        }
        "No UI template named “{name}” ({known})." => {
            "No hay una plantilla de interfaz llamada «{name}» ({known})."
        }
        "No background jobs yet. Commands the agent runs in the background show up here." => {
            "Aún no hay tareas en segundo plano. Aquí aparecen los comandos que el agente ejecuta en segundo plano."
        }
        "No docs yet. \u{201c}Document this project\u{201d} starts them." => {
            "Aún no hay documentación. Empieza con «Documentar este proyecto»."
        }
        "No model" => "Sin modelo",
        "No models match." => "No hay modelos que coincidan.",
        "No models yet. Gentle Shell on {backend} signs in there: /login." => {
            "Aún no hay modelos. Inicia sesión en Gentle Shell desde {backend} con /login."
        }
        "No models yet. Sign in to a provider." => {
            "Aún no hay modelos. Inicia sesión en un proveedor."
        }
        "No output yet." => "Aún no hay salida.",
        "No plan usage reported yet." => "Aún no se ha informado del uso del plan.",
        "No subagents yet. When el Gentleman delegates work, helpers appear here live." => {
            "Aún no hay subagentes. Cuando el Gentleman delegue trabajo, aparecerán aquí en tiempo real."
        }
        "No thinking" => "Sin razonamiento",
        "No" => "No",
        "Node.js wasn't found to run the docs check." => {
            "No se encontró Node.js para comprobar los documentos."
        }
        "Not set" => "Sin configurar",
        "Off for this repository." => "Desactivado para este repositorio.",
        "Off machine-wide, which wins over this repository's setting." => {
            "Desactivado en todo el equipo; este ajuste prevalece sobre el del repositorio."
        }
        "Off." => "Desactivado.",
        "On: nothing turned it off." => "Activado: nada lo desactivó.",
        "On: turned on for this repository." => "Activado para este repositorio.",
        "On: turned on machine-wide." => "Activado en todo el equipo.",
        "Only http:// and https:// URLs can open here." => {
            "Aquí solo se pueden abrir URL con http:// o https://."
        }
        "Open Agents ({shortcut})" => "Abrir Agentes ({shortcut})",
        "Open Memory" => "Abrir Memoria",
        "Open a URL in the in-app browser: /web http://localhost:3000" => {
            "Abrir una URL en el navegador de la app: /web http://localhost:3000"
        }
        "Open file" => "Abrir archivo",
        "Open folder" => "Abrir carpeta",
        "Open in default app" => "Abrir en la app predeterminada",
        "Open memory #{id}" => "Abrir recuerdo #{id}",
        "Open transcript" => "Abrir transcripción",
        "Open {project}'s memories" => "Abrir los recuerdos de {project}",
        "Output" => "Salida",
        "Permission mode: {label}" => "Modo de permisos: {label}",
        "Permissions switch to {mode} when the current turn ends." => {
            "Los permisos cambiarán a {mode} al terminar el turno actual."
        }
        "Permissions" => "Permisos",
        "Permissions: {mode}" => "Permisos: {mode}",
        "Plan limits need Gentle Shell 4.8.0 or newer" => {
            "Los límites del plan requieren Gentle Shell 4.8.0 o posterior"
        }
        "Plan usage limits · {plan}" => "Límites de uso del plan · {plan}",
        "Plan usage limits" => "Límites de uso del plan",
        "Quick reasoning for simple tasks" => "Razonamiento rápido para tareas sencillas",
        "Ran {count} command" => "Ejecutó {count} comando",
        "Ran {count} commands" => "Ejecutó {count} comandos",
        "Read {count} file" => "Leyó {count} archivo",
        "Read {count} files" => "Leyó {count} archivos",
        "Read" => "Leer",
        "Reading docs/…" => "Leyendo docs/…",
        "Reading the output…" => "Leyendo la salida…",
        "Reading this helper's activity…" => "Leyendo la actividad de este subagente…",
        "Reasoning" => "Razonamiento",
        "Record a decision" => "Registrar una decisión",
        "Rename this session: /name <title>" => "Cambiar el nombre de esta sesión: /name <title>",
        "Restart Gentle Shell, keeping the conversation" => {
            "Reiniciar Gentle Shell sin perder la conversación"
        }
        "Retry" => "Reintentar",
        "Review ?" => "Revisión: ¿?",
        "Review changes · {count} file" => "Revisar cambios · {count} archivo",
        "Review changes · {count} files" => "Revisar cambios · {count} archivos",
        "Review mode could not be changed: {message}" => {
            "No se pudo cambiar el modo de revisión: {message}"
        }
        "Review mode is already being changed; try again in a moment." => {
            "El modo de revisión ya está cambiando. Inténtalo de nuevo en un momento."
        }
        "Review mode" => "Modo de revisión",
        "Review off" => "Revisión desactivada",
        "Review on" => "Revisión activada",
        "Review preflight" => "Comprobación previa a la revisión",
        "Review" => "Revisión",
        "Running" => "En ejecución",
        "Screen" => "Pantalla",
        "Search models…" => "Buscar modelos…",
        "Searched {count} time" => "Buscó {count} vez",
        "Searched {count} times" => "Buscó {count} veces",
        "See all →" => "Ver todos →",
        "Select a file" => "Elige un archivo",
        "Send" => "Enviar",
        "Session cost" => "Costo de la sesión",
        "Set up" => "Configurar",
        "Shell commands cannot carry attachments. Remove them or drop the leading !." => {
            "Los comandos de terminal no admiten adjuntos. Quita los adjuntos o elimina el ! inicial."
        }
        "Shell" => "Terminal",
        "Show as checklist" => "Mostrar como lista de verificación",
        "Show fewer lines" => "Mostrar menos líneas",
        "Show less" => "Mostrar menos",
        "Show tasks" => "Mostrar tareas",
        "Sign in to a provider…" => "Iniciar sesión en un proveedor…",
        "Sign in to another provider…" => "Iniciar sesión en otro proveedor…",
        "Starting gentle-shell…" => "Iniciando gentle-shell…",
        "Steer the running agent" => "Orientar al agente en marcha",
        "Stop (esc · {shortcut})" => "Detener (esc · {shortcut})",
        "Stop ({shortcut})" => "Detener ({shortcut})",
        "Stop the agent" => "Detener el agente",
        "Stop" => "Detener",
        "Subagents are running in background" => "Hay subagentes trabajando en segundo plano",
        "Submit" => "Enviar",
        "Switches to {mode} when the current turn ends." => {
            "Cambia a {mode} al terminar el turno actual."
        }
        "Task" => "Tarea",
        "Teaches the agent to write and keep these docs. Installs into Gentle Shell." => {
            "Enseña al agente a redactar y mantener estos documentos. Se instala en Gentle Shell."
        }
        "That helper or job is no longer running." => {
            "Ese subagente o tarea ya no está en ejecución."
        }
        "The cancel request failed ({error})." => "No se pudo cancelar ({error}).",
        "The cancel request timed out; it was not applied." => {
            "La solicitud de cancelación agotó el tiempo de espera y no se aplicó."
        }
        "The configuration request failed ({error})." => {
            "No se pudo consultar la configuración ({error})."
        }
        "The configuration request timed out." => {
            "Se agotó el tiempo de espera de la solicitud de configuración."
        }
        "The decision, e.g. Keep balances in minor units" => {
            "La decisión, p. ej., guardar los saldos en unidades mínimas"
        }
        "The design canvas could not be read ({error})." => {
            "No se pudo leer el lienzo de diseño ({error})."
        }
        "The design canvas request timed out." => {
            "Se agotó el tiempo de espera de la solicitud del lienzo de diseño."
        }
        "The docs check gave no report" => "La comprobación no generó un informe",
        "The output could not be read." => "No se pudo leer la salida.",
        "The pasted image" => "La imagen pegada",
        "The permission mode change failed ({error})." => {
            "No se pudo cambiar el modo de permisos ({error})."
        }
        "The permission mode changed, but the new default was not saved ({code}: {message})." => {
            "El modo de permisos cambió, pero no se guardó como predeterminado ({code}: {message})."
        }
        "The permission request timed out; it was not applied." => {
            "La solicitud de permisos agotó el tiempo de espera y no se aplicó."
        }
        "The review mode request failed ({error})." => {
            "Falló la solicitud del modo de revisión ({error})."
        }
        "The review mode request timed out." => {
            "Se agotó el tiempo de espera de la solicitud del modo de revisión."
        }
        "The subscription usage request failed ({error})." => {
            "No se pudo consultar el uso de la suscripción ({error})."
        }
        "The subscription usage request timed out." => {
            "Se agotó el tiempo de espera al consultar el uso de la suscripción."
        }
        "The template couldn't be inserted: {error}" => "No se pudo insertar la plantilla: {error}",
        "There is no answer to copy yet." => "Aún no hay una respuesta que copiar.",
        "Thinking" => "Razonamiento",
        "Thinking…" => "Pensando…",
        "Thinks as long as it takes; slowest and costliest" => {
            "Razona cuanto haga falta; la opción más lenta y costosa"
        }
        "Thinks it through without dragging" => "Razona sin demorarse",
        "This Gentle Shell does not provide Gentle Desktop. Update gentle-pi, then reload the tab." => {
            "Esta versión de Gentle Shell no incluye Gentle Desktop. Actualiza gentle-pi y vuelve a cargar la pestaña."
        }
        "This Gentle Shell does not report its design canvas to Desktop." => {
            "Esta versión de Gentle Shell no informa de su lienzo de diseño a Desktop."
        }
        "This Gentle Shell does not serve its configuration to Desktop." => {
            "Esta versión de Gentle Shell no comparte su configuración con Desktop."
        }
        "This Gentle Shell does not support changing the permission mode from Desktop." => {
            "Esta versión de Gentle Shell no permite cambiar el modo de permisos desde Desktop."
        }
        "This Gentle Shell does not support reading subscription usage from Desktop." => {
            "Esta versión de Gentle Shell no permite consultar el uso de la suscripción desde Desktop."
        }
        "This Gentle Shell does not support stopping helpers and jobs from Desktop." => {
            "Esta versión de Gentle Shell no permite detener subagentes ni tareas desde Desktop."
        }
        "This helper has no activity yet." => "Este subagente aún no tiene actividad.",
        "This helper's activity could not be read." => {
            "No se pudo leer la actividad de este subagente."
        }
        "This job's output is no longer available." => {
            "La salida de esta tarea ya no está disponible."
        }
        "This repository" => "Este repositorio",
        "This session has no name yet. Name it with /name <title>." => {
            "Esta sesión aún no tiene nombre. Asígnale uno con /name <title>."
        }
        "This session is named “{name}”. Rename it with /name <title>." => {
            "Esta sesión se llama «{name}». Cambia su nombre con /name <title>."
        }
        "Thought" => "Razonamiento",
        "Time" => "Tiempo",
        "Todo" => "Tareas",
        "Todos" => "Tareas",
        "Tokens" => "Tokens",
        "Tools" => "Herramientas",
        "Turns" => "Turnos",
        "Unknown" => "Desconocido",
        "Update after changes" => "Actualizar después de los cambios",
        "Usage" => "Uso",
        "Usage: /web <url>" => "Uso: /web <url>",
        "Used tokens" => "Tokens usados",
        "Used {count} tool" => "Usó {count} herramienta",
        "Used {count} tools" => "Usó {count} herramientas",
        "Verification reminder" => "Recordatorio de verificación",
        "Waiting for Gentle Shell…" => "Esperando a Gentle Shell…",
        "Waiting for an answer: {text}" => "Esperando una respuesta: {text}",
        "What are we cooking?" => "¿Qué vamos a preparar?",
        "Working tree clean" => "No hay cambios en el árbol de trabajo",
        "Working…" => "Trabajando…",
        "Write" => "Escribir",
        "Yes" => "Sí",
        "authorizing" => "autorizando",
        "cancel" => "cancelar",
        "checking" => "verificando",
        "choose" => "elegir",
        "closing" => "cerrando",
        "compacting…" => "compactando…",
        "current: {current}" => "actual: {current}",
        "deciding" => "decidiendo",
        "exit 0" => "salida 0",
        "exploring" => "explorando",
        "gentle-shell install failed" => "Falló la instalación con gentle-shell",
        "gentle-shell stopped" => "gentle-shell se detuvo",
        "implementing" => "implementando",
        "issues" => "con incidencias",
        "no exit code" => "sin código de salida",
        "no" => "no",
        "none saved yet" => "aún no hay ninguna guardada",
        "planning" => "planificando",
        "reasoning" => "razonamiento",
        "researching" => "investigando",
        "stale" => "desactualizado",
        "submit" => "enviar",
        "the agent" => "el agente",
        "unknown" => "desconocido",
        "unverified" => "sin verificar",
        "updating…" => "actualizando…",
        "yes" => "sí",
        "{agent} message" => "Mensaje de {agent}",
        "{count} active" => "En actividad: {count}",
        "{count} changed files · {repo}" => "{count} archivos modificados · {repo}",
        "{count} documents, all current{commit}." => "{count} documentos, todos al día{commit}.",
        "{count} documents, {attention} need attention: “Update after changes” catches them up." => {
            "{count} documentos; {attention} requieren atención. Usa «Actualizar después de los cambios» para ponerlos al día."
        }
        "{count} earlier lines" => "{count} líneas anteriores",
        "{count} finished" => "Finalizados: {count}",
        "{count} job" => "{count} tarea",
        "{count} more lines" => "{count} líneas más",
        "{count} more waiting" => "{count} más en espera",
        "{count} queued" => "En cola: {count}",
        "{count} subagent" => "{count} subagente",
        "{count} tools" => "{count} herramientas",
        "{count} turns" => "{count} turnos",
        "{folder}/docs, written and kept current by the agent" => {
            "{folder}/docs, redactados y actualizados por el agente"
        }
        "{label} (receipt-driven development)" => "{label} (desarrollo guiado por comprobantes)",
        "{label} · switches to {mode} when the current turn ends" => {
            "{label} · cambia a {mode} al terminar el turno actual"
        }
        "{model} does not accept images. Switch to a model with image input, or remove the attachments." => {
            "{model} no admite imágenes. Cambia a un modelo que las admita o quita los adjuntos."
        }
        "{name} has a control character in its path; rename it to attach it." => {
            "La ruta de {name} contiene un carácter de control. Cambia su nombre para adjuntarlo."
        }
        "{name} has a line break in its path; rename it to attach it." => {
            "La ruta de {name} contiene un salto de línea. Cambia su nombre para adjuntarlo."
        }
        "{name} is a folder; attach files instead." => {
            "{name} es una carpeta. Adjunta archivos en su lugar."
        }
        "{name} is no longer there; remove it from the attachments to send." => {
            "{name} ya no está disponible. Quítalo de los adjuntos para enviar el mensaje."
        }
        "{name} is not a PNG, JPEG, GIF, WebP, BMP or TIFF image." => {
            "{name} no es una imagen PNG, JPEG, GIF, WebP, BMP ni TIFF."
        }
        "{name} is not valid UTF-8; rename it to attach it." => {
            "{name} no es UTF-8 válido. Cambia su nombre para adjuntarlo."
        }
        "{name} is {size} MB; images must be 20 MB or smaller." => {
            "{name} pesa {size} MB. Las imágenes no pueden superar los 20 MB."
        }
        "{path} is outside the workspace." => "{path} está fuera del espacio de trabajo.",
        "{path} isn't in the workspace." => "{path} no está en el espacio de trabajo.",
        "{running} running · {finished} finished" => {
            "En ejecución: {running} · Finalizadas: {finished}"
        }
        "… earlier items not shown" => "… se ocultaron los elementos anteriores",
        "… earlier output not shown" => "… se ocultó la salida anterior",
        "… {count} earlier items dropped" => "… se descartaron {count} elementos anteriores",
        "… {count} earlier lines not shown" => "… se ocultaron {count} líneas anteriores",
        "… {count} more lines not shown" => "… se ocultaron {count} líneas más",
        "… {count} more lines" => "… {count} líneas más",
        "↵ insert · tab folders" => "↵ insertar · tab carpetas",
        "stopping…" => "deteniendo…",
        "waiting for {provider}…" => "esperando a {provider}…",
        "working…" => "trabajando…",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};

    #[test]
    fn translates_activity_labels_without_requiring_an_icon_prefix() {
        for (english, spanish) in [
            ("stopping…", "deteniendo…"),
            ("waiting for {provider}…", "esperando a {provider}…"),
            ("working…", "trabajando…"),
        ] {
            assert_eq!(tr(Language::Spanish, english), spanish);
            assert_eq!(tr(Language::English, english), english);
        }
    }

    #[test]
    fn translates_session_and_document_labels_without_changing_english() {
        for (english, spanish) in [
            ("Copy answer", "Copiar respuesta"),
            ("Document this project", "Documentar este proyecto"),
        ] {
            assert_eq!(tr(Language::Spanish, english), spanish);
            assert_eq!(tr(Language::English, english), english);
        }
    }
}
