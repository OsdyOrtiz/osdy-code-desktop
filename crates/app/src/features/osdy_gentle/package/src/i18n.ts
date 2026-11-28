import { AsyncLocalStorage } from "node:async_hooks";
import { readFileSync } from "node:fs";

export type Locale = "en" | "es";

export function resolveLocale(env: NodeJS.ProcessEnv = process.env): Locale {
	const path = env.OSDY_DESKTOP_LOCALE_FILE;
	if (path) {
		try {
			const locale = readFileSync(path, "utf8").trim();
			if (locale === "en" || locale === "es") return locale;
		} catch {
			// The app file is optional (e.g. terminal sessions and remote agents).
		}
	}
	const locale = env.LC_ALL || env.LC_MESSAGES || env.LANG || "";
	return locale.toLowerCase().startsWith("es") ? "es" : "en";
}

// As in the desktop catalog, the keys are the English templates.
const es = {
	// commands
	"No accounts yet. Use /osdy-account to save the current sign-in or import Osdy-Pi accounts.": "Aún no hay cuentas. Usa /osdy-account para guardar la sesión actual o importar cuentas de Osdy-Pi.",
	"Checking Codex quota for stored accounts…": "Consultando la cuota de Codex de las cuentas guardadas…",
	" (active)": " (activa)",
	"model: {bound}": "modelo: {bound}",
	"Waiting for the agent to finish before switching…": "Esperando a que el agente termine antes de cambiar de cuenta…",
	"The agent is busy; nothing was switched. Try again when it is idle.": "El agente está ocupado y no se hizo ningún cambio. Vuelve a intentarlo cuando termine.",
	"Gentle profiles unavailable": "Perfiles de Gentle no disponibles",
	"Gentle did not answer, so the model profile \"{bound}\" cannot be applied. Switch only the account to {account}?": "Gentle no respondió, así que no se puede aplicar el perfil de modelo «{bound}». ¿Cambiar solo la cuenta a {account}?",
	"account {account} and model profile \"{bound}\"": "la cuenta {account} y el perfil de modelo «{bound}»",
	"account {account}": "la cuenta {account}",
	"Switch profile": "Cambiar de perfil",
	"Switch to {what}? The account changes for every Gentle Shell tab. Other providers stay signed in.": "¿Cambiar a {what}? La cuenta cambia en todas las pestañas de Gentle Shell. Los demás proveedores mantienen su sesión.",
	"Account switch failed.": "No se pudo cambiar de cuenta.",
	"Account {result.profile} was already active.": "La cuenta {result.profile} ya estaba activa.",
	"Account switched to {result.profile}.": "Se cambió a la cuenta {result.profile}.",
	"Previous unowned sign-in saved to {result.backup}.": "La sesión anterior, que no estaba asociada a una cuenta, se guardó en {result.backup}.",
	"Account switched to {result.profile}. Model profile \"{bound}\" was not applied: Gentle rejected the request.": "Se cambió a la cuenta {result.profile}. No se aplicó el perfil de modelo «{bound}»: Gentle rechazó la solicitud.",
	"Model profile \"{bound}\" applied to this session.": "Se aplicó el perfil de modelo «{bound}» a esta sesión.",
	"Model profile \"{bound}\" was not applied (cancelled).": "No se aplicó el perfil de modelo «{bound}» (se canceló la operación).",
	"Model profile \"{bound}\" no longer exists; relink it in /osdy-account.": "El perfil de modelo «{bound}» ya no existe. Vuelve a vincularlo en /osdy-account.",
	"Another profile apply is running; the model profile was not applied.": "Ya se está aplicando otro perfil. No se aplicó el perfil de modelo.",
	"Model profile \"{bound}\" was not applied.": "No se aplicó el perfil de modelo «{bound}».",
	"Unknown account: {requested}": "Cuenta desconocida: {requested}",
	"Switch Osdy profile (account + model)": "Cambiar de perfil Osdy (cuenta y modelo)",
	"Usage: /osdy-usage <name> | --all": "Uso: /osdy-usage <nombre> | --all",
	"Could not list saved accounts.": "No se pudieron listar las cuentas guardadas.",
	"Codex quota unavailable.": "Cuota de Codex no disponible.",
	"Accounts with unavailable Codex quota: {count}.": "Cuentas con cuota de Codex no disponible: {count}.",
	"Link model profile to an account": "Vincular un perfil de modelo a una cuenta",
	"Check account quota": "Consultar la cuota de una cuenta",
	"Use or check banked reset": "Usar o consultar un reinicio acumulado",
	"Save current sign-in as account": "Guardar la sesión actual como cuenta",
	"Add empty account (then sign in)": "Añadir una cuenta vacía (e iniciar sesión después)",
	"Rename account": "Cambiar el nombre de la cuenta",
	"Remove account": "Eliminar cuenta",
	"Import accounts from Osdy-Pi": "Importar cuentas de Osdy-Pi",
	"Osdy accounts": "Cuentas Osdy",
	"Account to link": "Cuenta que quieres vincular",
	"No saved Gentle model profiles found; create one in Configuration › Models › Profiles.": "No hay perfiles de modelo guardados en Gentle. Crea uno en Configuración › Modelos › Perfiles.",
	"Model profile for {account}": "Perfil de modelo para {account}",
	"(no model profile — account only)": "(sin perfil de modelo; solo la cuenta)",
	"{account} no longer changes the model profile.": "La cuenta {account} ya no cambia el perfil de modelo.",
	"{account} now applies model profile \"{choice}\".": "Ahora la cuenta {account} aplica el perfil de modelo «{choice}».",
	"Account quota": "Cuota de la cuenta",
	"Banked resets are unavailable in this Pi runtime.": "Los reinicios acumulados no están disponibles en este entorno de ejecución de Pi.",
	"Banked reset for the active Codex account": "Reinicio acumulado para la cuenta activa de Codex",
	"Cancel": "Cancelar",
	"Use a banked reset": "Usar un reinicio acumulado",
	"Check last reset attempt": "Consultar el último intento de reinicio",
	"Account name": "Nombre de la cuenta",
	"e.g. Osdy-Work": "p. ej., Osdy-Work",
	"Saved current sign-in as {saved} (now active).": "Se guardó la sesión actual como {saved} y quedó activa.",
	"Could not save the account.": "No se pudo guardar la cuenta.",
	"New account name": "Nombre de la cuenta nueva",
	"e.g. Osdy-Personal": "p. ej., Osdy-Personal",
	"Switch now?": "¿Cambiar ahora?",
	"Switch to {created} now? Only Codex will be signed out. Sign in again under Configuration › Models › Providers.": "¿Cambiar a {created} ahora? Solo se cerrará la sesión de Codex. Vuelve a iniciar sesión en Configuración › Modelos › Proveedores.",
	"Created {created}.": "Se creó {created}.",
	"Could not create the account.": "No se pudo crear la cuenta.",
	"Account to rename": "Cuenta cuyo nombre quieres cambiar",
	"New name for {account}": "Nuevo nombre para {account}",
	"Renamed {account} to {renamed}.": "Se cambió el nombre de {account} a {renamed}.",
	"Could not rename the account.": "No se pudo cambiar el nombre de la cuenta.",
	"Account to remove": "Cuenta que quieres eliminar",
	"Remove {account} and its stored sign-in? This cannot be undone.": "¿Eliminar {account} y su sesión guardada? No se puede deshacer.",
	"Removed {account}.": "Se eliminó {account}.",
	"Could not remove the account.": "No se pudo eliminar la cuenta.",
	"Import Osdy-Pi accounts": "Importar cuentas de Osdy-Pi",
	"Copy stored accounts from {source}/osdy-pi/profiles into Gentle Shell? Existing names are kept; Osdy-Pi is not modified.": "¿Copiar las cuentas guardadas de {source}/osdy-pi/profiles a Gentle Shell? Se conservan los nombres existentes y no se modifica Osdy-Pi.",
	"Already imported": "Ya se importaron las cuentas",
	"Osdy-Pi accounts were already imported. Import again (new names only)?": "Ya se importaron las cuentas de Osdy-Pi. ¿Volver a importar solo los nombres nuevos?",
	"Imported {count}: {names}.": "Cuentas importadas ({count}): {names}.",
	"none": "ninguna",
	"Skipped existing: {names}.": "Cuentas que ya existían: {names}.",
	"Failed: {names}.": "No se pudieron importar: {names}.",
	"Import failed.": "No se pudieron importar las cuentas.",

	// codex-reset
	"No banked reset attempt is saved for the active Codex account.": "No hay ningún intento de reinicio acumulado guardado para la cuenta activa de Codex.",
	"Banked reset action cancelled; no new reset was sent.": "Se canceló la operación. No se envió ningún reinicio nuevo.",
	"Banked reset action blocked. The local attempt store or account switch fence is unavailable; do not retry automatically.": "Se bloqueó la operación de reinicio. El registro local de intentos o el bloqueo de cambio de cuenta no está disponible. No vuelvas a intentarlo automáticamente.",
	"No currently available banked reset could be selected. Refresh usage to check availability.": "No se pudo seleccionar ningún reinicio acumulado disponible. Actualiza la información de uso para comprobar la disponibilidad.",
	"Outcome unknown; do not start another reset. Check pending reset only reads the local journal and cannot verify this attempt with Codex.": "No se conoce el resultado. No inicies otro reinicio. La consulta del reinicio pendiente solo lee el registro local y no puede verificar el intento con Codex.",
	"Use reset": "Usar reinicio",
	"expires {date} (local time)": "vence el {date} (hora local)",
	"Banked reset consumed; {windowsReset} rate-limit windows reset.": "Se consumió un reinicio acumulado y se restablecieron {windowsReset} ventanas de uso.",
	"No rate limits needed resetting; a banked reset was not consumed.": "No fue necesario restablecer los límites de uso. No se consumió ningún reinicio acumulado.",
	"No banked reset was available; a reset was not consumed.": "No había reinicios acumulados disponibles. No se consumió ningún reinicio.",
	"Codex reported that this banked reset was already redeemed.": "Codex indicó que este reinicio acumulado ya se había usado.",
	" Local attempt bookkeeping failed; do not start another reset before manual verification.": " No se pudo actualizar el registro local del intento. No inicies otro reinicio sin verificarlo manualmente.",
	"Reset {number} — {expiry}": "Reinicio {number} — {expiry}",
	"Choose a banked reset for the active Codex account": "Elige un reinicio acumulado para la cuenta activa de Codex",
	"For the active Codex account: Use one banked reset; one banked reset will be consumed if rate limits are reset.\nReset {expiry}.": "Para la cuenta activa de Codex: usa un reinicio acumulado. Se consumirá uno si se restablecen los límites de uso.\nEl reinicio {expiry}.",

	// index
	"Switch Osdy profile: Codex account (all tabs) + linked Gentle model profile": "Cambia de perfil Osdy: cuenta de Codex (todas las pestañas) y perfil de modelo de Gentle vinculado",
	"Manage Osdy Codex accounts: switch, link model profile, quota, banked resets, save, add, rename, remove, import": "Gestiona las cuentas de Codex de Osdy: cambia de cuenta, vincula perfiles de modelo, consulta cuotas, usa reinicios acumulados y guarda, añade, renombra, elimina o importa cuentas",
	"Report Pi auth owner compatibility without reading credentials or testing the lock": "Comprueba la compatibilidad con el gestor de autenticación de Pi sin leer credenciales ni probar el bloqueo",
	"View Codex quota without switching accounts: /osdy-usage <name> | --all [--run <id>]": "Consulta la cuota de Codex sin cambiar de cuenta: /osdy-usage <nombre> | --all [--run <id>]",

	// codex-usage
	"session": "sesión",
	"weekly": "semanal",
	"{days}d": "{days}d",
	"{hours}h": "{hours}h",
	"{minutes}m": "{minutes}m",
	"{percent}% left": "{percent}% disp.",
	"quota: no valid stored token": "cuota: sin token válido guardado",
	"quota: unavailable": "cuota: no disponible",
	"{count} banked reset": "{count} reinicio acumulado",
	"{count} banked resets": "{count} reinicios acumulados",
	"Codex usage {label} is invalid": "El campo {label} del uso de Codex no es válido",
	"Codex usage used_percent is invalid": "El campo used_percent del uso de Codex no es válido",
	"Codex usage does not contain any quota windows": "El uso de Codex no incluye ventanas de cuota",
	"Invalid token.": "Token no válido.",
	"Codex account ID is unavailable": "El ID de la cuenta de Codex no está disponible",
	"Expired token.": "El token caducó.",
	"Codex usage response is too large": "La respuesta de uso de Codex es demasiado grande",
	"Codex usage response is invalid": "La respuesta de uso de Codex no es válida",
	"Codex usage is temporarily unavailable": "La información de uso de Codex no está disponible por el momento",
	"Codex session expired; sign in again": "La sesión de Codex caducó. Vuelve a iniciar sesión",
	"Codex usage access is unavailable or forbidden": "El acceso a la información de uso de Codex no está disponible o no está permitido",

	// account-store
	"Profile names must use ASCII letters, numbers, and hyphens (not reserved names).": "Los nombres de perfil solo pueden usar letras ASCII, números y guiones. No se permiten nombres reservados.",
	"unknown error": "error desconocido",
	"Managed path is not a directory: {path}": "La ruta gestionada no es una carpeta: {path}",
	"Managed directory disappeared: {path}": "La carpeta gestionada ya no existe: {path}",
	"Account file is not a bounded regular file.": "El archivo de la cuenta no es un archivo normal dentro del límite de tamaño.",
	"Account file changed while reading it.": "El archivo de la cuenta cambió durante la lectura.",
	"Timed out waiting for account switch lock.": "Se agotó el tiempo de espera para el bloqueo de cambio de cuenta.",
	"Profile lookup is ambiguous due to a case-insensitive collision.": "No se puede identificar el perfil: los nombres coinciden si se ignoran las mayúsculas.",
	"Unknown account: {name}": "Cuenta desconocida: {name}",
	"An account named {safe} already exists.": "Ya existe una cuenta llamada {safe}.",
	"{label} is not a valid auth document.": "{label} no es un documento de autenticación válido.",
	"{label} contains invalid Codex credentials.": "{label} contiene credenciales de Codex no válidas.",
	"Updated auth document exceeds the size limit.": "El documento de autenticación actualizado supera el límite de tamaño.",
	"Current auth.json": "El archivo auth.json actual",
	"Incoming account auth.json": "El archivo auth.json de la cuenta entrante",
	"Outgoing account auth.json": "El archivo auth.json de la cuenta saliente",
	"There is no signed-in Codex account to save.": "No hay ninguna sesión de Codex iniciada para guardar.",
	"Account save failed and rollback could not be completed. Verify the account before retrying.": "No se pudo guardar la cuenta ni deshacer todos los cambios. Verifica la cuenta antes de volver a intentarlo.",
	"Account switch failed and rollback could not be completed. Verify the active account before retrying.": "No se pudo cambiar de cuenta ni deshacer todos los cambios. Verifica la cuenta activa antes de volver a intentarlo.",
	"Account switch failed and was rolled back: {detail}": "No se pudo cambiar de cuenta y se deshicieron los cambios: {detail}",
	"An account named {to} already exists.": "Ya existe una cuenta llamada {to}.",
	"Cannot remove the active account. Switch to another account first.": "No se puede eliminar la cuenta activa. Cambia primero a otra cuenta.",
	"Refusing to remove unexpected directory in account: {entry.name}": "No se eliminará una carpeta inesperada en la cuenta: {entry.name}",
	"No Osdy-Pi accounts found in {sourceRoot}": "No se encontraron cuentas de Osdy-Pi en {sourceRoot}",
	"Stored account credentials are unavailable.": "Las credenciales de la cuenta guardada no están disponibles.",

	// pi-auth-backend
	"Pi's auth lock is unavailable; no account changes were made.": "El bloqueo de autenticación de Pi no está disponible. No se hicieron cambios en la cuenta.",
	"Pi's auth lock failed after the account operation. Verify the active account before retrying.": "El bloqueo de autenticación de Pi falló después de la operación. Verifica la cuenta activa antes de volver a intentarlo.",
	"Pi's auth lock is busy (ELOCKED); no account changes were made.": "El bloqueo de autenticación de Pi está ocupado (ELOCKED). No se hicieron cambios en la cuenta.",
	"Pi's auth lock failed before the account operation; no account changes were made.": "El bloqueo de autenticación de Pi falló antes de la operación. No se hicieron cambios en la cuenta.",
	"Pi's auth lock returned without running the account operation; no account changes were made.": "El bloqueo de autenticación de Pi terminó sin ejecutar la operación. No se hicieron cambios en la cuenta.",

	// bindings
	"Invalid account name: {account}": "Nombre de cuenta no válido: {account}",

	// gentle-profiles
	"Gentle asked for another confirmation; nothing was applied.": "Gentle pidió otra confirmación. No se aplicaron cambios.",
} as const;

export type Key = keyof typeof es;
export type Vars = Record<string, string | number>;
export type Translate = (key: Key, vars?: Vars) => string;

// A command keeps the language it started in: every message it shows,
// including errors thrown deep in the account store, uses the locale read
// at entry, even if the app's language changes while a dialog is open.
const entryLocale = new AsyncLocalStorage<Locale>();

/** The locale of the running command, or the current one outside a command. */
export function currentLocale(): Locale {
	return entryLocale.getStore() ?? resolveLocale();
}

/** Run a command body with its locale fixed at entry (nested calls keep the outer one). */
export function inEntryLocale<T>(body: () => T): T {
	return entryLocale.getStore() === undefined ? entryLocale.run(resolveLocale(), body) : body();
}

export function translator(locale: Locale = currentLocale()): Translate {
	return (key, vars = {}) => (locale === "es" ? es[key] : key).replace(
		/\{([\w.]+)\}/g,
		(placeholder, name: string) => Object.hasOwn(vars, name) ? String(vars[name]) : placeholder,
	);
}

export function t(key: Key, vars?: Vars): string {
	return translator()(key, vars);
}
