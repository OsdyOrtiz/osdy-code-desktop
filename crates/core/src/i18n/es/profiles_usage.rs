//! Spanish text of the profiles usage area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Codex account picker.
        "Codex" => "Codex",
        "View usage" => "Ver uso",
        "Accounts" => "Cuentas",
        "Usage unavailable" => "Uso no disponible",
        "Loading usage…" => "Cargando uso…",
        "Codex accounts" => "Cuentas de Codex",
        "Active" => "Activa",
        "No saved Codex accounts." => "No hay cuentas de Codex guardadas.",
        "Manage accounts…" => "Administrar cuentas…",
        "This Codex account name cannot be selected here. Use Manage accounts…" => {
            "No se puede seleccionar este nombre de cuenta de Codex aquí. Usa Administrar cuentas…"
        }
        "Could not run the Codex account command in this tab." => {
            "No se pudo ejecutar el comando de cuentas de Codex en esta pestaña."
        }
        // Profiles.
        "Loading profiles…" => "Cargando perfiles…",
        "Retry" => "Reintentar",
        "No profiles left. Create one with New." => "No quedan perfiles. Crea uno con Nuevo.",
        "Agent-model routing snapshots, like /gentle:profiles." => {
            "Configuraciones de modelos para agentes, como en /gentle:profiles."
        }
        "Re-read profiles, agents and pins" => "Volver a leer perfiles, agentes y fijaciones",
        "profile-name" => "nombre-del-perfil",
        "Search models or providers…" => "Buscar modelos o proveedores…",
        "Stop using in this repo" => "Dejar de usar en este repositorio",
        "Use in this repo" => "Usar en este repositorio",
        "Clone-local pin in the Git common dir; subagent launches here resolve it" => {
            "Fijación local del clon en el directorio común de Git; se aplica a los subagentes que se inicien aquí"
        }
        "Unpublish from repo" => "Retirar del repositorio",
        "Publish to repo" => "Publicar en el repositorio",
        "Toggle the committed .pi/gentle-ai/profile.json declaration" => {
            "Activar o quitar la declaración versionada en .pi/gentle-ai/profile.json"
        }
        "Clear session binding" => "Desvincular perfil de la sesión",
        "{count} unsaved" => "{count} sin guardar",
        "Save" => "Guardar",
        "Make default" => "Establecer como predeterminado",
        "Default for new sessions: writes models.json, agent routing and settings.json" => {
            "Predeterminado para nuevas sesiones: actualiza models.json, la asignación de modelos a agentes y settings.json"
        }
        "Close" => "Cerrar",
        "Use in this session" => "Usar en esta sesión",
        "Switch this session's orchestrator to the profile (saves edits first)" => {
            "Aplicar el perfil al orquestador de esta sesión (primero guarda los cambios)"
        }
        "leave untouched" => "no cambiar",
        "inherit" => "heredar",
        "Pi persisted default model" => "Modelo predeterminado guardado en Pi",
        "Pi default" => "Predeterminado de Pi",
        "mixed" => "varios valores",
        "follows model" => "según el modelo",
        "Pi persisted default effort" => "Esfuerzo predeterminado guardado en Pi",
        "Inherit effort" => "Heredar el esfuerzo",
        "{tier} (n/a)" => "{tier} (no aplica)",
        "Fast" => "Rápido",
        "Standard" => "Estándar",
        "OpenAI Fast (service_tier priority): 2.5x subscription usage or 2x purchased credits" => {
            "OpenAI Fast (service_tier priority): consumo de suscripción ×2,5 o de créditos comprados ×2"
        }
        "\"{name}\" has {count} unsaved edit(s)" => "\"{name}\": cambios sin guardar ({count})",
        "Save them before leaving this profile, or discard them." => {
            "Guarda los cambios antes de salir del perfil o descártalos."
        }
        "Discard" => "Descartar",
        "Save and continue" => "Guardar y continuar",
        "Delete profile?" => "¿Eliminar el perfil?",
        "Delete profile \"{name}\" from profiles.json? The routing in {models} is not changed." => {
            "¿Eliminar el perfil \"{name}\" de profiles.json? La asignación de modelos en {models} no cambia."
        }
        "Delete" => "Eliminar",
        "Pin this clone" => "Fijar en este clon",
        "Apply" => "Aplicar",
        "Replace profile?" => "¿Reemplazar el perfil?",
        "Profile \"{name}\" already exists. Replace its routing with the import?" => {
            "El perfil \"{name}\" ya existe. ¿Reemplazar su asignación de modelos con la importada?"
        }
        "Replace" => "Reemplazar",
        "orchestrator" => "orquestador",
        "the orchestrator" => "el orquestador",
        "every agent" => "todos los agentes",
        "sets all rows" => "se aplica a todas las filas",
        "review role" => "rol de revisión",
        "Set" => "Establecer",
        "Organic" => "Orgánico",
        "The orchestrator may implement small changes itself" => {
            "El orquestador puede implementar cambios pequeños"
        }
        "Full delegation" => "Delegación total",
        "The orchestrator never implements: every change goes to a bounded writer" => {
            "El orquestador no implementa cambios: cada cambio se delega a un agente de implementación con alcance limitado"
        }
        "Orchestrator" => "Orquestador",
        "No extra authoring rules" => "Sin reglas de implementación adicionales",
        "Adds rules against unrequested machinery: to the orchestrator in Organic mode, and to writer and verify agents" => {
            "Agrega reglas para evitar complejidad no solicitada al orquestador en modo Orgánico y a los agentes de implementación y verificación"
        }
        "Controlled Engineering" => "Ingeniería controlada",
        "Used in this session" => "En uso en esta sesión",
        "Default for new sessions" => "Predeterminado para nuevas sesiones",
        "Used in this repo" => "En uso en este repositorio",
        "Global default: {active}" => "Predeterminado global: {active}",
        "No global default profile" => "Sin perfil predeterminado global",
        "Repo pin: none (not inside a Git worktree)" => {
            "Fijación del repositorio: ninguna (fuera de un árbol de trabajo de Git)"
        }
        "invalid file" => "archivo no válido",
        "Repo pin: local {local} · declaration {repo}" => {
            "Fijación del repositorio: local {local} · declaración {repo}"
        }
        "Not in use. Pick a button below to use it." => {
            "No está en uso. Para usarlo, elige una opción abajo."
        }
        "Routes" => "Asignaciones",
        "Route" => "Asignación",
        "Model" => "Modelo",
        "Effort" => "Esfuerzo",
        "Tier" => "Nivel de servicio",
        "All Fast" => "Todos en modo rápido",
        "Set every compatible OpenAI route to Fast (draft only)" => {
            "Activa el modo rápido en todas las asignaciones compatibles de OpenAI (solo en el borrador)"
        }
        "Pi default orchestrator now: {model}. Fast applies only to openai / openai-codex GPT-5.6 / 6 / 6.1 models; the selector is a request, not a billing guarantee." => {
            "Orquestador predeterminado actual de Pi: {model}. El modo rápido solo se aplica a los modelos GPT-5.6 / 6 / 6.1 de openai / openai-codex. La opción solicita el nivel de servicio, pero no garantiza la facturación."
        }
        "not set" => "sin configurar",
        "Import a profile…" => "Importar un perfil…",
        "Export this profile…" => "Exportar este perfil…",
        "{count} role" => "{count} rol",
        "{count} roles" => "{count} roles",
        "this session" => "esta sesión",
        "this repo" => "este repositorio",
        "declared" => "declarado",
        "New empty profile" => "Nuevo perfil vacío",
        "Duplicate as (empty = {name}-copy)" => "Duplicar como (vacío = {name}-copy)",
        "Rename \"{name}\" to" => "Cambiar el nombre de \"{name}\" a",
        "New" => "Nuevo",
        "Duplicate" => "Duplicar",
        "Rename" => "Cambiar nombre",
        "Custom model id…" => "ID de modelo personalizado…",
        "No model matches. Type a custom id…" => {
            "Ningún modelo coincide. Escribe un ID personalizado…"
        }
        "Not saved: {error}" => "No se guardó: {error}",
        "profiles.json was not written: {error}" => "No se pudo escribir profiles.json: {error}",
        "Nothing to save." => "No hay nada que guardar.",
        "Saved \"{name}\"." => "Se guardó \"{name}\".",
        "Using \"{name}\" in this session." => "Se usa \"{name}\" en esta sesión.",
        "Switching this session to {model}." => "Cambiando esta sesión a {model}.",
        "The orchestrator is left untouched." => "El orquestador no cambia.",
        "Applying \"{name}\" in Gentle Shell…" => "Aplicando \"{name}\" en Gentle Shell…",
        "\"{name}\" was not applied: {error}" => "No se aplicó \"{name}\": {error}",
        "\"{name}\" is not confirmed: {error}" => {
            "No se confirmó la aplicación de \"{name}\": {error}"
        }
        "Applied \"{name}\"." => "Se aplicó \"{name}\".",
        "Gentle Shell did not apply \"{name}\"." => "Gentle Shell no aplicó \"{name}\".",
        "Removed the clone pin. This clone falls back to the repository declaration, then to the global routing." => {
            "Se quitó la fijación del clon. Ahora se usa la declaración del repositorio o, si no existe, la asignación global de modelos."
        }
        "Pinned \"{name}\" for this clone. Subagent launches here resolve it; the orchestrator and other repositories keep their global routing." => {
            "Se fijó \"{name}\" para este clon. Se aplica a los subagentes que se inicien aquí; el orquestador y los demás repositorios conservan la asignación global."
        }
        "This folder is not inside a Git worktree, so nothing was pinned." => {
            "Esta carpeta no está en un árbol de trabajo de Git. No se fijó ningún perfil."
        }
        "This folder is not inside a Git worktree, so nothing was published." => {
            "Esta carpeta no está en un árbol de trabajo de Git. No se publicó ningún perfil."
        }
        "Removed the local pin from {path}. This clone falls back to the repository declaration, then to the global routing." => {
            "Se quitó la fijación local de {path}. Ahora se usa la declaración del repositorio o, si no existe, la asignación global de modelos."
        }
        "Pinned \"{name}\" for this clone in {path}. Subagent launches here resolve it; the orchestrator and other repositories keep their global routing." => {
            "Se fijó \"{name}\" para este clon en {path}. Se aplica a los subagentes que se inicien aquí; el orquestador y los demás repositorios conservan la asignación global."
        }
        "The repository declaration ({path}) already said that; nothing changed." => {
            "La declaración del repositorio ({path}) ya tenía esa configuración. No hubo cambios."
        }
        "Removed the repository declaration ({path}). This worktree falls back to the global routing; a clone pin still outranks it." => {
            "Se quitó la declaración del repositorio ({path}). Este árbol de trabajo vuelve a la asignación global de modelos, salvo que haya una fijación del clon."
        }
        "Declared \"{name}\" for this worktree in {path}. Subagent launches resolve it; commit the file to share the routing. A clone pin takes precedence over it." => {
            "Se declaró \"{name}\" para este árbol de trabajo en {path}. Se aplica al iniciar subagentes. Incluye el archivo en un commit para compartir la asignación. La fijación del clon tiene prioridad."
        }
        "Sent /gentle:profiles publish {name}. Refresh to see the declaration." => {
            "Se envió /gentle:profiles publish {name}. Actualiza para ver la declaración."
        }
        "Clearing this session's profile binding…" => "Desvinculando el perfil de esta sesión…",
        "Sent /gentle:profiles clear." => "Se envió /gentle:profiles clear.",
        "Cleared the session binding. This session is back on \"{profile}\"." => {
            "Se desvinculó el perfil de la sesión. Esta sesión vuelve a usar \"{profile}\"."
        }
        "Cleared the session binding. This session is back on the routing without a session profile." => {
            "Se desvinculó el perfil de la sesión. Se vuelve a usar la asignación de modelos sin un perfil de sesión."
        }
        "This session had no profile binding to clear." => {
            "Esta sesión no tenía ningún perfil vinculado."
        }
        "Gentle Shell could not confirm the clear; run /gentle:profiles in the terminal to see why." => {
            "Gentle Shell no pudo confirmar la desvinculación. Ejecuta /gentle:profiles en la terminal para ver por qué."
        }
        "The session binding was not cleared: {error}" => {
            "No se desvinculó el perfil de la sesión: {error}"
        }
        "The answer to the clear did not match it." => {
            "La respuesta no coincidió con la solicitud de desvinculación."
        }
        "Showing \"{name}\"." => "Mostrando \"{name}\".",
        "No profile created: no name entered." => {
            "No se creó el perfil porque no ingresaste un nombre."
        }
        "Profile \"{name}\" unchanged: no new name entered." => {
            "El perfil \"{name}\" no cambió porque no ingresaste un nombre nuevo."
        }
        "Profile \"{from}\" renamed to \"{name}\"." => {
            "Se cambió el nombre del perfil \"{from}\" a \"{name}\"."
        }
        "Updated the clone pin {path}." => "Se actualizó la fijación del clon en {path}.",
        "The committed declaration {path} still names \"{from}\"; Publish to repo republishes it." => {
            "La declaración versionada en {path} todavía indica \"{from}\". Usa Publicar en el repositorio para actualizarla."
        }
        "Profile \"{name}\" created." => "Se creó el perfil \"{name}\".",
        "Profile \"{name}\" duplicated from \"{from}\"." => {
            "Se creó el perfil \"{name}\" como copia de \"{from}\"."
        }
        "Profile {name} is active. Apply another profile before deleting it." => {
            "El perfil {name} está activo. Aplica otro perfil antes de eliminarlo."
        }
        "local pin" => "fijación local",
        "repo pin" => "fijación del repositorio",
        "{pin} at {path}" => "{pin} en {path}",
        " and " => " y ",
        "Profile \"{name}\" is pinned for this repository ({layers}). Remove the pin first (Use in this repo for the clone pin, Publish to repo for the declaration), then delete it." => {
            "El perfil \"{name}\" está fijado en este repositorio ({layers}). Quita la fijación antes de eliminarlo: usa Dejar de usar en este repositorio para el clon o Retirar del repositorio para la declaración."
        }
        "Profile not deleted: {error}" => "No se eliminó el perfil: {error}",
        "Deleted \"{name}\"." => "Se eliminó \"{name}\".",
        "Profile not exported: {error}" => "No se exportó el perfil: {error}",
        "Exported \"{name}\" to {path}." => "Se exportó \"{name}\" a {path}.",
        "Import" => "Importar",
        "{path} is not a profile export (gentle-pi.agent_model_profile)." => {
            "{path} no es un perfil exportado (gentle-pi.agent_model_profile)."
        }
        "Profile not imported: {error}" => "No se importó el perfil: {error}",
        "Imported \"{name}\" ({count} route)." => "Se importó \"{name}\" ({count} asignación).",
        "Imported \"{name}\" ({count} routes)." => "Se importó \"{name}\" ({count} asignaciones).",
        "Left out invalid entries: {entries}." => {
            "Se omitieron las entradas no válidas: {entries}."
        }
        "Updated the clone pin." => "Se actualizó la fijación del clon.",
        "The profile fallbacks were not updated: {error}" => {
            "No se actualizaron los perfiles alternativos: {error}"
        }
        "Saved. Showing \"{name}\"." => "Cambios guardados. Mostrando \"{name}\".",
        "Gentle Shell has not reported yet whether it serves its configuration to Desktop. Retry once the tab has finished starting." => {
            "Gentle Shell aún no ha indicado si proporciona su configuración a Desktop. Vuelve a intentarlo cuando termine de iniciarse la pestaña."
        }
        "Gentle Desktop's channel failed in this tab, so Profiles cannot tell whether Gentle Shell owns the configuration. Restart the tab's Gentle Shell, then open Profiles again." => {
            "Falló el canal de Gentle Desktop en esta pestaña. Perfiles no puede determinar si Gentle Shell administra la configuración. Reinicia Gentle Shell en la pestaña y vuelve a abrir Perfiles."
        }
        "Cannot open agent profiles: {error}" => {
            "No se pueden abrir los perfiles de agente: {error}"
        }
        "The {store} store could not be re-read: {error}" => {
            "No se pudo volver a leer el almacén {store}: {error}"
        }
        "Not saved: the {store} store changed in Gentle Shell since Profiles read it. Nothing was overwritten; the latest is being read now, and your unsaved edits stay so you can review them and save again." => {
            "No se guardó: el almacén {store} cambió en Gentle Shell desde que Perfiles lo leyó. No se sobrescribió nada. Se está leyendo la última versión y se conservan tus cambios para que puedas revisarlos y volver a guardar."
        }
        "Could not create the profiles store: Gentle Shell is applying a profile right now. Open Profiles again once it finishes." => {
            "No se pudo crear el almacén de perfiles: Gentle Shell está aplicando un perfil. Vuelve a abrir Perfiles cuando termine."
        }
        "Not saved: Gentle Shell is applying a profile right now, so the {store} store was not written. Your unsaved edits stay; save again once the apply finishes." => {
            "No se guardó el almacén {store} porque Gentle Shell está aplicando un perfil. Se conservan tus cambios; vuelve a guardar cuando termine."
        }
        "Could not create the profiles store: {error}" => {
            "No se pudo crear el almacén de perfiles: {error}"
        }
        "{error} Profiles edits the files directly from here on; a change Gentle Shell had not confirmed was not saved, so check it and save again." => {
            "{error} A partir de ahora, Perfiles editará los archivos directamente. Los cambios que Gentle Shell no confirmó no se guardaron; revísalos y vuelve a guardar."
        }
        "Seeded the \"current\" profile in the profiles store from the routing currently in effect." => {
            "Se creó el perfil \"current\" en el almacén de perfiles con la asignación de modelos vigente."
        }
        "Could not create the profiles store; Gentle Shell did not take the request." => {
            "No se pudo crear el almacén de perfiles: Gentle Shell no aceptó la solicitud."
        }
        "This Gentle Shell does not serve its configuration to Desktop." => {
            "Esta versión de Gentle Shell no comparte su configuración con Desktop."
        }
        "Gentle Shell is still starting in this tab; try again in a moment." => {
            "Gentle Shell aún se está iniciando en esta pestaña. Inténtalo de nuevo en un momento."
        }
        "Gentle Shell did not answer in time; nothing is confirmed." => {
            "Gentle Shell no respondió a tiempo. No se confirmó ningún cambio."
        }
        "Gentle Shell restarted before it answered; nothing is confirmed." => {
            "Gentle Shell se reinició antes de responder. No se confirmó ningún cambio."
        }
        "The configuration changed in Gentle Shell since it was read." => {
            "La configuración cambió en Gentle Shell desde que se leyó."
        }
        "Gentle Shell refused the data ({message})." => {
            "Gentle Shell rechazó los datos ({message})."
        }
        "Gentle Shell could not write it ({message})." => {
            "Gentle Shell no pudo guardarlo ({message})."
        }
        "Gentle Shell's configuration is unavailable ({message})." => {
            "La configuración de Gentle Shell no está disponible ({message})."
        }
        "Gentle Shell is applying a profile right now, so nothing was written." => {
            "Gentle Shell está aplicando un perfil, así que no se guardó nada."
        }
        "Gentle Shell refused the request ({code}: {message})." => {
            "Gentle Shell rechazó la solicitud ({code}: {message})."
        }
        "The configuration request failed ({reason})." => {
            "Falló la solicitud de configuración ({reason})."
        }
        "Make default is still writing the files; wait for Make default to finish, then save again." => {
            "Establecer como predeterminado aún está escribiendo los archivos. Espera a que termine y vuelve a guardar."
        }
        "Gentle Shell is still applying a profile; wait for it to finish, then try again." => {
            "Gentle Shell todavía está aplicando un perfil. Espera a que termine y vuelve a intentarlo."
        }
        "Gentle Shell has no saved profile by that name ({message})." => {
            "Gentle Shell no tiene ningún perfil guardado con ese nombre ({message})."
        }
        "Gentle Shell is already applying a profile; nothing was written. Try again once it finishes." => {
            "Gentle Shell ya está aplicando un perfil. No se guardó nada. Vuelve a intentarlo cuando termine."
        }
        "Gentle Shell's profiles are unavailable in this tab ({message})." => {
            "Los perfiles de Gentle Shell no están disponibles en esta pestaña ({message})."
        }
        "{row}: model {label}. Save keeps it in {selected}." => {
            "{row}: modelo {label}. Guarda para aplicarlo a {selected}."
        }
        "{row}: effort {level}. Save keeps it in {selected}." => {
            "{row}: esfuerzo {level}. Guarda para aplicarlo a {selected}."
        }
        "Standard/Fast is available only for compatible OpenAI models." => {
            "La opción Estándar/Rápido solo está disponible para modelos compatibles de OpenAI."
        }
        "{row}: {tier}. Save keeps it in {selected}.{note}" => {
            "{row}: {tier}. Guarda para aplicarlo a {selected}.{note}"
        }
        " Fast depends on plan/model; subscription usage 2.5x or purchased credits 2x. API pricing differs." => {
            " El modo rápido depende del plan y del modelo: consumo de suscripción ×2,5 o de créditos comprados ×2. La API tiene otros precios."
        }
        "No compatible OpenAI routes in this profile." => {
            "Este perfil no tiene asignaciones compatibles de OpenAI."
        }
        "All Fast: {count} compatible route. Save to keep. Subscription 2.5x; purchased credits 2x. API pricing differs." => {
            "Modo rápido en {count} asignación compatible. Guarda para conservar el cambio. Consumo de suscripción ×2,5 o de créditos comprados ×2. La API tiene otros precios."
        }
        "All Fast: {count} compatible routes. Save to keep. Subscription 2.5x; purchased credits 2x. API pricing differs." => {
            "Modo rápido en {count} asignaciones compatibles. Guarda para conservar los cambios. Consumo de suscripción ×2,5 o de créditos comprados ×2. La API tiene otros precios."
        }
        "No custom model set." => "No se configuró un modelo personalizado.",
        "Custom model id must be one provider/model line using letters, numbers, '.', '-', '_', '~', ':', '@', '/', '+', '%' only." => {
            "El ID del modelo personalizado debe tener el formato provider/model en una sola línea y solo puede contener letras, números y '.', '-', '_', '~', ':', '@', '/', '+', '%'."
        }
        "Cannot open agent profiles because {path} is invalid JSON or not a profiles file. Fix or remove the file, then open Profiles again." => {
            "No se pueden abrir los perfiles de agente: {path} no contiene JSON válido o no es un archivo de perfiles. Corrige o elimina el archivo y vuelve a abrir Perfiles."
        }
        "Could not create {path}: {error}" => "No se pudo crear {path}: {error}",
        "Seeded the \"current\" profile in {path} from the routing currently in effect." => {
            "Se creó el perfil \"current\" en {path} con la asignación de modelos vigente."
        }
        "profiles: {entries}" => "perfiles: {entries}",
        "routing entries: {entries}" => "asignaciones de modelos: {entries}",
        "active marker: {active}" => "perfil marcado como activo: {active}",
        "orchestrator modes: {entries}" => "modos del orquestador: {entries}",
        "Dropped invalid entries while loading {origin} — {entries}." => {
            "Se omitieron entradas no válidas al cargar {origin}: {entries}."
        }
        "Gentle Shell has not sent the profiles store yet." => {
            "Gentle Shell todavía no ha enviado el almacén de perfiles."
        }
        "Cannot open agent profiles because Gentle Shell reports its profiles store as invalid. Fix or remove profiles.json, then open Profiles again." => {
            "No se pueden abrir los perfiles de agente: Gentle Shell indica que el almacén no es válido. Corrige o elimina profiles.json y vuelve a abrir Perfiles."
        }
        "the profiles store" => "el almacén de perfiles",
        "The repository pins could not be read: {error}" => {
            "No se pudieron leer las fijaciones del repositorio: {error}"
        }
        "The profile fallbacks could not be read, so a rename or delete leaves them as they are: {error}" => {
            "No se pudieron leer los perfiles alternativos. Si cambias un nombre o eliminas un perfil, no se actualizarán: {error}"
        }
        "{source} layer invalid" => "capa {source} no válida",
        "unreadable ({error})" => "no se pudo leer ({error})",
        "{count} saved agent route" => "{count} asignación de agente guardada",
        "{count} saved agent routes" => "{count} asignaciones de agentes guardadas",
        "global routing: {text}" => "asignación global de modelos: {text}",
        "subagent settings: {text}" => "configuración de subagentes: {text}",
        "{count} installer-managed file" => "{count} archivo administrado por el instalador",
        "{count} installer-managed files" => "{count} archivos administrados por el instalador",
        "managed assets: {text}" => "recursos administrados: {text}",
        "Shown read-only from Gentle Shell — {details}. Make default rewrites the routing." => {
            "Datos de solo lectura de Gentle Shell: {details}. Establecer como predeterminado sobrescribe la asignación de modelos."
        }
        "Make default is already running; wait for it to finish." => {
            "Establecer como predeterminado ya está en curso. Espera a que termine."
        }
        "Gentle Shell is still applying a profile; wait for it to finish." => {
            "Gentle Shell aún está aplicando un perfil. Espera a que termine."
        }
        "A profile change is still being saved; Make default again once it lands." => {
            "Aún se está guardando un cambio en un perfil. Vuelve a elegir Establecer como predeterminado cuando termine."
        }
        "The profiles store is still being read; Make default again in a moment." => {
            "Se está leyendo el almacén de perfiles. Vuelve a elegir Establecer como predeterminado en un momento."
        }
        "The profiles store does not exist yet; open Profiles again to create it." => {
            "Todavía no existe el almacén de perfiles. Vuelve a abrir Perfiles para crearlo."
        }
        "Invalid profile name: {profile}." => "Nombre de perfil no válido: {profile}.",
        "This repository also has a committed declaration ({path}); the clone pin takes precedence, so Gentle Shell does not report which profile it declares." => {
            "Este repositorio también tiene una declaración versionada ({path}). La fijación del clon tiene prioridad, así que Gentle Shell no informa qué perfil se declara allí."
        }
        "Managed-assets lock path is unsafe and must be a regular file: {path}" => {
            "La ruta del bloqueo de recursos administrados no es segura y debe apuntar a un archivo normal: {path}"
        }
        "Timed out acquiring managed-assets lock file {path}. Verify no installer is active before removing this exact lock file." => {
            "Se agotó el tiempo de espera para obtener el bloqueo de recursos administrados en {path}. Confirma que no haya ningún instalador activo antes de eliminar ese archivo de bloqueo."
        }
        "Managed routing update failed and could not restore {path}: {error}" => {
            "Falló la actualización de la asignación de modelos administrada y no se pudo restaurar {path}: {error}"
        }
        "model id {model} is not a provider/model pair" => {
            "el ID de modelo {model} no tiene el formato provider/model"
        }
        "the file is not a JSON object" => "el archivo no contiene un objeto JSON",
        "Use \"{name}\" for this repository?" => "¿Usar \"{name}\" en este repositorio?",
        "This repository pins \"{profile}\" ({pin} at {path})." => {
            "Este repositorio tiene fijado \"{profile}\" ({pin} en {path})."
        }
        "Make default re-pins this clone instead: no global routing, settings.json or active marker is written." => {
            "Establecer como predeterminado cambia solo la fijación de este clon. No se modifican la asignación global de modelos, settings.json ni el perfil marcado como activo."
        }
        "Apply orchestrator-only profile?" => {
            "¿Aplicar el perfil que solo configura el orquestador?"
        }
        "Profile \"{name}\" has an orchestrator entry but no agent routing entries. Applying it will replace global routing in {path} with the orchestrator entry alone, clear every materialized agent route so every agent returns to inherit its default model, set the configured orchestrator entry in settings.json, and switch this session to that orchestrator model." => {
            "El perfil \"{name}\" configura el orquestador, pero no asigna modelos a los agentes. Si lo aplicas, la asignación global en {path} solo incluirá el orquestador y se quitarán todas las asignaciones escritas para que cada agente vuelva a heredar su modelo predeterminado. También se actualizará el orquestador en settings.json y se usará ese modelo en esta sesión."
        }
        "Apply empty profile?" => "¿Aplicar el perfil vacío?",
        "Profile \"{name}\" has no routing entries. Applying it will replace global routing in {path} with an empty configuration and return every agent to inherit its default model." => {
            "El perfil \"{name}\" no tiene asignaciones de modelos. Si lo aplicas, se vaciará la configuración global en {path} y todos los agentes volverán a heredar su modelo predeterminado."
        }
        ", set the configured orchestrator entry in settings.json, and switch this session to that orchestrator model" => {
            ", actualizará el orquestador en settings.json y usará ese modelo en esta sesión"
        }
        "{role}: {from} → inherit (cleared)" => {
            "{role}: {from} → heredar (se quitó la asignación)"
        }
        "{role}: {to} (added)" => "{role}: {to} (se agregó la asignación)",
        "Applying \"{name}\" replaces global routing in {path} with this profile's routes{effects}." => {
            "Aplicar \"{name}\" reemplazará la asignación global de modelos en {path} por la del perfil{effects}."
        }
        "Its agent routes already match the current global routing." => {
            "Sus asignaciones de modelos a agentes ya coinciden con las globales."
        }
        "Profile \"{name}\" has agent routing entries, but the current global routing in {path} could not be read, so existing routes are not listed. Applying it will replace global routing in {path} with this profile's routes, so every existing agent route may be replaced or cleared back to inherit{effects}." => {
            "El perfil \"{name}\" tiene asignaciones de modelos a agentes, pero no se pudo leer la configuración global en {path}, así que no se muestran las asignaciones actuales. Si lo aplicas, se reemplazará la configuración global de {path} por la del perfil: cada asignación de agente se podrá reemplazar o volver a heredar{effects}."
        }
        "Apply profile \"{name}\"?" => "¿Aplicar el perfil \"{name}\"?",
        "Profile does not exist: {name}." => "El perfil no existe: {name}.",
        "Profile already exists: {name}." => "El perfil ya existe: {name}.",
        "Invalid profile name: {name}. Use 1-64 characters (letters, numbers, '.', '_', '-') starting with a letter or number." => {
            "Nombre de perfil no válido: {name}. Usa entre 1 y 64 caracteres (letras, números, '.', '_', '-') y empieza con una letra o un número."
        }
        "The {pin} file {path} is not a valid pin and is ignored. Fix or remove it." => {
            "El archivo de {pin} {path} no es válido y se ignora. Corrígelo o elimínalo."
        }
        "The {pin} names \"{profile}\", which no profile defines, so it is ignored ({path})." => {
            "Se ignora la {pin} en {path} porque el perfil \"{profile}\" no existe."
        }
        "This repository pins \"{profile}\" ({pin} at {path}), so its subagent launches resolve that profile instead of the global routing." => {
            "Este repositorio tiene fijado \"{profile}\" ({pin} en {path}). Los subagentes que se inicien aquí usarán ese perfil en lugar de la asignación global."
        }
        "Profile key \"{key}\" matches no agent; did you mean {suggestion}?" => {
            "La clave \"{key}\" del perfil no coincide con ningún agente. ¿Quisiste decir {suggestion}?"
        }
        "Profile key \"{key}\" matches no agent configured in gentle-shell, so it is ignored at launch." => {
            "La clave \"{key}\" del perfil no coincide con ningún agente configurado en gentle-shell. Se ignorará al iniciar los agentes."
        }
        "Could not pin profile \"{name}\" in {path}: {error}" => {
            "No se pudo fijar el perfil \"{name}\" en {path}: {error}"
        }
        "Applied \"{name}\" repo-scoped: this clone now pins it in {path}. No global routing or orchestrator was written." => {
            "Se aplicó \"{name}\" solo en este repositorio: el clon ahora lo fija en {path}. No se modificaron la asignación global de modelos ni el orquestador."
        }
        "The committed declaration {path} still declares \"{profile}\"; the clone pin now takes precedence." => {
            "La declaración versionada en {path} todavía indica \"{profile}\". Ahora tiene prioridad la fijación del clon."
        }
        "Could not update {path}: {error}" => "No se pudo actualizar {path}: {error}",
        "routing" => "asignación de modelos",
        "active marker" => "perfil marcado como activo",
        "nothing" => "nada",
        " {path} still holds this profile's routing because no previously active profile was recorded to restore." => {
            " {path} conserva la asignación de modelos de este perfil porque no había un perfil activo anterior que se pudiera restaurar."
        }
        "Could not apply profile \"{name}\": {error}. Restored: {restored}.{unresolved}" => {
            "No se pudo aplicar el perfil \"{name}\": {error}. Se restauró: {restored}.{unresolved}"
        }
        "could not write {path}: {error}" => "no se pudo escribir {path}: {error}",
        "could not materialize the routing: {error}" => {
            "no se pudo escribir la asignación de modelos: {error}"
        }
        "Applied profile \"{name}\" — {count} agent updated. New routing takes effect on the next subagent launch." => {
            "Se aplicó el perfil \"{name}\" y se actualizó {count} agente. La nueva asignación se usa al volver a iniciar el subagente."
        }
        "Applied profile \"{name}\" — {count} agents updated. New routing takes effect on the next subagent launch." => {
            "Se aplicó el perfil \"{name}\" y se actualizaron {count} agentes. La nueva asignación se usa al volver a iniciar los subagentes."
        }
        "could not set the orchestrator: {reason}; {path} was left unchanged" => {
            "no se pudo configurar el orquestador: {reason}; {path} no se modificó"
        }
        "Orchestrator set to {model} in {path}." => {
            "Se configuró el orquestador con {model} en {path}."
        }
        "This repository also has a pin layer that does not resolve; the global routing governs its subagents until the pin is fixed." => {
            "Este repositorio tiene una fijación que no apunta a un perfil válido. Los subagentes usarán la asignación global hasta que corrijas la fijación."
        }
        // Usage.
        "Subscription windows of signed-in providers" => {
            "Períodos de uso de las suscripciones con sesión iniciada"
        }
        "Claude Code · bridge" => "Claude Code · puente",
        "unknown" => "desconocido",
        "limit" => "límite",
        "{count} provider" => "{count} proveedor",
        "{count} providers" => "{count} proveedores",
        "Token and cost totals from local sessions" => {
            "Totales de tokens y costos de las sesiones locales"
        }
        "{count} session files" => "Archivos de sesión: {count}",
        "From Gentle Shell" => "Datos de Gentle Shell",
        "Usage" => "Uso",
        "Subscription" => "Suscripción",
        "Local stats" => "Estadísticas locales",
        "Refresh" => "Actualizar",
        "updated just now" => "actualizado ahora mismo",
        "updated {minutes}m ago" => "actualizado hace {minutes} min",
        "Reading subscription usage…" => "Leyendo el uso de la suscripción…",
        "Could not read subscription usage" => "No se pudo leer el uso de la suscripción",
        "Subscriptions" => "Suscripciones",
        "No subscription provider is signed in. Sign in to ChatGPT (Codex), Claude or NaN from Providers." => {
            "No has iniciado sesión en ningún proveedor de suscripción. Inicia sesión en ChatGPT (Codex), Claude o NaN desde Proveedores."
        }
        "This server has not reported any subscription usage yet." => {
            "Este servidor todavía no ha informado el uso de ninguna suscripción."
        }
        "No subscription usage for: {providers}" => {
            "No hay datos de uso de suscripción para: {providers}"
        }
        "fetching" => "cargando",
        "signed out" => "sin sesión",
        "error" => "error",
        "pending" => "pendiente",
        "stale" => "desactualizado",
        "unavailable" => "no disponible",
        "Fetching usage…" => "Consultando el uso…",
        "No subscription usage is available for this provider." => {
            "No hay datos de uso de suscripción para este proveedor."
        }
        "No subscription usage reported yet." => {
            "Todavía no se ha informado el uso de la suscripción."
        }
        "refresh unsupported" => "no admite actualización",
        "refresh out of scope" => "actualización no aplicable",
        "limit reached" => "límite alcanzado",
        "period" => "período",
        "week" => "semana",
        "Reading session history…" => "Leyendo el historial de sesiones…",
        "Skipped {count} session file larger than 64 MiB." => {
            "Se omitió {count} archivo de sesión de más de 64 MiB."
        }
        "Skipped {count} session files larger than 64 MiB." => {
            "Se omitieron {count} archivos de sesión de más de 64 MiB."
        }
        "{count} session file(s) could not be read." => {
            "Archivos de sesión que no se pudieron leer: {count}."
        }
        "Range" => "Período",
        "All time" => "Todo el historial",
        "30 days" => "30 días",
        "7 days" => "7 días",
        "Tokens" => "Tokens",
        "Cost" => "Costo",
        "Sessions" => "Sesiones",
        "Messages" => "Mensajes",
        "Active days" => "Días activos",
        "input" => "entrada",
        "output" => "salida",
        "cache read" => "lectura de caché",
        "cache write" => "escritura en caché",
        "Last {count} days" => "Últimos {count} días",
        "{day} · {count} tokens" => "{day} · {count} tokens",
        "peak {value}" => "máximo {value}",
        "No usage in this range." => "No hay uso en este período.",
        "Share" => "Porcentaje",
        "Providers" => "Proveedores",
        "Provider" => "Proveedor",
        "Projects" => "Proyectos",
        "Project" => "Proyecto",
        "Only the 50 largest models are listed; the totals count every model." => {
            "Solo se muestran los 50 modelos con más uso; los totales incluyen todos los modelos."
        }
        "Only the 50 largest projects are listed; the totals count every project." => {
            "Solo se muestran los 50 proyectos con más uso; los totales incluyen todos los proyectos."
        }
        "Only the 50 largest providers are listed; the totals count every provider." => {
            "Solo se muestran los 50 proveedores con más uso; los totales incluyen todos los proveedores."
        }
        "Active days count only the 400 most recent days with usage." => {
            "Los días activos solo incluyen los últimos 400 días con uso."
        }
        "This provider reported no rate-limit windows for this plan." => {
            "Este proveedor no informó períodos con límites de uso para este plan."
        }
        "Usage could not be read for this provider." => "No se pudo leer el uso de este proveedor.",
        "API key credential — subscription usage needs a ChatGPT sign-in." => {
            "Credencial de API key: para ver el uso de la suscripción, inicia sesión en ChatGPT."
        }
        "No access token stored — sign in again." => {
            "No hay un token de acceso guardado. Vuelve a iniciar sesión."
        }
        "The ChatGPT token expired — sign in again (a chat turn with this provider also refreshes it)." => {
            "El token de ChatGPT venció. Vuelve a iniciar sesión o envía un mensaje de chat con este proveedor para renovarlo."
        }
        "The token carries no ChatGPT account id — sign in again." => {
            "El token no contiene un ID de cuenta de ChatGPT. Vuelve a iniciar sesión."
        }
        "Claude reports its 5h and weekly windows only in response headers, so they are available while chatting in the terminal UI." => {
            "Claude solo informa sus períodos de 5 horas y semanales en los encabezados de respuesta, así que están disponibles al conversar en la terminal."
        }
        "No NaN API key stored." => "No hay una API key de NaN guardada.",
        "{detail} is not a JSON object." => "{detail} no es un objeto JSON.",
        "Could not read {path}: {error}" => "No se pudo leer {path}: {error}",
        "Network error: {detail}" => "Error de red: {detail}",
        "Could not read the response: {detail}" => "No se pudo leer la respuesta: {detail}",
        "The response was not JSON." => "La respuesta no era JSON.",
        "ChatGPT reported no rate-limit windows for this plan." => {
            "ChatGPT no informó períodos con límites de uso para este plan."
        }
        "ChatGPT rejected the stored token — sign in again." => {
            "ChatGPT rechazó el token guardado. Vuelve a iniciar sesión."
        }
        "Usage request failed (HTTP {detail})." => "Falló la consulta de uso (HTTP {detail}).",
        "The NaN API key could not be resolved (an environment variable it references is unset, or its command failed)." => {
            "No se pudo obtener la API key de NaN: una variable de entorno que usa no está definida o su comando falló."
        }
        "The quota response carried no metered models." => {
            "La respuesta de cuota no incluyó modelos con consumo medido."
        }
        "NaN rejected the API key — set it again in Providers." => {
            "NaN rechazó la API key. Vuelve a configurarla en Proveedores."
        }
        "Refused a redirect from the quota endpoint (HTTP {detail})." => {
            "Se rechazó una redirección al consultar la cuota (HTTP {detail})."
        }
        "Quota request failed (HTTP {detail})." => "Falló la consulta de cuota (HTTP {detail}).",
        "resets now" => "se restablece ahora",
        "resets in {minutes}m" => "se restablece en {minutes} min",
        "resets in {hours}h {minutes}m" => "se restablece en {hours} h {minutes} min",
        "resets in {days}d {hours}h" => "se restablece en {days} d {hours} h",
        "Weekly · all models" => "Semanal · todos los modelos",
        "5-hour limit" => "Límite de 5 horas",
        "resets at {time}" => "se restablece a las {time}",
        "resets {weekday} {time}" => "se restablece el {weekday} a las {time}",
        "Sun" => "dom",
        "Mon" => "lun",
        "Tue" => "mar",
        "Wed" => "mié",
        "Thu" => "jue",
        "Fri" => "vie",
        "Sat" => "sáb",
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
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, tr};
    use crate::profiles::{
        PinRead, PinStatus, ProfilesFile, RouteEntry, Routing, create_profile, evaluate_pin,
        format_selection, pin_notes, unmatched_notes,
    };
    use crate::usage::subscription::{
        UsageWindow, format_reset_in, reset_clock_label_in, window_display_label_in,
    };
    use crate::usage::time::day_label_in;

    #[test]
    fn profiles_usage_translates_spanish_and_preserves_english() {
        for (en, es) in [
            ("Loading profiles…", "Cargando perfiles…"),
            (
                "Reading session history…",
                "Leyendo el historial de sesiones…",
            ),
        ] {
            assert_eq!(tr(Language::English, en), en);
            assert_eq!(tr(Language::Spanish, en), es);
        }

        let file = ProfilesFile::default();
        let invalid_name = "bad/name";
        let error = file.create(invalid_name, Routing::default()).unwrap_err();
        assert_eq!(
            error.message(Language::English),
            create_profile(&file, invalid_name, Routing::default()).unwrap_err()
        );
        assert_eq!(
            error.message(Language::Spanish),
            "Nombre de perfil no válido: \"bad/name\". Usa entre 1 y 64 caracteres (letras, números, '.', '_', '-') y empieza con una letra o un número."
        );
        assert!(!file.has(invalid_name));

        let entry = RouteEntry {
            model: Some("openai/example".into()),
            thinking: None,
            tier: Some("fast".into()),
        };
        assert_eq!(
            RouteEntry::format_selection_in(Some(&entry), Language::English),
            format_selection(Some(&entry))
        );
        assert_eq!(
            RouteEntry::format_selection_in(Some(&entry), Language::Spanish),
            "openai/example · Rápido"
        );
        let routes = Routing(vec![("worker".into(), entry)]);
        assert_eq!(
            routes.unmatched_notes_in(&["pkg.worker".into()], Language::English),
            unmatched_notes(&routes, &["pkg.worker".into()])
        );
        assert_eq!(
            routes.unmatched_notes_in(&["pkg.worker".into()], Language::Spanish),
            [
                "La clave \"worker\" del perfil no coincide con ningún agente. ¿Quisiste decir pkg.worker?"
            ]
        );
        let pin = PinStatus {
            local_path: "clone-pin.json".into(),
            repo_path: "profile.json".into(),
            local: PinRead::Invalid,
            repo: PinRead::Missing,
        };
        let evaluation = evaluate_pin(Some(&pin), &file);
        assert_eq!(
            evaluation.notes_in(Language::English),
            pin_notes(Some(&pin), &file)
        );
        assert_eq!(
            evaluation.notes_in(Language::Spanish),
            [
                "El archivo de fijación local clone-pin.json no es válido y se ignora. Corrígelo o elimínalo."
            ]
        );

        let reset = Some(60_000);
        assert_eq!(format_reset_in(Language::English, reset, 0), "resets in 1m");
        assert_eq!(
            format_reset_in(Language::Spanish, reset, 0),
            "se restablece en 1 min"
        );
        assert_eq!(
            reset_clock_label_in(Language::English, Some(86_400_000), 0, 0),
            "resets Fri 12:00 a.m."
        );
        assert_eq!(
            reset_clock_label_in(Language::Spanish, Some(86_400_000), 0, 0),
            "se restablece el vie a las 12:00 a.m."
        );
        assert_eq!(day_label_in(Language::English, 0), "Jan 1");
        assert_eq!(day_label_in(Language::Spanish, 0), "1 ene");
        let window = UsageWindow {
            label: "week".into(),
            used_percent: 0.0,
            window_seconds: 604_800.0,
            reset_at: None,
            used: None,
            budget: None,
        };
        assert_eq!(
            window_display_label_in(Language::English, &window),
            "Weekly · all models"
        );
        assert_eq!(
            window_display_label_in(Language::Spanish, &window),
            "Semanal · todos los modelos"
        );
    }
}
