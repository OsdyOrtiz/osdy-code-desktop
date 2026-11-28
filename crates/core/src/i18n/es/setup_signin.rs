//! Spanish text of the setup signin area.

pub(super) fn lookup(en: &str) -> Option<&'static str> {
    Some(match en {
        // Provider selection and sign-in.
        "Search providers" => "Buscar proveedores",
        "Loading providers…" => "Cargando proveedores…",
        "Providers" => "Proveedores",
        "Credentials go to Gentle Shell's auth.json, like /login." => {
            "Las credenciales se guardan en el auth.json de Gentle Shell, igual que con /login."
        }
        "Agents here run on {label}, so they sign in there: open Gentle Shell in a terminal on {label} and run /login. Credentials go to its auth.json." => {
            "Los agentes se ejecutan en {label} e inician sesión allí: abre Gentle Shell en una terminal de {label} y ejecuta /login. Las credenciales se guardan en su auth.json."
        }
        "Open a terminal on {label}" => "Abrir una terminal en {label}",
        "The sign-in helper stopped." => "El asistente de inicio de sesión se detuvo.",
        "Back" => "Volver",
        "Sign in to" => "Iniciar sesión en",
        "Your browser opened the sign-in page. Finish there; if it does not return here, paste the code or redirect URL below." => {
            "La página de inicio de sesión se abrió en tu navegador. Termina allí; si no vuelves aquí automáticamente, pega abajo el código o la URL de redirección."
        }
        "Open the sign-in page again" => "Volver a abrir la página de inicio de sesión",
        "Enter this code on the verification page (copied to the clipboard):" => {
            "Introduce este código en la página de verificación (ya está en el portapapeles):"
        }
        "Open verification page" => "Abrir la página de verificación",
        "Waiting for the provider…" => "Esperando al proveedor…",
        "Signed in. Tabs without a conversation reload their models now; open a new tab for the others." => {
            "Sesión iniciada. Se recargan los modelos en las pestañas sin conversación. Para las demás, abre una pestaña nueva."
        }
        "Signed in" => "Con sesión iniciada",
        "Subscriptions" => "Suscripciones",
        "API keys" => "Claves de API",
        "No providers match" => "No se encontraron proveedores",
        "Try another name or id." => "Prueba con otro nombre o identificador.",
        "env" => "entorno",
        "signed in" => "sesión iniciada",
        "via {source}" => "mediante {source}",
        "Sign in" => "Iniciar sesión",
        "API key" => "Clave de API",
        "Sign out" => "Cerrar sesión",
        // Welcome and requirements checks.
        "Welcome to Osdy Code Desktop" => "Te damos la bienvenida a Osdy Code Desktop",
        "Looking at what this machine already has…" => {
            "Comprobando qué hay instalado en este equipo…"
        }
        "Let's get you set up" => "Configuremos tu equipo",
        "A few things Osdy Code Desktop needs are missing. Install them here; anything that needs your password opens in a terminal for you to run." => {
            "Faltan algunos requisitos de Osdy Code Desktop. Instálalos aquí. Si un paso requiere tu contraseña, se abrirá una terminal para que lo ejecutes."
        }
        "You're all set" => "Ya está todo listo",
        "Everything Osdy Code Desktop needs is here. Optional extras are below." => {
            "Osdy Code Desktop ya tiene todo lo que necesita. Debajo verás los extras opcionales."
        }
        "Everything required is ready. The recommended extras below make it better." => {
            "Ya está todo lo necesario. Los extras recomendados de abajo mejoran la experiencia."
        }
        "Welcome to Osdy Code Desktop." => "Te damos la bienvenida a Osdy Code Desktop.",
        "You're ready." => "Ya está todo listo.",
        "Let's get you set up." => "Configuremos tu equipo.",
        "Everything Osdy Code Desktop needs is installed. Sign in to a model provider now, or later from the chat." => {
            "Ya está instalado todo lo que necesita Osdy Code Desktop. Inicia sesión en un proveedor de modelos ahora o más tarde desde el chat."
        }
        "Everything Osdy Code Desktop needs is here. Open your first agent whenever you like; the extras below make it better." => {
            "Osdy Code Desktop ya tiene todo lo que necesita. Abre tu primer agente cuando quieras. Los extras de abajo mejoran la experiencia."
        }
        "A few steps and you're in. Everything installs right here; anything that needs your password opens in a terminal for you to run." => {
            "Solo faltan unos pasos. Todo se instala aquí. Si un paso requiere tu contraseña, se abrirá una terminal para que lo ejecutes."
        }
        "Checking again…" => "Comprobando de nuevo…",
        "Starting…" => "Iniciando…",
        "Retry" => "Reintentar",
        "Update" => "Actualizar",
        "Open terminal" => "Abrir terminal",
        "Required" => "Obligatorio",
        "Recommended" => "Recomendado",
        "Optional" => "Opcional",
        "Skip for now" => "Omitir por ahora",
        "Continue" => "Continuar",
        "Installing…" => "Instalando…",
        "Install everything" => "Instalar todo",
        "Checking Node, Gentle Shell and the rest…" => {
            "Comprobando Node, Gentle Shell y los demás requisitos…"
        }
        "Installing" => "Instalando",
        "Missing" => "Falta",
        "Outdated" => "Desactualizado",
        "Failed" => "Falló",
        "{done} of {steps} ready" => "Pasos completados: {done} de {steps}",
        "Also good to have" => "Extras útiles",
        "Agents on {backend}" => "Agentes en {backend}",
        "Runs Gentle Shell and its packages (22.19 or newer, with npm)." => {
            "Ejecuta Gentle Shell y sus paquetes (22.19 o posterior, con npm)."
        }
        "Builds the Gentle AI companion on Windows (1.25.10 or newer)." => {
            "Compila el complemento de Gentle AI en Windows (1.25.10 o posterior)."
        }
        "The gentle-pi launcher and the Pi agent it bundles." => {
            "El iniciador gentle-pi y el agente Pi que incluye."
        }
        "Companion setup" => "Configuración del complemento",
        "Gentle AI's packages, skills and memory in your Gentle Shell home." => {
            "Paquetes, habilidades y memoria de Gentle AI en tu directorio de Gentle Shell."
        }
        "AI provider" => "Proveedor de IA",
        "Sign in to at least one model provider to chat." => {
            "Inicia sesión en al menos un proveedor de modelos para chatear."
        }
        "Reviews, worktrees, changes and branches." => {
            "Revisiones, árboles de trabajo, cambios y ramas."
        }
        "Long-term memory across sessions." => "Memoria a largo plazo entre sesiones.",
        "Code graph for the agent: symbols, references and calls (MCP)." => {
            "Grafo de código para el agente: símbolos, referencias y llamadas (MCP)."
        }
        "Codegraph setup is only available for local agents." => {
            "Codegraph solo se puede configurar para agentes locales."
        }
        "Codegraph CLI did not report a version." => "La CLI de Codegraph no indicó su versión.",
        "Codegraph CLI is unavailable. Install Codegraph first." => {
            "La CLI de Codegraph no está disponible. Instala Codegraph primero."
        }
        "Install Gentle Shell first." => "Instala Gentle Shell primero.",
        "Installing Codegraph with npm…" => "Instalando Codegraph con npm…",
        "Codegraph was installed, but its CLI did not report a version." => {
            "Codegraph se instaló, pero su CLI no indicó su versión."
        }
        "Registering Codegraph with the agent…" => "Registrando Codegraph en el agente…",
        "Gentle Shell did not enable Codegraph in mcp.json." => {
            "Gentle Shell no habilitó Codegraph en mcp.json."
        }
        "Disconnecting Codegraph from the agent…" => "Desconectando Codegraph del agente…",
        "Codegraph is still enabled in mcp.json." => "Codegraph sigue habilitado en mcp.json.",
        "Gentle Shell runs the Node.js in gentle-ai's managed install" => {
            "Gentle Shell usa el Node.js de la instalación gestionada por gentle-ai"
        }
        "Not needed: gentle-ai's managed install includes Gentle AI" => {
            "No hace falta: la instalación gestionada por gentle-ai incluye Gentle AI"
        }
        "Go is only needed on Windows." => "Go solo hace falta en Windows.",
        "Sign in from the Providers screen." => "Inicia sesión desde la pantalla de proveedores.",
        "Node.js isn't installed" => "Node.js no está instalado",
        "{path} doesn't run" => "No se puede ejecutar {path}",
        "Unexpected `node --version`: {output}" => {
            "Resultado inesperado de `node --version`: {output}"
        }
        "Node {version} — needs {minimum} or newer" => {
            "Node {version}: se requiere la versión {minimum} o posterior"
        }
        "Node {version} found, but not npm" => "Se encontró Node {version}, pero no npm",
        "winget isn't available on this system." => "winget no está disponible en este sistema.",
        "Go isn't installed" => "Go no está instalado",
        "Go {version} — needs {minimum} or newer" => {
            "Go {version}: se requiere la versión {minimum} o posterior"
        }
        "Go was installed, but this app can't find it yet." => {
            "Go se instaló, pero la app todavía no lo encuentra."
        }
        "Git isn't installed" => "Git no está instalado",
        "Install Git from your system's package manager." => {
            "Instala Git desde el gestor de paquetes de tu sistema."
        }
        "Git was installed, but this app can't find it yet." => {
            "Git se instaló, pero la app todavía no lo encuentra."
        }
        "ENGRAM_BIN points to {path}, which can't be run. Fix or remove it." => {
            "ENGRAM_BIN apunta a {path}, que no se puede ejecutar. Corrige o elimina la variable."
        }
        "Engram {version} — needs {minimum} or newer" => {
            "Engram {version}: se requiere la versión {minimum} o posterior"
        }
        "Engram was installed, but it doesn't run." => {
            "Engram se instaló, pero no se puede ejecutar."
        }
        "Engram isn't installed." => "Engram no está instalado.",
        "Engram was updated, but it doesn't run." => {
            "Engram se actualizó, pero no se puede ejecutar."
        }
        "Signed in: {providers}" => "Sesión iniciada: {providers}",
        "{key} is set" => "{key} está definida",
        "Claude through your Claude Code sign-in" => "Claude mediante tu sesión de Claude Code",
        "No provider signed in" => "No has iniciado sesión en ningún proveedor",
        "gentle-shell isn't installed" => "gentle-shell no está instalado",
        "Pi wasn't found next to gentle-shell" => "No se encontró Pi junto a gentle-shell",
        "gentle-shell {shell} — needs {minimum} or newer for Gentle Desktop" => {
            "gentle-shell {shell}: Gentle Desktop requiere la versión {minimum} o posterior"
        }
        "pi {pi} — needs {minimum} or newer" => {
            "pi {pi}: se requiere la versión {minimum} o posterior"
        }
        "Needs a working Gentle Shell first" => {
            "Primero necesitas una instalación funcional de Gentle Shell"
        }
        "Linked to {home}" => "Vinculado a {home}",
        "{home} isn't set up yet" => "{home} aún no está configurado",
        "Gentle AI {version} · set up {date}" => "Gentle AI {version} · configuración: {date}",
        "Gentle AI companion · set up {date}" => {
            "Complemento de Gentle AI · configuración: {date}"
        }
        "Not set up yet — the first chat would do it silently" => {
            "Aún sin configurar: el primer chat lo configuraría sin avisar"
        }
        "Set up with Gentle AI {recorded}; gentle-pi now pins {pinned}" => {
            "Configurado con Gentle AI {recorded}; gentle-pi ahora fija la versión en {pinned}"
        }
        "Set up with gentle-pi {version}; {installed} is installed" => {
            "Configurado con gentle-pi {version}; está instalada la versión {installed}"
        }
        "Set up with gentle-pi an older version; {installed} is installed" => {
            "Configurado con una versión anterior de gentle-pi; está instalada la versión {installed}"
        }
        "Setup finished, but it isn't recorded as done: {reason}" => {
            "La configuración terminó, pero no quedó registrada como completa: {reason}"
        }
        "Only Windows' gentle-shell is visible from {label} ({path})" => {
            "Desde {label} solo se puede ver el gentle-shell de Windows ({path})"
        }
        "gentle-shell isn't installed in {label}" => "gentle-shell no está instalado en {label}",
        "No provider signed in: run /login in Gentle Shell" => {
            "No has iniciado sesión en ningún proveedor. Ejecuta /login en Gentle Shell"
        }
        "Install git in {label}" => "Instala git en {label}",
        "Not installed in {label}" => "Sin instalar en {label}",
        "Not needed" => "No hace falta",
        "Node.js isn't installed in {label}" => "Node.js no está instalado en {label}",
        "Set up {date}" => "Configurado el {date}",
        "Set up with Gentle AI {recorded}" => "Configurado con Gentle AI {recorded}",
        "This system has no app data folder." => {
            "Este sistema no tiene una carpeta de datos para la app."
        }
        "Couldn't save the device settings: {error}" => {
            "No se pudo guardar la configuración de dispositivos: {error}"
        }
        "iOS simulators need a Mac." => "Los simuladores de iOS requieren una Mac.",
        "Streaming iOS simulators needs Apple silicon." => {
            "Para transmitir simuladores de iOS necesitas Apple silicon."
        }
        "Install Xcode for iOS simulators." => "Instala Xcode para usar simuladores de iOS.",
        "Node.js isn't installed (Setup & requirements)." => {
            "Node.js no está instalado (Configuración inicial y requisitos)."
        }
        "{package} needs Node.js {minimum} or newer; this one is {version}." => {
            "{package} requiere Node.js {minimum} o posterior; la versión instalada es {version}."
        }
        "npm isn't installed (Setup & requirements)." => {
            "npm no está instalado (Configuración inicial y requisitos)."
        }
        "npm finished, but {package} isn't where it should be." => {
            "npm terminó, pero {package} no está en la ubicación esperada."
        }
        "Couldn't reach {label}." => "No se pudo conectar con {label}.",
        "Couldn't tell {label}'s machine." => "No se pudo identificar la arquitectura de {label}.",
        "uv finished, but gentle-stream isn't in {music}/bin." => {
            "uv terminó, pero gentle-stream no está en {music}/bin."
        }
        "gentle-stream runs on macOS and Linux only." => {
            "gentle-stream solo funciona en macOS y Linux."
        }
        "uv finished, but gentle-stream isn't where it should be." => {
            "uv terminó, pero gentle-stream no está en la ubicación esperada."
        }
        "Install gentle-stream first." => "Instala gentle-stream primero.",
        "Couldn't delete {path}: {error}" => "No se pudo eliminar {path}: {error}",
        "Couldn't delete {path} in {label}." => "No se pudo eliminar {path} en {label}.",
        "Couldn't find Gentle Shell's package folder in {label}." => {
            "No se encontró la carpeta del paquete de Gentle Shell en {label}."
        }
        "Couldn't read Gentle Shell's version in {label}." => {
            "No se pudo leer la versión de Gentle Shell en {label}."
        }
        "npx was not found next to Node.js or on the PATH." => {
            "No se encontró npx junto a Node.js ni en el PATH."
        }
        "Couldn't find Gentle Shell's package folder." => {
            "No se encontró la carpeta del paquete de Gentle Shell."
        }
        "Couldn't read Gentle Shell's version." => "No se pudo leer la versión de Gentle Shell.",
        "Couldn't unpack the source: {error}" => "No se pudo extraer el código fuente: {error}",
        "Gentle Shell {version}'s source has no web/tower." => {
            "El código fuente de Gentle Shell {version} no incluye web/tower."
        }
        "The build finished without dist/index.html." => {
            "La compilación terminó sin generar dist/index.html."
        }
        "Couldn't write into {path}: {error}" => "No se pudo escribir en {path}: {error}",
        "gentle-shell didn't start: {error}" => "No se pudo iniciar gentle-shell: {error}",
        "Pi lists no maestro server." => "Pi no muestra ningún servidor maestro.",
        "Java was installed, but it doesn't run." => "Java se instaló, pero no se puede ejecutar.",
        "Maestro was installed, but this app can't find it." => {
            "Maestro se instaló, pero la app no lo encuentra."
        }
        "gentle-shell added Maestro, but it isn't in this home's mcp.json." => {
            "gentle-shell agregó Maestro, pero no aparece en el mcp.json de este directorio."
        }
        "gentle-shell added Maestro, but it isn't in {label}'s mcp.json." => {
            "gentle-shell agregó Maestro, pero no aparece en el mcp.json de {label}."
        }
        "Couldn't delete them in {label}." => "No se pudieron eliminar en {label}.",
        "Couldn't find a folder for Engram in {label}." => {
            "No se encontró una carpeta para Engram en {label}."
        }
        "Engram is in {path}, but it doesn't run there: is that folder on the PATH?" => {
            "Engram está en {path}, pero no se puede ejecutar. ¿Está esa carpeta en el PATH?"
        }
        "Engram is in {path}, but `engram version` says: {output}" => {
            "Engram está en {path}, pero `engram version` indica: {output}"
        }
        "Couldn't reach {host}: {error}" => "No se pudo conectar con {host}: {error}",
        "{host} answered HTTP {status} for {url}" => "{host} respondió HTTP {status} para {url}",
        "Couldn't read {url}: {error}" => "No se pudo leer {url}: {error}",
        "the server closed the connection early" => "el servidor cerró la conexión antes de tiempo",
        "Couldn't restart the download" => "No se pudo reiniciar la descarga",
        "{name} doesn't match its published checksum; refusing to install it." => {
            "{name} no coincide con la suma de verificación publicada. No se instalará."
        }
        "The checksums list has no entry for {name}; refusing to install an unverified download." => {
            "La lista de sumas de verificación no incluye {name}. No se instalará una descarga sin verificar."
        }
        "Couldn't unpack {name}: {error}" => "No se pudo extraer {name}: {error}",
        "nodejs.org's release index is unreadable" => {
            "No se pudo leer la lista de versiones de nodejs.org"
        }
        "nodejs.org lists no Node.js 22 LTS release." => {
            "nodejs.org no muestra ninguna versión de Node.js 22 LTS."
        }
        "nodejs.org has no Node.js build for this system." => {
            "nodejs.org no ofrece una compilación de Node.js para este sistema."
        }
        "{asset} doesn't have the expected layout." => "{asset} no tiene la estructura esperada.",
        "Couldn't replace the previous Node.js in {path} ({error}). Close anything using it and try again." => {
            "No se pudo reemplazar la instalación anterior de Node.js en {path} ({error}). Cierra los programas que la usen y vuelve a intentarlo."
        }
        "Couldn't move Node.js into {path}: {error}" => {
            "No se pudo mover Node.js a {path}: {error}"
        }
        "GitHub's answer about Engram's latest release is unreadable." => {
            "No se pudo leer la respuesta de GitHub sobre la última versión de Engram."
        }
        "Engram publishes no build for this system." => {
            "Engram no ofrece una compilación para este sistema."
        }
        "Engram {version} has no {name}." => "Engram {version} no incluye {name}.",
        "Couldn't install {path}: {error}" => "No se pudo instalar {path}: {error}",
        "The diagram renderer has no build for this system." => {
            "No hay una compilación del generador de diagramas para este sistema."
        }
        "{name} has no {exe}." => "{name} no incluye {exe}.",
        "Java runtime" => "entorno de ejecución de Java",
        "The {component} download doesn't have the expected layout." => {
            "La descarga de {component} no tiene la estructura esperada."
        }
        "Couldn't replace the previous {component} in {path} ({error}). Close anything using it and try again." => {
            "No se pudo reemplazar la instalación anterior de {component} en {path} ({error}). Cierra los programas que la usen y vuelve a intentarlo."
        }
        "Couldn't move {component} into {path}: {error}" => {
            "No se pudo mover {component} a {path}: {error}"
        }
        "Adoptium publishes no Java runtime for this system." => {
            "Adoptium no ofrece un entorno de ejecución de Java para este sistema."
        }
        "Adoptium's answer about its Java runtimes is unreadable." => {
            "No se pudo leer la respuesta de Adoptium sobre sus entornos de ejecución de Java."
        }
        "GitHub's answer about Maestro's latest release is unreadable." => {
            "No se pudo leer la respuesta de GitHub sobre la última versión de Maestro."
        }
        "Maestro {version} has no {name}." => "Maestro {version} no incluye {name}.",
        "Maestro {version} publishes no checksums; refusing to install an unverified download." => {
            "Maestro {version} no publica sumas de verificación. No se instalará una descarga sin verificar."
        }
        "uv has no build for this system." => "No hay una compilación de uv para este sistema.",
        "uv has nowhere to go." => "uv no tiene una carpeta de destino.",
        "{name} doesn't match its pinned checksum; refusing to install it." => {
            "{name} no coincide con la suma de verificación establecida. No se instalará."
        }
        "The uv download has no uv executable." => {
            "La descarga de uv no contiene el ejecutable uv."
        }
        "Couldn't install uv" => "No se pudo instalar uv",
        "Couldn't make uv executable" => "No se pudieron asignar permisos de ejecución a uv",
        // Backend messages.
        "no data folder" => "no hay carpeta de datos",
        "{backend} can't reach {path}. Open the project from a folder it can reach." => {
            "{backend} no puede acceder a {path}. Abre el proyecto desde una carpeta a la que tenga acceso."
        }
        "Only Windows' gentle-shell is visible from {backend} ({path}). Install it there: Setup & requirements has the command." => {
            "Desde {backend} solo se puede ver el gentle-shell de Windows ({path}). Instala gentle-shell en ese entorno. Encontrarás el comando en Configuración inicial y requisitos."
        }
        "Agents run on this computer." => "Los agentes se ejecutan en este equipo.",
        "Couldn't write {path} on {backend}." => "No se pudo escribir {path} en {backend}.",
        "{backend} didn't answer while copying {path}." => {
            "{backend} no respondió al copiar {path}."
        }
        "Couldn't reach {backend}: {error}" => "No se pudo conectar con {backend}: {error}",
        "no input" => "sin entrada",
        "The copy stopped." => "La copia se detuvo.",
        "Couldn't copy into {backend}: {error}" => "No se pudo copiar en {backend}: {error}",
        "Couldn't unpack into {path} on {backend}: {error}" => {
            "No se pudo extraer en {path} de {backend}: {error}"
        }
        "Couldn't start {command}: {error}" => "No se pudo iniciar {command}: {error}",
        "{command} didn't finish: {error}" => "{command} no terminó: {error}",
        "{command} failed (exit code {code})." => "{command} falló (código de salida {code}).",
        "{command} was stopped." => "Se detuvo {command}.",
        "npm wasn't found. Install Node.js first." => {
            "No se encontró npm. Instala Node.js primero."
        }
        "Couldn't create {path}: {error}" => "No se pudo crear {path}: {error}",
        "gentle-shell upgrade finished, but the launcher the app runs still reports: {reason}" => {
            "gentle-shell upgrade terminó, pero el iniciador que usa la app sigue indicando: {reason}"
        }
        "Git needs your password to install: run `{command}`." => {
            "Se necesita tu contraseña para instalar Git: ejecuta `{command}`."
        }
        "Engram was installed, but another one runs: {reason}" => {
            "Engram se instaló, pero se ejecuta otra copia: {reason}"
        }
        // Gentle Tower.
        "Couldn't start gentle-shell tower: {error}" => {
            "No se pudo iniciar gentle-shell tower: {error}"
        }
        "no output" => "sin salida",
        "no error output" => "sin salida de error",
        "gentle-shell tower ended without serving. Is your Gentle Shell up to date?" => {
            "gentle-shell tower terminó sin iniciar el servidor. ¿Está actualizado Gentle Shell?"
        }
        "Starting Gentle Tower…" => "Iniciando Gentle Tower…",
        "Building Gentle Tower (a few minutes, once)…" => {
            "Compilando Gentle Tower (tarda unos minutos y solo se hace una vez)…"
        }
        "This Gentle Shell came without Tower's web app" => {
            "Esta instalación de Gentle Shell no incluye la app web de Tower"
        }
        "Gentle Tower didn't start" => "No se pudo iniciar Gentle Tower",
        "Enable builds it from the same version's source on GitHub and adds it to your install. It takes a few minutes, once." => {
            "Al activarla, se compila el código de la misma versión en GitHub y se agrega a tu instalación. Solo se hace una vez y tarda unos minutos."
        }
        // Gentle Shell and Pi update status.
        "Channel" => "Canal",
        "updated by gentle-ai" => "gentle-ai se encarga de actualizarlo",
        "newer commit {latest} on main" => "commit más reciente en main: {latest}",
        "{latest} is available" => "{latest} está disponible",
        "an update is available" => "hay una actualización disponible",
        "Update command" => "Comando de actualización",
        "a newer version" => "una versión más reciente",
        " (you have {version})" => " (tienes {version})",
        "{latest} is available{current} · run `{command}`" => {
            "{latest} está disponible{current} · ejecuta `{command}`"
        }
        "not checked: Gentle Shell checks Pi only on bundled installs" => {
            "sin comprobar: Gentle Shell solo comprueba Pi cuando viene incluido"
        }
        "Last checked" => "Última comprobación",
        "managed by gentle-ai" => "gestionado por gentle-ai",
        "release" => "estable",
        "bundled" => "incluido",
        "unknown" => "desconocido",
        "up to date ({version})" => "al día ({version})",
        "not checked: Gentle Shell is offline (PI_OFFLINE)" => {
            "sin comprobar: Gentle Shell está sin conexión (PI_OFFLINE)"
        }
        "not checked yet" => "aún sin comprobar",
        "couldn't check" => "no se pudo comprobar",
        "update" => "actualización",
        "update → {latest}" => "actualización → {latest}",
        "pinned" => "versión fijada",
        "not checked" => "sin comprobar",
        "no data folder to record into" => "no hay una carpeta de datos donde guardar la grabación",
        "could not start a recording in {folder}: {error}" => {
            "no se pudo iniciar la grabación en {folder}: {error}"
        }
        // Desktop updater.
        "Installing Osdy Code Desktop {version}" => "Instalando Osdy Code Desktop {version}",
        "Restart to update" => "Reiniciar para actualizar",
        "Couldn't remember the skip" => "No se pudo guardar la decisión de omitir esta versión",
        "Osdy Code Desktop {version} is installed, but the restart couldn't be scheduled ({error}). Quit and reopen the app." => {
            "Osdy Code Desktop {version} está instalado, pero no se pudo programar el reinicio ({error}). Cierra y vuelve a abrir la app."
        }
        "Couldn't reach GitHub" => "No se pudo conectar con GitHub",
        "GitHub answered HTTP {status} for {url}" => "GitHub respondió HTTP {status} para {url}",
        "GitHub answered HTTP {status}." => "GitHub respondió HTTP {status}.",
        "No release has been published yet." => "Todavía no se ha publicado ninguna versión.",
        "GitHub's rate limit was hit; try again in a while." => {
            "Se alcanzó el límite de solicitudes de GitHub. Inténtalo de nuevo más tarde."
        }
        "Couldn't read GitHub's answer" => "No se pudo leer la respuesta de GitHub",
        "GitHub's latest release is not a stable vX.Y.Z release." => {
            "La última versión publicada en GitHub no es una versión estable vX.Y.Z."
        }
        "Couldn't create the download file" => "No se pudo crear el archivo de descarga",
        "The download was interrupted" => "Se interrumpió la descarga",
        "The download is larger than announced; stopped." => {
            "La descarga supera el tamaño anunciado y se detuvo."
        }
        "Couldn't write the download" => "No se pudo guardar la descarga",
        "Couldn't read {file}" => "No se pudo leer {file}",
        "Running from a build directory; download the release from GitHub." => {
            "La app se está ejecutando desde una carpeta de compilación. Descarga la versión publicada en GitHub."
        }
        "macOS is running Osdy Code Desktop from a temporary read-only copy. Move Osdy Code.app to Applications, open it from there, then update." => {
            "macOS está ejecutando Osdy Code Desktop desde una copia temporal de solo lectura. Mueve Osdy Code.app a Aplicaciones, ábrela desde allí y actualízala."
        }
        "Self-update isn't available on {platform}; download the release from GitHub." => {
            "No se puede actualizar la app desde {platform}. Descarga la versión publicada en GitHub."
        }
        "Osdy Code Desktop can't write to {path}. Move the app to a folder you own (e.g. ~/Applications) or install the update from GitHub yourself." => {
            "Osdy Code Desktop no puede escribir en {path}. Mueve la app a una carpeta de tu propiedad (por ejemplo, ~/Applications) o instala la actualización desde GitHub."
        }
        "Couldn't stage the update" => "No se pudo preparar la actualización",
        "Couldn't replace the app" => "No se pudo reemplazar la app",
        "Couldn't locate the running app" => "No se pudo localizar la app en ejecución",
        "Release {version} has no download for {os}-{arch}." => {
            "La versión {version} no tiene una descarga para {os}-{arch}."
        }
        "Release {version} has no {file}; refusing to install an unverified download." => {
            "La versión {version} no incluye {file}. No se instalará una descarga sin verificar."
        }
        "Couldn't create a download folder" => "No se pudo crear la carpeta de descarga",
        "Couldn't extract the update" => "No se pudo extraer la actualización",
        "The update archive is not what was expected" => {
            "El archivo de actualización no era el esperado"
        }
        "SHA256SUMS has no entry for {name}; refusing to install an unverified download." => {
            "SHA256SUMS no contiene una entrada para {name}. No se instalará una descarga sin verificar."
        }
        "{name} failed verification (sha256 {actual}, expected {expected}); nothing was installed." => {
            "No se pudo verificar {name} (sha256 {actual}; se esperaba {expected}). No se instaló nada."
        }
        "the archive has an entry outside its folder" => {
            "el archivo contiene una entrada fuera de su carpeta"
        }
        "the archive links {path} outside its folder" => {
            "el archivo enlaza {path} fuera de su carpeta"
        }
        "{path} is a symlink; this platform's archive should not have one" => {
            "{path} es un enlace simbólico; el archivo para esta plataforma no debería contener enlaces simbólicos"
        }
        "not a valid .tar.xz: {error}" => "no es un .tar.xz válido: {error}",
        "the archive should hold exactly one .app" => {
            "el archivo debe contener exactamente un .app"
        }
        "{path} has no Contents/MacOS/{binary}" => "{path} no contiene Contents/MacOS/{binary}",
        "the archive has no {path}" => "el archivo no contiene {path}",
        "ditto failed ({status})" => "ditto falló ({status})",
        "no data directory" => "no hay directorio de datos",
        "the launcher config is not a JSON object" => {
            "la configuración del iniciador no es un objeto JSON"
        }
        "{path} didn't report its version (Node.js or Pi may be missing)" => {
            "{path} no informó su versión (puede faltar Node.js o Pi)"
        }
        "On Windows gentle-pi builds its Gentle AI companion with Go {version} or newer. Install Go first, then try again." => {
            "En Windows, gentle-pi compila el complemento de Gentle AI con Go {version} o posterior. Instala Go primero y vuelve a intentarlo."
        }
        "No second gentle-pi copy was installed." => "No se instaló otra copia de gentle-pi.",
        "gentle-pi was installed, but gentle-shell still isn't on the PATH." => {
            "gentle-pi se instaló, pero gentle-shell todavía no está en el PATH."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::backend::Backend;
    use crate::i18n::{Language, tr};

    #[test]
    fn setup_signin_translations_keep_english_available() {
        for (en, es) in [
            ("Loading providers…", "Cargando proveedores…"),
            ("Open verification page", "Abrir la página de verificación"),
            ("Go isn't installed", "Go no está instalado"),
        ] {
            assert_eq!(tr(Language::Spanish, en), es);
            assert_eq!(tr(Language::English, en), en);
        }
        assert_eq!(Backend::Local.label(), "This computer");
        assert_eq!(Backend::Local.label_in(Language::Spanish), "Este equipo");
        let wsl = Backend::Wsl {
            distro: "Ubuntu".into(),
        };
        assert_eq!(wsl.label_in(Language::Spanish), "WSL · Ubuntu");
    }
}
