# osdy-gentle

Osdy-Pi's Codex accounts and profile switching for **Gentle Shell desktop**, as a lightweight Pi extension.

It keeps the parts of [Osdy-Pi](https://github.com/OsdyOrtiz/Osdy-Pi) that work in the desktop app and leaves out themes, mascots, headers, the Companion workspace and every terminal-only UI. Every screen uses the dialogs Gentle Shell renders natively (select, confirm, input, notify).

| Osdy-Pi feature | Here |
| --- | --- |
| `/osdy-account` Codex account snapshots | `/osdy-account` (switch, link model profile, quota, save, add, rename, remove, import) |
| Profile selection | `/osdy-profile` = **Codex account + Gentle model profile** together |
| Stored-account quota preview | `/osdy-usage <name>` updates the desktop account picker without switching accounts. Also shown in the `/osdy-profile` picker and in *Check account quota*. |
| Banked Codex resets | `/osdy-account` → **Use or check banked reset** for the active Codex account |
| Sound alerts | Gentle Shell's native **Configuration › Notifications** with Osdy's sound ([below](#sound-alerts)) |

## Install in Gentle Shell

1. Open **Configuration › Extensions**.
2. Add the local path of this folder, for example `/Users/osdy/Documents/GitHub/IngracionOsdy-Pi`.
3. Open a new agent tab (or restart the current one). `/osdy-profile`, `/osdy-account`, `/osdy-usage`, and `/osdy-auth-status` appear in the command list.

Requirements: Gentle Shell with gentle-pi 5.0.0 or later (for applying model profiles) and Node.js 22.19 or later. Changing or saving the active Codex sign-in and redeeming reset credits require the session-owned Pi file credential store. The adapter recognizes the Pi 0.99.1 source build and the Pi 1.6.0, 1.6.1, and 1.7.0 CLI bundles. Support is exact: other versions and storage modes leave account changes and resets unavailable instead of writing without Pi's lock.

## First run: import your Osdy-Pi accounts

Run `/osdy-account` → **Import accounts from Osdy-Pi**.

- It copies every account from `~/.pi/agent/osdy-pi/profiles/<name>/auth.json` (or `$OSDY_PI_SHARED_AGENT_DIR`) into Gentle Shell's agent dir.
- Osdy-Pi is never modified. After the import the two stores are independent.
- Names that already exist in Gentle Shell are skipped. A second import asks first and only adds new names.

If you were already signed in to Codex in Gentle Shell, run **Save current sign-in as account** to give that Codex sign-in a name. Otherwise the first switch backs up the full current sign-in file, including any other providers, to `osdy-gentle/backups/` instead of assigning it to an account.

## Profiles: account + model

An Osdy profile is a Codex account with an optional linked **Gentle model profile** (the named routes in Configuration › Models › Profiles).

1. `/osdy-account` → **Link model profile to an account** → pick the account → pick a Gentle profile, or *no model profile* to switch only the account.
2. `/osdy-profile` lists the accounts with their linked model and remaining Codex quota. You can also run `/osdy-profile <name>` directly; names autocomplete.

**From the editor:** Open the Command Palette (`⌘K` on macOS, `Ctrl+K` elsewhere) and choose `osdy-profile`. This keeps your draft and attachments. The chip above the editor shows the active Codex account; click it to read the full name and shortcut, not to switch. Very narrow panes may hide the chip.

Selecting a profile:

1. Asks for confirmation.
2. Waits until the agent is idle and checks again after acquiring the switch lock. If the agent is busy, nothing is switched.
3. Saves the departing profile's latest Codex sign-in and replaces only the `openai-codex` entry under Pi's credential lock. Claude and other providers keep their live sign-ins. Legacy whole-file snapshots are still supported; their other providers are not restored.
4. Applies the linked model profile **to this session**, through Gentle's own "Use in this session" action. If Gentle asks for a confirmation, you see its exact message.

> **The account is global.** All Gentle Shell tabs share one `auth.json`, so switching affects every tab from its next request. The model profile applies only to the tab where you ran the command.

The two steps are not one transaction. If the model profile cannot be applied (for example, Gentle rejects the request, the profile was removed, another apply is running, or you cancelled), the account switch is kept and the notification says so.

### Adding a new account

`/osdy-account` → **Add empty account (then sign in)**. Switching to an empty profile signs out of Codex only. Sign in to Codex from **Configuration › Models › Providers**; the next switch saves that sign-in to the profile.

If a previous version signed you out of Claude, sign in to Claude once from **Providers**. The extension does not restore older Claude credentials.

## Diagnose a Desktop auth lock error

In the affected Desktop tab, run `/osdy-auth-status` (or pick `osdy-auth-status` from the Command Palette). Copy the notification line beginning with `osdy-auth/v1`, for example:

```text
osdy-auth/v1 sdk=1.6.0 stage=verified storePath=match backendPath=match
```

The line is safe to paste: it contains only a sanitized SDK version, a check stage, and lexical path match results, not paths, credentials, or raw errors. You do not need to switch accounts. The command does not access account files or directories, call providers, or attempt a lock. `verified` means the credential owner is recognized, not that Pi's lock can be acquired. If a real switch reports `ELOCKED`, Pi reported the lock busy before the switch ran. Other pre-switch lock errors do not establish that the lock is busy; if a failure is reported after the operation, verify the active account before retrying.

## Banked Codex resets

1. Open `/osdy-account` → **Use or check banked reset** → **Use a banked reset**.
2. Choose an available credit by its expiry time and confirm **Use reset**. This acts on the Codex account currently signed in, not on a stored account preview.
3. If the outcome is uncertain, use **Check last reset attempt**. It reads the local attempt record only: it never redeems a credit or asks Codex to repeat a request. An unknown outcome stays unknown and blocks further redemption for that account.

After a crash, a leftover reset lease can block **Use a banked reset** even when **Check last reset attempt** shows no saved attempt. Do not retry automatically or delete a pending journal. Before any manual recovery, stop all processes sharing the Pi agent directory and verify whether the lease is stale and whether a reset may have been sent.

## Sound alerts

Gentle Shell plays sounds natively. **Configuration › Notifications** only has On/Off switches; turn on the alerts you want there, then edit the config to choose their sounds:

1. Make a backup copy of `~/.gentle-shell/gentle-ai/notifications.json`.
2. Edit `audio.events` in that file, keeping the other settings. Each custom sound value starts with `file:` followed by an absolute path. For example, to use the bundled `assets/osdy-notification.wav` (replace `/absolute/path/to/osdy-gentle` with the path where you installed this extension):

   ```json
   {
     "audio": {
       "events": {
         "agent.completed": "file:/absolute/path/to/osdy-gentle/assets/osdy-notification.wav",
         "agent.attention": "file:/absolute/path/to/osdy-gentle/assets/osdy-notification.wav",
         "agent.failed": "file:/absolute/path/to/osdy-gentle/assets/osdy-notification.wav",
         "subagent.completed": "file:/absolute/path/to/osdy-gentle/assets/osdy-notification.wav",
         "context.high": "file:/absolute/path/to/osdy-gentle/assets/osdy-notification.wav"
       }
     }
   }
   ```

   The desktop app plays only `agent.completed`, `agent.attention` and `context.high`. Failure and subagent events (`agent.failed`, `subagent.*`) are terminal (TUI) settings: the desktop does not play them, even if you set a sound.

## Files

Everything is stored under the Pi agent dir (Gentle Shell: `~/.gentle-shell/agent`, from `PI_CODING_AGENT_DIR`):

```
auth.json                                 active sign-in (read by Pi)
osdy-gentle/profiles/<name>/auth.json     one snapshot per account (0600)
osdy-gentle/active-account.json           { "version": 1, "profile": "<name>" }
osdy-gentle/bindings.json                 account → Gentle model profile
osdy-gentle/backups/                      original sign-in file backed up before a first switch
osdy-gentle/imported-from-osdy.json       import marker
osdy-gentle/codex-reset-attempts/         account-scoped reset journal (no credentials)
```

If an account-switch lock remains after a crash, account changes stop rather than deleting a lock that another process could hold. Stop all processes sharing the agent directory and verify the stale lock before any manual recovery.

Quota previews use each account's **stored** access token and send it only to `chatgpt.com/backend-api/wham/usage`. They never refresh tokens and never fall back to another account. Expired tokens show *quota: no valid stored token* until that account is used again.

## Development

```bash
npm test   # node:test, TypeScript run natively
```

Load it in a scratch Pi without touching your real agent dir:

```bash
PI_CODING_AGENT_DIR=$(mktemp -d) pi -e ./src/index.ts
```
