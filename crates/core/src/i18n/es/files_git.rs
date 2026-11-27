//! Spanish text of Files, Explorer, Changes, Branches and Git History.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Files and Explorer.
        "A name is required" => "Escribe un nombre",
        "an entry with that name already exists" => "ya existe un elemento con ese nombre",
        "Can't paste {path}" => "No se puede pegar {path}",
        "Can't paste “{name}” into itself" => "No se puede pegar «{name}» dentro de sí mismo",
        "Can't read this folder: {error}" => "No se puede leer esta carpeta: {error}",
        "Change folder" => "Cambiar carpeta",
        "Close" => "Cerrar",
        "close" => "cerrar",
        "Collapse All" => "Contraer todo",
        "Copy" => "Copiar",
        "Copy Path" => "Copiar ruta",
        "Copy Relative Path" => "Copiar ruta relativa",
        "Couldn't create “{name}”: {error}" => "No se pudo crear «{name}»: {error}",
        "Couldn't duplicate “{name}”: {error}" => "No se pudo duplicar «{name}»: {error}",
        "Couldn't move “{name}” to the Trash: {error}" => {
            "No se pudo enviar «{name}» a la papelera: {error}"
        }
        "Couldn't open {name}: {error}" => "No se pudo abrir {name}: {error}",
        "Couldn't paste “{name}”: {error}" => "No se pudo pegar «{name}»: {error}",
        "Couldn't rename “{name}”: {error}" => {
            "No se pudo cambiar el nombre de «{name}»: {error}"
        }
        "Cut" => "Cortar",
        "Duplicate" => "Duplicar",
        "Empty folder" => "Carpeta vacía",
        "enter" => "entrar",
        "File name" => "Nombre del archivo",
        "Filter…" => "Filtrar…",
        "Folder name" => "Nombre de la carpeta",
        "home" => "inicio",
        "Insert Path" => "Insertar ruta",
        "Loading…" => "Cargando…",
        "More" => "Más",
        "Move to Trash" => "Enviar a la papelera",
        "New File" => "Nuevo archivo",
        "New File…" => "Nuevo archivo…",
        "New Folder" => "Nueva carpeta",
        "New Folder…" => "Nueva carpeta…",
        "No matches" => "No hay coincidencias",
        "Open" => "Abrir",
        "Open file" => "Abrir archivo",
        "Open in New Tab" => "Abrir en una pestaña nueva",
        "Open in {name}" => "Abrir en {name}",
        "Open Osdy Code Desktop here" => "Abrir Osdy Code Desktop aquí",
        "Open to the Side" => "Abrir al lado",
        "Open with Default App" | "Open with default app" => "Abrir con la app predeterminada",
        "parent" => "carpeta superior",
        "Parent folder (-)" => "Carpeta superior (-)",
        "Paste" => "Pegar",
        "Refresh" => "Actualizar",
        "Rename…" => "Cambiar nombre…",
        "repo" => "repositorio",
        "terminal" => "terminal",
        "Reveal in Finder" => "Mostrar en el Finder",
        "Reveal in File Explorer" => "Mostrar en el Explorador de archivos",
        "Reveal in File Manager" => "Mostrar en el gestor de archivos",
        "Search files" => "Buscar archivos",
        "Show Ignored Files" => "Mostrar archivos ignorados",
        "This folder is empty" => "Esta carpeta está vacía",
        "“{name}” already exists in {folder}" => "«{name}» ya existe en {folder}",
        "“{name}” is not a valid name" | "“{name}” isn't a valid name" => {
            "«{name}» no es un nombre válido"
        }
        "“{name}” no longer exists" => "«{name}» ya no existe",

        // File and diff preview.
        "A new, empty file." => "Un archivo nuevo y vacío.",
        "Added" => "Agregado",
        "An empty file was deleted." => "Se eliminó un archivo vacío.",
        "Binary changes can't be shown as text." => {
            "Los cambios binarios no se pueden mostrar como texto."
        }
        "Binary file" => "Archivo binario",
        "Binary file changed" => "Archivo binario modificado",
        "Can't read this file" => "No se puede leer este archivo",
        "Changed on disk" => "Modificado en el disco",
        "Close pane" => "Cerrar panel",
        "Commit not found" => "No se encontró el commit",
        "Conflict" => "Conflicto",
        "Copied" => "Copiado",
        "Couldn't read the diff" => "No se pudo leer la comparación",
        "Couldn't save" => "No se pudo guardar",
        "Deleted" => "Eliminado",
        "Deleted on disk" => "Eliminado del disco",
        "Diff" => "Comparación",
        "Diff too large" => "Comparación demasiado grande",
        "Discard your edits and load the file from disk" => {
            "Descarta tus cambios y carga el archivo del disco"
        }
        "Dismiss" => "Ocultar",
        "Don't wrap long lines (scroll sideways)" => {
            "No ajustar líneas largas (desplazamiento horizontal)"
        }
        "Edit" => "Editar",
        "Hide this notice" => "Ocultar este aviso",
        "Keep mine" => "Conservar mis cambios",
        "Keep your edits; saving overwrites the file on disk" => {
            "Conserva tus cambios; al guardar se sobrescribe el archivo en el disco"
        }
        "{count} line" => "{count} línea",
        "{count} lines" => "{count} líneas",
        "Load anyway" => "Cargar de todos modos",
        "Mermaid couldn't draw this diagram: {error}" => {
            "Mermaid no pudo dibujar este diagrama: {error}"
        }
        "Mermaid diagram" => "Diagrama de Mermaid",
        "Modified" => "Modificado",
        "No changes" => "Sin cambios",
        "No content changes" => "Sin cambios en el contenido",
        "not a file path" => "la ruta no corresponde a un archivo",
        "Only the file mode changed." => "Solo cambió el modo del archivo.",
        "Plain text" => "Texto sin formato",
        "Preview" => "Vista previa",
        "Reload" => "Recargar",
        "Renamed" => "Renombrado",
        "Renamed without content changes." => "Cambió el nombre, pero no el contenido.",
        "Save ({shortcut})" => "Guardar ({shortcut})",
        "Save ({shortcut}) to write it back." => "Guárdalo ({shortcut}) para volver a crearlo.",
        "Saved" => "Guardado",
        "Shell" => "Terminal",
        "Split" => "Dividido",
        "Starting the download…" => "Iniciando la descarga…",
        "Syntax colors are off for content over {size}" => {
            "El resaltado de sintaxis se desactiva para contenido de más de {size}"
        }
        "The diagram renderer couldn't be installed: {error}" => {
            "No se pudo instalar el renderizador de diagramas: {error}"
        }
        "couldn't start the diagram renderer: {error}" => {
            "no se pudo iniciar el renderizador de diagramas: {error}"
        }
        "the diagram renderer gave no drawing" => "el renderizador no generó ningún diagrama",
        "the diagram renderer has no input" => "el renderizador de diagramas no tiene entrada",
        "the file is read-only" => "el archivo es de solo lectura",
        "This file isn't text. Open it with its default app instead." => {
            "Este archivo no es de texto. Ábrelo con su app predeterminada."
        }
        "This file matches HEAD." => "Este archivo coincide con HEAD.",
        "This file was changed outside the editor while you had unsaved edits." => {
            "El archivo cambió fuera del editor mientras tenías cambios sin guardar."
        }
        "This is a folder" => "Esto es una carpeta",
        "Too large to preview" => "Demasiado grande para la vista previa",
        "Unified" => "Unificado",
        "Wrap long lines" => "Ajustar líneas largas",
        "{blocks} Mermaid diagram here. Draw it with the diagram renderer, a one-time download of about 20 MB." => {
            "Aquí hay {blocks} diagrama de Mermaid. Para dibujarlo, descarga el renderizador de diagramas una sola vez (unos 20 MB)."
        }
        "{blocks} Mermaid diagrams here. Draw them with the diagram renderer, a one-time download of about 20 MB." => {
            "Aquí hay {blocks} diagramas de Mermaid. Para dibujarlos, descarga el renderizador de diagramas una sola vez (unos 20 MB)."
        }
        "{size} is over the {limit} diff limit." => {
            "{size} supera el límite de {limit} para la comparación."
        }
        "{size} is over the {limit} preview limit." => {
            "{size} supera el límite de {limit} para la vista previa."
        }
        "{size} is over {limit}, the most the app loads." => {
            "{size} supera los {limit} que puede cargar la app."
        }

        // Changes and references.
        "Amend" => "Enmendar",
        "Apply" => "Aplicar",
        "bare" => "sin árbol de trabajo",
        "Branches" => "Ramas",
        "Changes" => "Cambios",
        "Check out" => "Cambiar a esta rama",
        "Check out as local branch" => "Cambiar a una rama local",
        "Commit" => "Commit",
        "commit" => "commit",
        "commits" => "commits",
        "{count} commit" => "{count} commit",
        "{count} commits" => "{count} commits",
        "Commit & Push" => "Crear commit y publicar",
        "Committed, but the push failed: {error}" => {
            "Se creó el commit, pero no se pudo publicar: {error}"
        }
        "Committing and pushing…" => "Creando y publicando el commit…",
        "Committing…" => "Creando el commit…",
        "conflict" => "conflicto",
        "Could not start gentle-shell: {error}" => "No se pudo iniciar gentle-shell: {error}",
        "detached at {sha}" => "HEAD separado en {sha}",
        "Detached at {sha}" => "HEAD separado en {sha}",
        "Discard" => "Descartar",
        "Discard changes to “{name}”?" => "¿Descartar los cambios en «{name}»?",
        "Discard changes…" => "Descartar cambios…",
        "Drop" => "Eliminar",
        "Drop {name}?" => "¿Eliminar {name}?",
        "Drop…" => "Eliminar…",
        "Filter refs" => "Filtrar referencias",
        "from HEAD" => "desde HEAD",
        "Generate" => "Generar",
        "Generate message" => "Generar mensaje",
        "gentle-shell failed: {error}" => "gentle-shell falló: {error}",
        "gentle-shell returned no message" => "gentle-shell no devolvió ningún mensaje",
        "gone" => "ya no existe",
        "Group by folder" => "Agrupar por carpeta",
        "Its folder {path} is deleted; the branch stays. git refuses while it has uncommitted changes." => {
            "Se elimina la carpeta {path}, pero se conserva la rama. Git impide eliminarla si tiene cambios sin commit."
        }
        "List" => "Lista",
        "locked" => "bloqueado",
        "Message (optional)" => "Mensaje (opcional)",
        "moved" => "movido",
        "New worktree" => "Nuevo árbol de trabajo",
        "New worktree on branch “{name}”" => "Nuevo árbol de trabajo en la rama «{name}»",
        "No commits yet" => "Todavía no hay commits",
        "No refs match “{query}”" => "Ninguna referencia coincide con «{query}»",
        "No upstream" => "Sin rama de seguimiento",
        "Not a git repository" => "No es un repositorio de Git",
        "not initialized" => "sin inicializar",
        "Open in agent tab" => "Abrir en una pestaña de agente",
        "Open in the Git pane" => "Abrir en el panel de Git",
        "Open the stashes in Branches" => "Abrir los stashes en Ramas",
        "Pop" => "Aplicar y quitar",
        "prunable" => "se puede limpiar",
        "Prune" => "Limpiar",
        "Reading git status…" => "Consultando el estado de Git…",
        "Reading refs…" => "Leyendo referencias…",
        "Remotes" => "Remotos",
        "Remove the “{title}” worktree?" => "¿Eliminar el árbol de trabajo «{title}»?",
        "Remove worktree…" => "Eliminar árbol de trabajo…",
        "Repository: {path}" => "Repositorio: {path}",
        "Rewrite the last commit instead of making a new one" => {
            "Modifica el último commit en vez de crear uno nuevo"
        }
        "Show stashes" => "Mostrar stashes",
        "Stage" => "Preparar",
        "Stage all" => "Preparar todo",
        "Stage some changes first; the message is written from the staged diff" => {
            "Prepara algunos cambios primero; el mensaje se escribe a partir de esos cambios"
        }
        "Staged" => "Preparados",
        "Staged, unstaged and untracked changes move to a new stash and the working tree is left clean." => {
            "Los cambios preparados, sin preparar y sin seguimiento pasan a un nuevo stash. El árbol de trabajo queda limpio."
        }
        "Stash" => "Guardar en un stash",
        "Stash all changes" => "Guardar todos los cambios en un stash",
        "Stash changes" => "Guardar cambios en un stash",
        "Stash changes…" => "Guardar cambios en un stash…",
        "Stashes" => "Cambios en stash",
        "Submodules" => "Submódulos",
        "Summary" => "Resumen",
        "Switch" => "Cambiar",
        "Switch to “{branch}”?" => "¿Cambiar a «{branch}»?",
        "Tags" => "Etiquetas",
        "The file goes back to the last commit, staged edits included." => {
            "El archivo vuelve al último commit, incluso si tenía cambios preparados."
        }
        "The new file is removed from the index and deleted." => {
            "El archivo nuevo se quita del índice y se elimina."
        }
        "The untracked file is deleted." => "Se elimina el archivo sin seguimiento.",
        "This can't be undone." => "Esta acción no se puede deshacer.",
        "Unstage" => "Quitar de preparados",
        "Unstage all" => "Quitar todo de preparados",
        "Unstaged" => "Sin preparar",
        "Unstaged edits go back to the staged version." => {
            "Los cambios sin preparar vuelven a la versión preparada."
        }
        "Untracked" => "Sin seguimiento",
        "Why this change…" => "Motivo de este cambio…",
        "Working tree" => "Árbol de trabajo",
        "Working tree clean" => "No hay cambios en el árbol de trabajo",
        "Worktrees" => "Árboles de trabajo",
        "Write a summary first" => "Escribe primero un resumen",
        "Write a summary, or let Gentle Shell write it" => {
            "Escribe un resumen o deja que Gentle Shell lo escriba"
        }
        "Write the message from the staged diff with Gentle Shell" => {
            "Escribir el mensaje con Gentle Shell a partir de los cambios preparados"
        }
        "Writing…" => "Escribiendo…",
        "You have uncommitted changes. They come along to the other branch; git refuses the switch if they conflict." => {
            "Tienes cambios sin commit. Se trasladan a la otra rama, pero Git impide el cambio si hay conflictos."
        }
        "“{message}” is deleted. This can't be undone." => {
            "Se elimina «{message}». Esta acción no se puede deshacer."
        }

        // Git history, actions and activity.
        "Abort" => "Cancelar operación",
        "Abort the {operation}?" => "¿Cancelar esta operación ({operation})?",
        "Abort…" => "Cancelar operación…",
        "Aborting…" => "Cancelando operación…",
        "Add" => "Agregar",
        "Add remote" => "Agregar remoto",
        "Add remote…" => "Agregar remoto…",
        "Added remote {remote}" => "Se agregó el remoto {remote}",
        "Adding remote…" => "Agregando remoto…",
        "All branches" => "Todas las ramas",
        "binary" => "binario",
        "Branch" => "Rama",
        "Branch name" => "Nombre de la rama",
        "Changed" => "Cambios",
        "Checking out…" => "Cambiando al commit…",
        "Check out this commit" => "Cambiar a este commit",
        "Checked out {sha}" => "Se cambió al commit {sha}",
        "Cherry-pick" => "Aplicar commit",
        "Cherry-picked {sha}" => "Se aplicó {sha}",
        "Cherry-picking…" => "Aplicando commit…",
        "Co-authored with {authors}" => "En coautoría con {authors}",
        "Committed" => "Fecha del commit",
        "Continue" => "Continuar",
        "Continuing…" => "Continuando…",
        "Copy commit message" => "Copiar mensaje del commit",
        "Copy commit SHA" => "Copiar SHA del commit",
        "Create" => "Crear",
        "Created and switched to {branch}" => "Se creó y se seleccionó la rama {branch}",
        "Created tag {tag}" => "Se creó la etiqueta {tag}",
        "Creating branch…" => "Creando rama…",
        "Creating tag…" => "Creando etiqueta…",
        "Delete" => "Eliminar",
        "Delete tag {tag}?" => "¿Eliminar la etiqueta {tag}?",
        "Delete tag {tag}…" => "Eliminar etiqueta {tag}…",
        "Deleted tag {tag}" => "Se eliminó la etiqueta {tag}",
        "Deleting tag…" => "Eliminando etiqueta…",
        "Detached HEAD" => "HEAD separado",
        "Earlier history" => "Historial anterior",
        "Edit HEAD's message" => "Editar mensaje de HEAD",
        "Edit message…" => "Editar mensaje…",
        "Everything goes back to how it was before it started, including the conflicts you resolved." => {
            "Se restaura el estado anterior a la operación, incluidos los conflictos que resolviste."
        }
        "Explain with the agent" => "Pedir al agente que lo explique",
        "Fast-forward if possible" => "Avance rápido si es posible",
        "Fast-forward only" => "Solo avance rápido",
        "Fetch" => "Obtener",
        "Fetch all remotes" => "Obtener cambios de todos los remotos",
        "Fetched all remotes" => "Se obtuvieron cambios de todos los remotos",
        "Fetching all remotes…" => "Obteniendo cambios de todos los remotos…",
        "file" => "archivo",
        "files" => "archivos",
        "Filter commits" => "Filtrar commits",
        "GitHub has no account @{login}." => "No existe la cuenta @{login} en GitHub.",
        "GitHub is limiting requests; try again in a while." => {
            "GitHub está limitando las solicitudes; vuelve a intentarlo en un rato."
        }
        "GitHub showed no calendar for @{login}." => {
            "GitHub no mostró un calendario para @{login}."
        }
        "Couldn't reach GitHub." => "No se pudo conectar con GitHub.",
        "Couldn't tell your GitHub account. Set it with `git config --global github.user <login>`." => {
            "No se pudo identificar tu cuenta de GitHub. Configúrala con `git config --global github.user <login>`."
        }
        "HEAD (detached)" => "HEAD (separado)",
        "Jump to HEAD" => "Ir a HEAD",
        "Less" => "Menos",
        "LFS checkout" => "Recuperar archivos de LFS",
        "LFS prune" => "Limpiar LFS",
        "LFS pull" => "Descargar archivos de LFS",
        "LFS push" => "Subir archivos de LFS",
        "Merge" => "Fusión",
        "Merge of {count} parents" => "Fusión de {count} commits padre",
        "Message (optional: makes an annotated tag)" => {
            "Mensaje (opcional; crea una etiqueta anotada)"
        }
        "Message saved" => "Se guardó el mensaje",
        "More actions" => "Más acciones",
        "Name" => "Nombre",
        "New branch at HEAD" => "Nueva rama en HEAD",
        "New branch at {label}" => "Nueva rama en {label}",
        "New branch here…" => "Nueva rama aquí…",
        "New tag at {label}" => "Nueva etiqueta en {label}",
        "New tag here…" => "Nueva etiqueta aquí…",
        "No changes against the first parent." => "Sin cambios respecto al primer commit padre.",
        "No commits here yet." => "Todavía no hay commits aquí.",
        "No file changes." => "No cambió ningún archivo.",
        "No loaded commit matches the filter." => "Ningún commit cargado coincide con el filtro.",
        "Nothing" => "Nada",
        "now" => "ahora",
        "Open terminal here" => "Abrir terminal aquí",
        "Pull" => "Actualizar rama",
        "Pull integration" => "Método de integración",
        "Pull options" => "Opciones de actualización",
        "Pulled" => "Se actualizó la rama",
        "Pulling…" => "Actualizando rama…",
        "Push" => "Publicar",
        "Push (publishes the branch)" => "Publicar (sube la rama al remoto)",
        "Push tag {tag}" => "Publicar etiqueta {tag}",
        "Push to {upstream}" => "Publicar en {upstream}",
        "Pushed" => "Se publicaron los cambios",
        "Pushed tag {tag} to {remote}" => "Se publicó la etiqueta {tag} en {remote}",
        "Pushing tag…" => "Publicando etiqueta…",
        "Pushing…" => "Publicando…",
        "Reading history…" => "Leyendo el historial…",
        "Reading the changes…" => "Leyendo los cambios…",
        "Reading your GitHub contributions…" => "Cargando tus contribuciones de GitHub…",
        "Rebase" => "Reorganizar commits",
        "Renamed from {old}" => "Nombre anterior: {old}",
        "Reset and discard" => "Restablecer y descartar",
        "Reset to {sha}" => "Se restableció a {sha}",
        "Reset {branch} to {sha}?" => "¿Restablecer {branch} a {sha}?",
        "Reset {target} here, discard changes…" => {
            "Restablecer {target} aquí y descartar cambios…"
        }
        "Reset {target} here, keep changes" => "Restablecer {target} aquí y conservar cambios",
        "Resetting…" => "Restableciendo…",
        "Revert" => "Revertir",
        "Reverted {sha}" => "Se revirtió {sha}",
        "Reverting…" => "Revirtiendo…",
        "Running Git LFS…" => "Ejecutando Git LFS…",
        "Save" => "Guardar",
        "Saving message…" => "Guardando mensaje…",
        "Show branch" => "Mostrar rama",
        "Showing {shown} of {total} commits" => "Se muestran {shown} de {total} commits",
        "Stashed changes" => "Se guardaron los cambios en un stash",
        "Stashing…" => "Guardando en un stash…",
        "Switch to {branch}" => "Cambiar a {branch}",
        "Switched to {branch}" => "Se cambió a {branch}",
        "Switching…" => "Cambiando de rama…",
        "Tag name" => "Nombre de la etiqueta",
        "The commits after it leave the branch, and every uncommitted change is discarded. Changes that were never committed can't be recovered." => {
            "Los commits posteriores salen de la rama y se descartan todos los cambios sin commit. Esos cambios no se pueden recuperar."
        }
        "The tag is removed here; copies already pushed to a remote stay." => {
            "La etiqueta se elimina aquí; las copias ya publicadas en un remoto se conservan."
        }
        "This folder isn't in a Git repository." => {
            "Esta carpeta no está en un repositorio de Git."
        }
        "This repo" => "Este repositorio",
        "This repository has no remote" => "Este repositorio no tiene ningún remoto",
        "This repository has no remote to push to" => {
            "Este repositorio no tiene un remoto donde publicar"
        }
        "Uncommitted changes" => "Cambios sin commit",
        "Write a message first" => "Escribe primero un mensaje",
        "Write a name first" => "Escribe primero un nombre",
        "Write the remote's name and URL" => "Escribe el nombre y la URL del remoto",
        "Your GitHub contributions" => "Tus contribuciones de GitHub",
        "{description} This can't be undone." => "{description} Esta acción no se puede deshacer.",
        "{name} · diff" => "{name} · comparación",
        "{count}w" => "{count} sem",
        "{count}mo" => "{count} mes",
        "{count}y" => "{count} a",
        "{count} file changed" => "{count} archivo modificado",
        "{count} files changed" => "{count} archivos modificados",
        "{operation} aborted" => "Operación cancelada: {operation}",
        "{operation} done" => "Operación completada: {operation}",
        "{operation} finished" => "Operación completada: {operation}",
        "{operation} in progress. Resolve the conflicts and stage them in Changes, then continue." => {
            "Operación en curso: {operation}. Resuelve los conflictos y prepara los cambios en Cambios; luego continúa."
        }
        "{shown} of {total} loaded commits match" => {
            "Coinciden {shown} de {total} commits cargados"
        }
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
        "Sun" => "dom",
        "Mon" => "lun",
        "Tue" => "mar",
        "Wed" => "mié",
        "Thu" => "jue",
        "Fri" => "vie",
        "Sat" => "sáb",
        "contribution" => "contribución",
        "contributions" => "contribuciones",
        "{weekday}, {month} {day}, {year}" => "{weekday}, {day} de {month} de {year}",
        "No {unit} on {date}" => "Sin {unit} el {date}",
        "{count} {unit} on {date}" => "{count} {unit} el {date}",
        "{count} {unit} in the last year" => "{count} {unit} en el último año",
        "{count} {unit} in {weeks} weeks" => "{count} {unit} en {weeks} semanas",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::git::activity::{Day, Unit, WEEKS, day_summary, heading};
    use crate::i18n::{Language, tr};
    use crate::usage::time::days_from_civil;

    #[test]
    fn files_git_translates_its_ui_in_spanish_but_not_in_english() {
        for (en, es) in [
            ("No commits yet", "Todavía no hay commits"),
            ("Write a name first", "Escribe primero un nombre"),
            (
                "Reveal in File Explorer",
                "Mostrar en el Explorador de archivos",
            ),
            ("{count} files changed", "{count} archivos modificados"),
        ] {
            assert_eq!(tr(Language::Spanish, en), es);
            assert_eq!(tr(Language::English, en), en);
        }
        let day = Day {
            day: days_from_civil(2026, 11, 20),
            count: 0,
            level: 0,
            future: false,
        };
        assert_eq!(
            day_summary(Language::Spanish, &day, Unit::Commits),
            "Sin commits el vie, 20 de nov de 2026"
        );
        assert_eq!(
            heading(Language::Spanish, 1, WEEKS, Unit::Contributions),
            "1 contribución en el último año"
        );
        assert_eq!(
            heading(Language::Spanish, 2, 20, Unit::Contributions),
            "2 contribuciones en 20 semanas"
        );
    }
}
