# TODO für Agent: macOS-Fertigstellung

> **Stand:** Codebase ist implementiert, aber **noch nie auf macOS kompiliert oder getestet**.
>
> Entwicklung erfolgte auf Windows. Rust ist dort nicht installiert — `cargo test` und `cargo tauri build` wurden **nicht ausgeführt**.
>
> **Ziel:** App auf macOS zum Laufen bringen, manuell verifizieren, Compile-Fehler beheben, Abnahme-Kriterien aus `AGENT_BRIEF.md` erfüllen.

---

## Priorität 0 — Erster Build auf macOS

Diese Schritte zuerst. Erst wenn der Build grün ist, weiter mit manuellen Tests.

- [ ] **Rust-Toolchain installieren** (`rustup`) + Xcode CLI Tools (`xcode-select --install`)
- [ ] **Tauri-Voraussetzungen** installieren: https://v2.tauri.app/start/prerequisites/#macos
- [ ] **Dependencies holen und Core testen:**
  ```bash
  cd timetrack
  cargo test -p timetrack-core
  ```
- [ ] **Gesamten Workspace bauen:**
  ```bash
  cargo build --workspace
  ```
- [ ] **Compile-Fehler in `crates/monitor/` beheben** — besonders:
  - `crates/monitor/src/macos/workspace.rs` — `objc2` / `NSRunningApplication` APIs
  - `crates/monitor/src/macos/accessibility.rs` — `accessibility-sys`, `core-foundation`, `core-graphics`
  - Prüfen ob `NSRunningApplication::runningApplicationWithProcessIdentifier(pid)` korrekte Signatur hat (objc2 0.3 / 0.6)
  - Prüfen ob `workspace.runningApplications().iter()` korrekt ist
- [ ] **Tauri-App starten:**
  ```bash
  npm install
  npm run tauri dev
  ```

### Bekannte Risiko-Stellen (wahrscheinliche Compile-Probleme)

| Datei | Problem | Was tun |
|-------|---------|---------|
| `workspace.rs` | `objc2-app-kit` API-Signaturen ungetestet | Gegen aktuelle objc2-Docs anpassen |
| `workspace.rs` | `MainThreadMarker` evtl. nicht auf Tracker-Thread verfügbar | CGWindowList-Fallback ist primär; Workspace-Fallback ggf. entfernen oder auf Main Thread dispatch |
| `accessibility.rs` | `CFRelease` / Ownership bei AX-Elementen | Mit Clippy + manuellem Test prüfen, Leaks vermeiden |
| `accessibility.rs` | `CGEventType::from(0xFFFF_FFFF)` für Idle | Gegen `kCGAnyInputEventType` verifizieren |
| `src-tauri/src/lib.rs` | Tray + `default_window_icon()` | Echtes Icon setzen (aktuell 1×1 px Platzhalter) |
| `tauri.conf.json` | `macOSPrivateApi: true` | Nur behalten wenn wirklich nötig |

---

## Priorität 1 — Manuelle Funktionstests auf macOS

Checkliste aus `AGENT_BRIEF.md` §14. Jeden Punkt auf einem echten Mac durchspielen.

### App-Start & UI

- [ ] App startet ohne Crash
- [ ] Menüleisten-Icon (Tray) erscheint
- [ ] Linksklick öffnet/schließt Timeline-Fenster
- [ ] Tray-Menü: Timeline öffnen, Tracking pausieren, Beenden funktioniert
- [ ] UI ist auf Deutsch
- [ ] Timeline aktualisiert sich (auto-refresh alle 5 s)

### Berechtigungen

- [ ] Banner erscheint wenn Bedienungshilfen **nicht** erteilt
- [ ] Button „Berechtigung anfragen“ öffnet System-Dialog
- [ ] Button „Einstellungen öffnen“ öffnet Accessibility-Einstellungen
- [ ] Nach Erteilen verschwindet der Banner
- [ ] `get_tracker_status` liefert `accessibility_granted: true`

### Tracking-Grundfunktionen

- [ ] **App-Wechsel** werden in SQLite gespeichert (`~/Library/Application Support/timetrack/timeline.db`)
- [ ] **Fenstertitel** werden mit Bedienungshilfen erfasst
- [ ] **Dauer** pro Segment wird korrekt berechnet (nicht 0 Sekunden)
- [ ] **Heute / Gestern** Tabs zeigen korrekte Tagesdaten
- [ ] **Tracking pausieren** stoppt neue Segmente
- [ ] **Daten löschen** leert die Timeline

### Browser (jeweils einzeln testen)

- [ ] Google Chrome — aktive URL wird erfasst
- [ ] Safari — aktive URL wird erfasst
- [ ] Firefox — aktive URL wird erfasst
- [ ] Optional: Brave, Edge, Arc

