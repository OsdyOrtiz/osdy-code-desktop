//! Spanish text of the genui area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        "sees and draws on this canvas" => "ve este lienzo y dibuja en él",
        "Hide (keeps working)" => "Ocultar (sigue trabajando)",
        "End this agent" => "Finalizar este agente",
        "Ellipse (O) · click again for rectangle" => {
            "Elipse (O) · haz clic de nuevo para usar el rectángulo"
        }
        "Rectangle (R) · click again for ellipse" => {
            "Rectángulo (R) · haz clic de nuevo para usar la elipse"
        }
        "Select (V)" => "Seleccionar (V)",
        "Lasso (S)" => "Lazo (S)",
        "Hand (H, or hold Space)" => "Mano (H o mantén presionada la barra espaciadora)",
        "Pen (P)" => "Pluma (P)",
        "Highlighter (M)" => "Resaltador (M)",
        "Eraser (E)" => "Borrador (E)",
        "Laser pointer (K)" => "Puntero láser (K)",
        "Arrow (A)" => "Flecha (A)",
        "Line (L)" => "Línea (L)",
        "Text (T)" => "Texto (T)",
        "Sticky note (N)" => "Nota adhesiva (N)",
        "Image" => "Imagen",
        "Agent: reads, sees and draws on this canvas" => "Agente: lee, ve y dibuja en este lienzo",
        "Keep the tool after drawing" => "Mantener la herramienta después de dibujar",
        "Undo" => "Deshacer",
        "Redo" => "Rehacer",
        "Zoom out (−)" => "Alejar (−)",
        "Zoom in (=)" => "Acercar (=)",
        "Zoom to 100% (⇧0)" => "Zoom al 100 % (⇧0)",
        "Zoom to fit (⇧1)" => "Ajustar a la vista (⇧1)",
        "Minimap" => "Minimapa",
        "Select all" => "Seleccionar todo",
        "Bring to front" => "Traer al frente",
        "Send to back" => "Enviar al fondo",
        "Zoom to fit" => "Ajustar a la vista",
        "Clear canvas" => "Limpiar lienzo",
        "Stroke" => "Trazo",
        "Fill" => "Relleno",
        "Size" => "Tamaño",
        "Ink" => "Tinta",
        "Orange" => "Naranja",
        "Yellow" => "Amarillo",
        "Emerald" => "Esmeralda",
        "Cyan" => "Cian",
        "Sky" => "Celeste",
        "Violet" => "Violeta",
        "Pink" => "Rosa",
        "Indigo" => "Índigo",
        "Slate" => "Pizarra",
        "Light slate" => "Pizarra clara",
        "Pale slate" => "Pizarra pálida",
        "Paper" => "Papel",
        "Monoline" => "Trazo uniforme",
        "Fountain pen" => "Pluma estilográfica",
        "width follows speed" => "el grosor cambia con la velocidad",
        "one even width" => "grosor uniforme",
        "hairlines when fast, full swells when slow, tapered end" => {
            "trazo fino al ir rápido, grueso al ir despacio, con punta afinada"
        }
        "Couldn't read this canvas: {error}" => "No se pudo leer este lienzo: {error}",
        "Couldn't open this canvas: {error}" => "No se pudo abrir este lienzo: {error}",
        "Insert" => "Insertar",
        "Save" => "Guardar",
        "Open" => "Abrir",
        "Discard" => "Descartar",
        "Resume" => "Reanudar",
        "Expand" => "Expandir",
        "Unpin" => "Desanclar",
        "Title" => "Título",
        "Description" => "Descripción",
        "Assignee" => "Responsable",
        "Tags, comma-separated" => "Etiquetas separadas por comas",
        "Filter rows…" => "Filtrar filas…",
        "No rows match the filter." => "Ninguna fila coincide con el filtro.",
        "Ask about this row" => "Preguntar por esta fila",
        "{count} row" => "{count} fila",
        "{count} rows" => "{count} filas",
        "{visible} of {total} rows" => "{visible} de {total} filas",
        "{count} column" => "{count} columna",
        "{count} columns" => "{count} columnas",
        "{count} selected" => "Selección: {count}",
        "{count} node" => "{count} nodo",
        "{count} nodes" => "{count} nodos",
        "{count} edge" => "{count} conexión",
        "{count} edges" => "{count} conexiones",
        "{count} change" => "{count} cambio",
        "{count} changes" => "{count} cambios",
        "{count} series" => "{count} series",
        "{done} of {total} done" => "{done} de {total} completados",
        "Table" => "Tabla",
        "Form" => "Formulario",
        "Checklist" => "Lista de verificación",
        "Board" => "Tablero",
        "Chart" => "Gráfico",
        "Sequence" => "Secuencia",
        "Data model" => "Modelo de datos",
        "State machine" => "Máquina de estados",
        "Timeline" => "Cronograma",
        "Diagram" => "Diagrama",
        "Click a header to sort · check rows to select · double-click a cell to edit" => {
            "Haz clic en un encabezado para ordenar · marca filas para seleccionarlas · haz doble clic en una celda para editarla"
        }
        "Click a header to sort · filter · select rows to send them" => {
            "Haz clic en un encabezado para ordenar · filtra · selecciona las filas que quieras enviar"
        }
        "Fill in and submit; the agent gets every answer at once" => {
            "Completa el formulario y envíalo; el agente recibe todas las respuestas juntas"
        }
        "Click to check · double-click to rename · drag ⋮⋮ to reorder · Tab to indent" => {
            "Haz clic para marcar · haz doble clic para cambiar el nombre · arrastra ⋮⋮ para reordenar · usa Tab para aplicar sangría"
        }
        "Drag cards between columns · click one for its details" => {
            "Arrastra tarjetas entre columnas · haz clic en una para ver sus detalles"
        }
        "Hover for values · click one to ask about it" => {
            "Pasa el cursor para ver los valores · haz clic en uno para preguntar por él"
        }
        "j/k to move · a accept · r reject · c comment · Shift-A/R the whole file" => {
            "j/k para moverte · a para aceptar · r para rechazar · c para comentar · Shift-A/R para todo el archivo"
        }
        "Drag a lifeline to another to add a message · drag messages and participants to reorder · double-click to rename" => {
            "Arrastra una línea de vida a otra para añadir un mensaje · arrastra mensajes y participantes para reordenarlos · haz doble clic para cambiar el nombre"
        }
        "Drag tables to move · drag ● from a column to another to relate them · double-click to rename" => {
            "Arrastra las tablas para moverlas · arrastra ● de una columna a otra para relacionarlas · haz doble clic para cambiar el nombre"
        }
        "Drag states to move · drag ● to add a transition · double-click to rename" => {
            "Arrastra los estados para moverlos · arrastra ● para añadir una transición · haz doble clic para cambiar el nombre"
        }
        "Drag a bar to move it, its ends to resize · drag ● to another task to link · double-click to rename" => {
            "Arrastra una barra para moverla o sus extremos para ajustar la duración · arrastra ● a otra tarea para vincularlas · haz doble clic para cambiar el nombre"
        }
        "Nothing drawn yet. Add a node with +." => "No hay nada dibujado. Añade un nodo con +.",
        "Nothing drawn yet. Add a participant with +." => {
            "No hay nada dibujado. Añade un participante con +."
        }
        "No tables yet. Add one with +." => "Todavía no hay tablas. Añade una con +.",
        "No states yet. Add one with +." => "Todavía no hay estados. Añade uno con +.",
        "No tasks yet. Add one with +." => "Todavía no hay tareas. Añade una con +.",
        "the spec must be a JSON object" => "la especificación debe ser un objeto JSON",
        "\"columns\" must be an array" => "\"columns\" debe ser una lista",
        "\"rows\" must be an array" => "\"rows\" debe ser una lista",
        "\"fields\" must be an array" => "\"fields\" debe ser una lista",
        "\"items\" must be an array" => "\"items\" debe ser una lista",
        "\"cards\" must be an array" => "\"cards\" debe ser una lista",
        "\"series\" must be an array" => "\"series\" debe ser una lista",
        "\"files\" must be an array" => "\"files\" debe ser una lista",
        "\"participants\" must be an array" => "\"participants\" debe ser una lista",
        "\"tables\" must be an array" => "\"tables\" debe ser una lista",
        "\"states\" must be an array" => "\"states\" debe ser una lista",
        "\"tasks\" must be an array" => "\"tasks\" debe ser una lista",
        "a table needs at least one column" => "una tabla necesita al menos una columna",
        "a board needs at least one column" => "un tablero necesita al menos una columna",
        "a form needs at least one field" => "un formulario necesita al menos un campo",
        "a review needs at least one file" => "una revisión necesita al menos un archivo",
        "a chart needs at least one series" => "un gráfico necesita al menos una serie",
        "a live chart needs labels: any type but scatter" => {
            "un gráfico en vivo necesita etiquetas: sirve cualquier tipo excepto dispersión"
        }
        "\"sort\" needs a column" => "\"sort\" necesita una columna",
        "this table's rows can't be selected" => "no se pueden seleccionar las filas de esta tabla",
        "this table is read-only" => "esta tabla es de solo lectura",
        "a cell holds text, a number or a link" => "una celda admite texto, un número o un enlace",
        "a new card needs an id and a title" => "una tarjeta nueva necesita un ID y un título",
        "a card needs a title" => "una tarjeta necesita un título",
        "an item needs a label" => "un elemento necesita una etiqueta",
        "a new item needs an id and a label" => "un elemento nuevo necesita un ID y una etiqueta",
        "a sub-item can't have its own items" => "un subelemento no puede contener otros elementos",
        "a table needs a name" => "una tabla necesita un nombre",
        "a new table needs an id and a name" => "una tabla nueva necesita un ID y un nombre",
        "a column needs a name" => "una columna necesita un nombre",
        "a new relation needs an id" => "una relación nueva necesita un ID",
        "a new state needs an id and a label" => "un estado nuevo necesita un ID y una etiqueta",
        "a state needs a label" => "un estado necesita una etiqueta",
        "a new transition needs an id" => "una transición nueva necesita un ID",
        "a new node needs an id" => "un nodo nuevo necesita un ID",
        "a new edge needs an id" => "una conexión nueva necesita un ID",
        "a new group needs an id" => "un grupo nuevo necesita un ID",
        "a task can't end before it starts" => "una tarea no puede terminar antes de empezar",
        "a new task needs an id and a label" => "una tarea nueva necesita un ID y una etiqueta",
        "a task needs a label" => "una tarea necesita una etiqueta",
        "a new participant needs an id and a label" => {
            "un participante nuevo necesita un ID y una etiqueta"
        }
        "a participant needs a label" => "un participante necesita una etiqueta",
        "a new message needs an id" => "un mensaje nuevo necesita un ID",
        "only a message to its own sender can be a self call" => {
            "solo un mensaje dirigido a su propio remitente puede ser una llamada a sí mismo"
        }
        "unknown kind \"{detail}\"" => "tipo desconocido \"{detail}\"",
        "unknown column \"{detail}\"" => "columna desconocida \"{detail}\"",
        "unknown row \"{detail}\"" => "fila desconocida \"{detail}\"",
        "unknown node \"{detail}\"" => "nodo desconocido \"{detail}\"",
        "unknown edge \"{detail}\"" => "conexión desconocida \"{detail}\"",
        "unknown group \"{detail}\"" => "grupo desconocido \"{detail}\"",
        "node \"{detail}\" already exists" => "el nodo \"{detail}\" ya existe",
        "edge \"{detail}\" already exists" => "la conexión \"{detail}\" ya existe",
        "group \"{detail}\" already exists" => "el grupo \"{detail}\" ya existe",
        "too many nodes (at most {detail})" => "demasiados nodos (máximo {detail})",
        "too many edges (at most {detail})" => "demasiadas conexiones (máximo {detail})",
        "too many groups (at most {detail})" => "demasiados grupos (máximo {detail})",
        "the output isn't JSON ({error}): {output}" => "la salida no es JSON ({error}): {output}",
        "The agent's latest version can't be drawn" => {
            "No se puede dibujar la última versión del agente"
        }
        "This can't be drawn" => "No se puede dibujar",
        "Click to expand" => "Haz clic para expandir",
        "Back to the conversation" => "Volver a la conversación",
        "Add a note…" => "Añadir una nota…",
        "Add a note for the agent (optional)" => "Añade una nota para el agente (opcional)",
        "Sent to the agent" => "Enviado al agente",
        "Send to agent" => "Enviar al agente",
        "Send decisions" => "Enviar decisiones",
        "View only · unlock to edit" => "Solo lectura · desbloquea para editar",
        "Drag to move · double-click to rename · drag ● to connect · shift-drag to select · ⌘/Ctrl-scroll to zoom" => {
            "Arrastra para mover · haz doble clic para cambiar el nombre · arrastra ● para conectar · arrastra con Shift para seleccionar · desplázate con ⌘/Ctrl para hacer zoom"
        }
        "Left to right (switch to top to bottom)" => {
            "De izquierda a derecha (cambiar a arriba-abajo)"
        }
        "Top to bottom (switch to left to right)" => {
            "De arriba abajo (cambiar a izquierda-derecha)"
        }
        "Zoom in" => "Acercar",
        "Zoom out" => "Alejar",
        "Fit the whole view" => "Ajustar toda la vista",
        "Fit the whole diagram" => "Ajustar todo el diagrama",
        "Unlock editing" => "Desbloquear edición",
        "Lock: view only" => "Bloquear: solo lectura",
        "Add node" => "Añadir nodo",
        "Group the selected nodes" => "Agrupar los nodos seleccionados",
        "Group" => "Agrupar",
        "Ungroup" => "Desagrupar",
        "No group" => "Sin grupo",
        "New group…" => "Nuevo grupo…",
        "Add item" => "Añadir elemento",
        "Add card" => "Añadir tarjeta",
        "Add participant" => "Añadir participante",
        "Add table" => "Añadir tabla",
        "Add state" => "Añadir estado",
        "Add task" => "Añadir tarea",
        "Ask about this" => "Preguntar sobre esto",
        "Open in the expanded view" => "Abrir en la vista expandida",
        "open in the expanded view" => "abierto en la vista expandida",
        "pinned to the side panel" => "anclado al panel lateral",
        "Back here" => "Volver aquí",
        "closed by the agent" => "cerrado por el agente",
        "v{version} · updated below" => "v{version} · versión nueva más abajo",
        "You asked to add {title} to the docs" => "Pediste añadir {title} a la documentación",
        "as {format}" => "en formato {format}",
        "You inserted {title} from a template" => "Insertaste {title} desde una plantilla",
        "You, from {title}" => "Tú, desde {title}",
        "You reviewed {title}" => "Revisaste {title}",
        "You edited {title}" => "Editaste {title}",
        "… {count} more" => "… {count} más",
        "Open {path}" => "Abrir {path}",
        "{path} isn't in the workspace (yet)" => "{path} todavía no está en el espacio de trabajo",
        "{path} is outside the workspace" => "{path} está fuera del espacio de trabajo",
        "Open in the file viewer" => "Abrir en el visor de archivos",
        "Open in {editor}" => "Abrir en {editor}",
        "Looking for editors…" => "Buscando editores…",
        "UI templates" => "Plantillas de interfaz",
        "No templates yet. Save one from a surface's Export menu (Save as template)." => {
            "Todavía no hay plantillas. Usa Guardar como plantilla en el menú Exportar de una interfaz."
        }
        "Couldn't rename it: {error}" => "No se pudo cambiar el nombre: {error}",
        "Couldn't rename it: no data folder" => {
            "No se pudo cambiar el nombre: no hay carpeta de datos"
        }
        "Couldn't delete it: {error}" => "No se pudo eliminar: {error}",
        "Export" => "Exportar",
        "Patch" => "Parche",
        "Unpin from the side panel" => "Desanclar del panel lateral",
        "Pin to the side panel" => "Anclar al panel lateral",
        "Copy to clipboard" => "Copiar al portapapeles",
        "Save to file…" => "Guardar en un archivo…",
        "Ask agent to add to docs" => "Pedir al agente que lo añada a la documentación",
        "Save as template" => "Guardar como plantilla",
        "Copied “{title}” as {format}" => "Se copió «{title}» en formato {format}",
        "Saved {path}" => "Se guardó {path}",
        "Couldn't save {path}: {error}" => "No se pudo guardar {path}: {error}",
        "Couldn't ask where to save: {error}" => {
            "No se pudo abrir el diálogo para guardar: {error}"
        }
        "no data folder to keep templates in" => "no hay carpeta de datos para guardar plantillas",
        "the picture has no size" => "la imagen no tiene dimensiones",
        "a {kind} can't be exported as {format}" => {
            "no se puede exportar {kind} en formato {format}"
        }
        "a {kind} has no docs format" => "{kind} no tiene un formato para documentación",
        "edge \"{first}\": node \"{second}\" does not exist" => {
            "conexión \"{first}\": no existe el nodo \"{second}\""
        }
        "node \"{first}\": group \"{second}\" is not in groups" => {
            "nodo \"{first}\": no está el grupo \"{second}\" en la lista de grupos"
        }
        "Saved the template “{name}”; insert it with /ui-template" => {
            "Se guardó la plantilla «{name}»; insértala con /ui-template"
        }
        "Submitted to the agent" => "Formulario enviado al agente",
        "It can reopen the form to ask again" => "El agente puede volver a abrir el formulario",
        "Submitting…" => "Enviando…",
        "Not sent yet" => "Aún no se ha enviado",
        "Edit answers" => "Editar respuestas",
        "1 answer needs a fix" => "Corrige 1 respuesta",
        "{count} answers need a fix" => "Corrige {count} respuestas",
        "The agent gets every answer at once" => "El agente recibe todas las respuestas juntas",
        "can't be shown" => "no se puede mostrar",
        "isolated · no network" => "aislado · sin acceso a la red",
        "may reach {origins}" => "puede conectarse a {origins}",
        "Your message from the page reached the agent" => {
            "El agente recibió tu mensaje de la página"
        }
        "An interactive page from the agent. Open it to use it; what you do there can reach the agent." => {
            "Una página interactiva del agente. Ábrela para usarla; el agente puede recibir tus acciones."
        }
        "The page wants to open a link" => "La página quiere abrir un enlace",
        "Open in the browser card" => "Abrir en la tarjeta del navegador",
        "Sending the page's message to the agent…" => {
            "Enviando al agente el mensaje de la página…"
        }
        "Runs isolated from your files and the app · messages from the page go to the agent" => {
            "Se ejecuta sin acceso a tus archivos ni a la app · el agente recibe los mensajes de la página"
        }
        "waiting for your OK" => "esperando tu confirmación",
        "not allowed" => "sin permiso",
        "paused" => "en pausa",
        "failed" => "falló",
        "reading…" => "leyendo…",
        "waiting for data" => "esperando datos",
        "updated {age}" => "actualizado {age}",
        "Live · {state}" => "En vivo · {state}",
        "{count} s ago" => "hace {count} s",
        "{count} min ago" => "hace {count} min",
        "{count} h ago" => "hace {count} h",
        "From {source}" => "Fuente: {source}",
        "Allow this UI to run a command?" => "¿Permitir que esta interfaz ejecute un comando?",
        "every {seconds} s in {path}, while this card is on screen" => {
            "cada {seconds} s en {path}, mientras esta tarjeta esté en pantalla"
        }
        "the workspace" => "el espacio de trabajo",
        "Deny" => "Denegar",
        "Always for this session" => "Durante toda esta sesión",
        "Allow once" => "Permitir una vez",
        "Not running {command}" => "No se está ejecutando {command}",
        "Ask again" => "Volver a preguntar",
        "the session's background jobs" => "las tareas en segundo plano de la sesión",
        "the session's subagents" => "los subagentes de la sesión",
        "the agent's todo list" => "la lista de tareas del agente",
        "{command}, every {seconds} s" => "{command}, cada {seconds} s",
        "a command" => "un comando",
        "The agent's todo list, live · read-only · Ask about an item from its row" => {
            "Lista de tareas del agente en vivo · solo lectura · pregunta por un elemento desde su fila"
        }
        "Live items · read-only" => "Elementos en vivo · solo lectura",
        "Skip or reorder steps, then Run (all) or Run next · the agent reports each step" => {
            "Omite o reordena pasos y elige Ejecutar (todos) o Ejecutar el siguiente · el agente informa de cada paso"
        }
        "Drag a card into a column marked with a bot: the agent starts a subagent on it" => {
            "Arrastra una tarjeta a una columna con un bot para que el agente inicie un subagente"
        }
        "Turn off Run plan" => "Desactivar la ejecución del plan",
        "Run plan: drive the agent through these steps" => {
            "Ejecutar plan: guía al agente por estos pasos"
        }
        "New sub-item, Enter to add" => "Nuevo subelemento; pulsa Enter para añadirlo",
        "New item, Enter to add" => "Nuevo elemento; pulsa Enter para añadirlo",
        "Nothing pending to run" => "No hay nada pendiente de ejecutar",
        "Nothing here yet." => "Todavía no hay nada aquí.",
        "Every step is through" => "Todos los pasos terminaron",
        "Running step {number} · {item}" => "Ejecutando el paso {number} · {item}",
        "Next: step {number} · {item}" => "Siguiente: paso {number} · {item}",
        "Run" => "Ejecutar",
        "Run next" => "Ejecutar el siguiente",
        "Don't skip" => "No omitir",
        "Skip this step" => "Omitir este paso",
        "Add a sub-item" => "Añadir un subelemento",
        "Card title, Enter to add" => "Título de la tarjeta; pulsa Enter para añadirla",
        "A card dropped here: the agent starts a subagent on it" => {
            "Al soltar una tarjeta aquí, el agente inicia un subagente"
        }
        "not started" => "sin iniciar",
        "Edit card" => "Editar tarjeta",
        "Stop subagent" => "Detener subagente",
        "Comment for the agent, Enter to keep" => {
            "Comentario para el agente; pulsa Enter para guardarlo"
        }
        "Accept all" => "Aceptar todo",
        "Reject all" => "Rechazar todo",
        "Accept" => "Aceptar",
        "Reject" => "Rechazar",
        "Comment" => "Comentar",
        "Applied" => "Aplicado",
        "Accepted" => "Aceptado",
        "Rejected" => "Rechazado",
        "Undecided" => "Sin decidir",
        "Tech (e.g. Postgres)" => "Tecnología (p. ej., Postgres)",
        "sync call" => "llamada síncrona",
        "data flow" => "flujo de datos",
        "dependency" => "dependencia",
        "sync" => "síncrona",
        "async" => "asíncrona",
        "return" => "retorno",
        "actor" => "actor",
        "service" => "servicio",
        "db" => "BD",
        "database" => "base de datos",
        "queue" => "cola",
        "cache" => "caché",
        "client" => "cliente",
        "external" => "externo",
        "storage" => "almacenamiento",
        "function" => "función",
        "note" => "nota",
        "planned" => "planificado",
        "existing" => "existente",
        "deprecated" => "obsoleto",
        "risk" => "riesgo",
        "bar" => "barras",
        "line" => "líneas",
        "area" => "área",
        "pie" => "circular",
        "donut" => "anillo",
        "scatter" => "dispersión",
        "initial" => "inicial",
        "normal" => "normal",
        "final" => "final",
        "choice" => "decisión",
        "step {number}" => "paso {number}",
        "no columns" => "sin columnas",
        "no type" => "sin tipo",
        "The machine is in this state now" => "Este es el estado actual",
        "no event" => "sin evento",
        "After {tasks}" => "Después de {tasks}",
        "Starts before “{task}” ends" => "Empieza antes de que termine «{task}»",
        "On the critical path" => "En la ruta crítica",
        "{percent}% done" => "{percent} % completado",
        "{task} waits for {dependency}" => "{task} espera a {dependency}",
        "{count} field" => "{count} campo",
        "{count} fields" => "{count} campos",
        "1 required" => "1 obligatorio",
        "{count} required" => "{count} obligatorios",
        "{cards} in {columns}" => "{cards} en {columns}",
        "{count} card" => "{count} tarjeta",
        "{count} cards" => "{count} tarjetas",
        "{count} hunk" => "{count} bloque",
        "{count} hunks" => "{count} bloques",
        "{decided} of {hunks} decided" => "Decisiones: {decided} de {hunks}",
        "{count} file" => "{count} archivo",
        "{count} files" => "{count} archivos",
        "{count} participant" => "{count} participante",
        "{count} participants" => "{count} participantes",
        "{count} message" => "{count} mensaje",
        "{count} messages" => "{count} mensajes",
        "{count} table" => "{count} tabla",
        "{count} tables" => "{count} tablas",
        "{count} relation" => "{count} relación",
        "{count} relations" => "{count} relaciones",
        "{count} state" => "{count} estado",
        "{count} states" => "{count} estados",
        "{count} transition" => "{count} transición",
        "{count} transitions" => "{count} transiciones",
        "{count} task" => "{count} tarea",
        "{count} tasks" => "{count} tareas",
        "{month} {day}" => "{day} {month}",
        "Jan" => "ene",
        "Feb" => "feb",
        "Mar" => "mar",
        "Apr" => "abr",
        "May" => "may",
        "Jun" => "jun",
        "Jul" => "jul",
        "Aug" => "ago",
        "Sep" => "sep",
        "Oct" => "oct",
        "Nov" => "nov",
        "Dec" => "dic",
        "milestone" => "hito",
        "1 day" => "1 día",
        "{count} day" => "{count} día",
        "{count} days" => "{count} días",
        "not a git repository" => "no es un repositorio de Git",
        "git log failed" => "falló git log",
        "no command" => "no hay comando",
        "handed over, not read" => "datos recibidos, sin lectura directa",
        "{path} is not a folder" => "{path} no es una carpeta",
        "couldn't start: {error}" => "no se pudo iniciar: {error}",
        "its output is over {limit} KiB" => "la salida supera los {limit} KiB",
        "stopped after {seconds} s" => "se detuvo tras {seconds} s",
        "no workspace folder to read from" => "no hay carpeta del espacio de trabajo para leer",
        "{label} must be text" => "{label} debe ser texto",
        "{label} needs at least {value} characters" => {
            "{label} necesita al menos {value} caracteres"
        }
        "{label} takes at most {value} characters" => {
            "{label} admite como máximo {value} caracteres"
        }
        "{label} must be a number" => "{label} debe ser un número",
        "{label} must be at least {value}" => "{label} debe ser como mínimo {value}",
        "{label} must be at most {value}" => "{label} debe ser como máximo {value}",
        "{label} must be true or false" => "{label} debe ser verdadero o falso",
        "{label} must be one of its options" => "Elige una de las opciones de {label}",
        "“{value}” is not an option of {label}" => "{label} no admite la opción «{value}»",
        "{label} must be a list of options" => "{label} debe ser una lista de opciones",
        "{label}: choose at least {value}" => "{label}: elige al menos {value}",
        "{label}: choose at most {value}" => "{label}: elige como máximo {value}",
        "{label} must be a date (YYYY-MM-DD)" => "{label} debe ser una fecha (YYYY-MM-DD)",
        "{label} must be on or after {value}" => "{label} debe ser el {value} o después",
        "{label} must be on or before {value}" => "{label} debe ser el {value} o antes",
        "{label} must be checked" => "Marca {label}",
        "{label} is required" => "{label} es obligatorio",
        "Today" => "Hoy",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};

    #[test]
    fn genui_and_canvas_text_translates_without_changing_english() {
        for (en, es) in [
            ("Filter rows…", "Filtrar filas…"),
            ("Clear canvas", "Limpiar lienzo"),
        ] {
            assert_eq!(tr(Language::Spanish, en), es);
            assert_eq!(tr(Language::English, en), en);
        }

        use crate::genui::form::Form;
        use serde_json::json;
        let form =
            Form::parse(&json!({"fields": [{"id": "name", "label": "{value}", "required": true}]}))
                .unwrap();
        let answers = Default::default();
        let errors = form.validate_answers(&answers).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "name");
        assert_eq!(
            errors[0].1.render(Language::Spanish),
            "{value} es obligatorio"
        );
        assert_eq!(errors[0].1.render(Language::English), "{value} is required");
        assert!(form.values.is_empty());
    }
}