Bei Fehlschlag: AX-Baum in `crates/monitor/src/macos/accessibility.rs` anpassen. Referenz: `mado`-Crate (https://crates.io/crates/mado) oder Apple Accessibility Docs.

### Zed

- [ ] Zed-Fenster mit Titel `datei.rs — projektname` öffnen
- [ ] Timeline zeigt: App `Zed`, Projekt + Datei im Untertitel
- [ ] Parser-Unit-Tests laufen: `cargo test -p timetrack-core zed`

### Terminal & Git

- [ ] Terminal.app / iTerm2 / Warp — Branch aus Fenstertitel (Fallback)
- [ ] Shell-Hook installieren → `~/.timetrack/hook.sh` in `.zshrc` sourcen
- [ ] Nach Hook: `cwd` + `git_branch` in Timeline sichtbar
- [ ] Parser-Unit-Tests: `cargo test -p timetrack-core terminal`

### Idle

- [ ] 5+ Minuten keine Eingabe → Segment mit `Idle` erscheint
- [ ] Idle-Zeit wird in Gesamtdauer mitgezählt (oder bewusst separat — aktuelles Verhalten dokumentieren)

---

## Priorität 2 — Bugs & Stabilität

Nach den ersten Tests wahrscheinlich nötig:

- [ ] **Offene Segmente beim App-Neustart** — Crash-Recovery prüfen (`db.rs` → `close_open_segments()`)
- [ ] **Sehr kurze App-Wechsel** (< 2 s) — Verhalten evaluieren, ggf. Mindestdauer-Filter
- [ ] **Tracker-Thread vs. Main Thread** — NSWorkspace-Aufrufe ggf. via `AppHandle::run_on_main_thread` ausführen
- [ ] **Speicher-Leaks** in Accessibility-Traversierung — mit Instruments prüfen
- [ ] **Fehler-Logging** — `tracing`-Output bei AX-Fehlern sichtbar machen
- [ ] **Platzhalter-Icon ersetzen** — echtes App-Icon erstellen:
  ```bash
  npm run tauri icon path/to/icon.png
  ```

---

## Priorität 3 — Fehlende MVP+-Features (aus AGENT_BRIEF)

Noch nicht implementiert, aber in der Spec als Should-Have:

- [ ] **App-Icons** in der Timeline (P2)
- [ ] **Suche/Filter** in der Timeline nach App, URL, Projekt (P2)
- [ ] **Idle-Timeout konfigurierbar** in der UI (aktuell hardcoded 300 s in `TrackerSettings`)
- [ ] **Einstellungen-Fenster** statt nur Tray-Menü

---

## Priorität 4 — Release-Vorbereitung

Erst relevant wenn P0–P2 erledigt:

- [ ] `cargo tauri build` → `.app` Bundle erzeugen
- [ ] App signieren (Apple Developer Account)
- [ ] Notarisierung für Gatekeeper
- [ ] **LaunchAgent** für Auto-Start beim Login (`~/Library/LaunchAgents/com.timetrack.app.plist`)
- [ ] Optional: GitHub Actions CI mit `macos-latest` Runner

---

## Priorität 5 — Roadmap (nicht blockierend)

Aus README / AGENT_BRIEF — bewusst später:

- [ ] Smart Grouping nach Domain
- [ ] CSV/JSON Export
- [ ] Ausschlussliste (Apps/URLs)
- [ ] Wöchentliche Zusammenfassung
- [ ] Windows/Linux Port

---

## Test-Befehle (Referenz)

```bash
# Unit-Tests (läuft auch ohne macOS-spezifische APIs)
cargo test -p timetrack-core

# Workspace-Build
cargo build --workspace

# Dev-Modus
npm run tauri dev

# Release
npm run tauri build

# DB manuell inspizieren
sqlite3 ~/Library/Application\ Support/timetrack/timeline.db "SELECT * FROM activities ORDER BY started_at DESC LIMIT 20;"
```

---

## Abnahme: Definition of Done

Alle Punkte aus `AGENT_BRIEF.md` §14 abhaken:

- [ ] App startet und erscheint in der Menüleiste
- [ ] Accessibility-Berechtigung wird angefragt und Status angezeigt
- [ ] App-Wechsel werden in SQLite gespeichert
- [ ] Fenstertitel werden erfasst (mit Accessibility)
- [ ] Aktive Browser-URL wird bei Chrome/Safari/Firefox erfasst
- [ ] Zed-Projektname wird aus Fenstertitel geparst
- [ ] Timeline zeigt den heutigen Tag chronologisch
- [ ] Idle-Zeit wird erkannt und markiert
- [ ] README erklärt Installation, Berechtigungen, Terminal-Hook
- [ ] **`cargo test -p timetrack-core` läuft grün** — Unit- + Integrationstests für Parser, DB, Segmente
- [ ] **`cargo test -p timetrack-monitor` läuft grün** — plattformabhängige Stub-Tests

---

## Hinweise für den Agent

1. **Nicht over-engineeren** — erst bauen, testen, dann gezielt fixen
2. **Privacy beibehalten** — alles lokal, keine Telemetrie
3. **UI-Sprache: Deutsch**, Code-Kommentare: Englisch
4. **Keine Commits** erstellen, es sei denn der User fragt explizit danach
5. Bei hartnäckigen AX-Problemen: [`mado`](https://crates.io/crates/mado)-Crate als Referenz oder Dependency erwägen
6. Vollständige Spec in [`AGENT_BRIEF.md`](AGENT_BRIEF.md)

---

*Erstellt: 2026-05-28 — Basierend auf MVP-Implementierung ohne macOS-Verifikation.*
