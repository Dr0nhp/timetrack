# Agent Brief: macOS Time Tracker (MemTime / TimeBro Style)

> **Ziel:** Dieses Dokument enthält alle Anweisungen, Recherche-Ergebnisse und Umsetzungsideen, damit ein Agent die App vollständig implementieren kann.
>
> **Plattform (Phase 1):** macOS only  
> **Später optional:** Windows, Linux  
> **Workspace:** `timetrack/` (aktuell leer — Greenfield)

---

## 1. Produktvision

Baue einen **automatischen Time Tracker** für macOS, ähnlich wie **MemTime**, **TimeBro** oder **ManicTime**.

Der Nutzer soll **nichts manuell starten/stoppen** müssen. Eine Hintergrund-App erfasst passiv:

- Welche **App** gerade im Vordergrund ist
- **Fenstertitel** (Dateiname, Projekt, etc.)
- Im **Browser**: welcher **Tab aktiv** ist (URL + Titel)
- Bei **Zed**: das **aktuelle Projekt** (aus Fenstertitel)
- Im **Terminal**: optional der **Git-Branch** und Arbeitsverzeichnis

Alles wird **lokal** in einer Timeline gespeichert (Privacy-first, offline). Eine UI zeigt den Tagesverlauf und erlaubt später die Zuordnung zu Projekten/Zeiteinträgen.

---

## 2. Kern-Features (MVP)

### Must-Have

| Feature | Beschreibung | Priorität |
|---------|--------------|-----------|
| App-Tracking | Aktive App erkennen (Wechsel im Vordergrund) | P0 |
| Fenstertitel | Titel des fokussierten Fensters mit erfassen | P0 |
| Dauer-Berechnung | Zeit pro Activity-Segment aggregieren (Sekunden/Minuten) | P0 |
| Browser-Tab (aktiv) | URL + Tab-Titel des **aktiven** Browser-Tabs | P0 |
| Lokale Speicherung | SQLite, keine Cloud | P0 |
| Timeline-UI | Chronologische Ansicht des Tages | P0 |
| Hintergrund-Betrieb | Menüleisten-App oder unsichtbarer Daemon + UI | P0 |
| macOS-Berechtigungen | Accessibility-Anfrage + Erklärung in UI | P0 |

### Should-Have (MVP+)

| Feature | Beschreibung | Priorität |
|---------|--------------|-----------|
| Zed-Projekt-Parsing | Projektname aus Zed-Fenstertitel extrahieren | P1 |
| Terminal-Git-Branch | Branch aus Terminal-Fenstertitel oder Shell-Hook | P1 |
| Idle-Erkennung | Keine Zeit zählen bei Inaktivität (z.B. >5 Min) | P1 |
| App-Icons | Icon der aktiven App in der Timeline | P2 |
| Suche/Filter | Timeline nach App, URL, Projekt durchsuchen | P2 |

### Nice-to-Have (später)

- Alle offenen Browser-Tabs (nicht nur aktiver) — deutlich schwieriger
- Projekt-Zuordnung / Zeiteinträge exportieren (CSV, JSON)
- Ausschlussliste (Apps/URLs blockieren)
- Windows/Linux Port
- Zed-Plugin oder offizielle Integration (falls Zed API bietet)
- ManicTime-Style Git-Repo-Erkennung aus IDE-Pfaden

---

## 3. Was technisch machbar ist (Recherche-Ergebnisse)

### ✅ Zuverlässig machbar

- **Aktive App:** macOS `NSWorkspace` (`didActivateApplicationNotification`)
- **Fenstertitel:** Accessibility API (`AXUIElement`, `kAXTitleAttribute`)
- **Fenster-Fokus-Wechsel:** `AXObserver` + `kAXFocusedWindowChangedNotification`
- **Aktiver Browser-Tab (URL):** Accessibility API — UI-Baum des Browsers traversieren
- **Zed-Projekt:** Aus Fenstertitel parsen (siehe Abschnitt 5)
- **Lokale Timeline:** SQLite

### ⚠️ Machbar, aber fragil

- **Browser-URL-Extraktion:** Browser-Updates können AX-Pfade brechen → Wartung nötig
- **Git-Branch aus Terminal:** Heuristisch (Titel parsen) oder Shell-Hook (zuverlässiger)
- **Alle Tabs auf einmal:** Braucht Browser-Extensions oder AppleScript/Automation — nicht für MVP

### ❌ Nicht trivial / nicht für MVP

- Wayland/Linux (User will erst macOS)
- Sandbox-kompatibles Cross-Process-Tracking (App Sandbox blockiert AX für andere Prozesse)
- Offizielle Zed-API für Time-Tracking (existiert nicht)

---

## 4. Architektur (Empfehlung)

```
┌─────────────────────────────────────────────────────────┐
│  Menüleisten-App / Tauri UI                             │
│  - Timeline anzeigen                                    │
│  - Berechtigungen erklären                              │
│  - Einstellungen (Idle-Timeout, Ausschlüsse)            │
└───────────────────────┬─────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────┐
│  Tracker Service (Rust)                                 │
│  - Poll/Event-basiert: alle 1–2 Sekunden Snapshot       │
│  - Erkennt App-Wechsel, erzeugt Activity-Segmente       │
│  - Ruft Context-Parser auf (Zed, Terminal, Browser)     │
└───────────────────────┬─────────────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
  macOS Native     Context Parser    SQLite DB
  (Swift/ObjC      (Rust)            (timeline.db)
   oder Rust
   via objc2)
```

### Empfohlener Tech-Stack

| Layer | Empfehlung | Alternative |
|-------|------------|-------------|
| UI | **Tauri 2** (Rust + Web) | SwiftUI native |
| Core/Storage | **Rust** + `rusqlite` | — |
| macOS APIs | **Swift via swift-rs** oder **objc2** | Crate `mado` (fertige macOS-Library) |
| Background | Tauri System Tray + Rust-Thread | LaunchAgent (später) |

**Warum Tauri + Rust:** Gute Balance aus nativem Backend (macOS-APIs) und schneller UI-Entwicklung. Später Windows-Port einfacher als reines SwiftUI.

**Warum `mado` erwägen:** Fertige Rust-Library für macOS Window/Browser-Tracking via Accessibility API. Unterstützt Chrome, Safari, Brave, Edge, Arc, Opera, Firefox.  
→ https://crates.io/crates/mado

---

## 5. Kontext-Erkennung (Parsing-Regeln)

### 5.1 Zed Editor

Zed-Fenstertitel folgen typisch diesem Muster:

```
{filename} — {project}
```

Beispiele:
- `channel.rs — app`
- `main.rs — timetrack`

**Parser-Logik (Rust):**
```rust
// Pattern: "datei — projekt" (Em-Dash oder normaler Dash)
// Regex: ^(.+?)\s*[—–-]\s*(.+?)$
// Ergebnis: { file: "channel.rs", project: "timetrack" }
```

Zed zeigt den **Basename des Worktrees** als Projektname. Custom Project Names sind in Zed-Settings möglich (`Project Name` in Project Settings).

**Fallback:** Wenn Parsing fehlschlägt → nur `window_title` speichern.

Referenz: Zed Title Bar — `effective_active_worktree()` in `crates/title_bar/src/title_bar.rs`

### 5.2 Terminal + Git Branch

**Strategie A — Fenstertitel parsen (Zero-Setup, fragil):**

Viele Shells zeigen Branch im Prompt, der im Terminal-Fenstertitel landet:
- iTerm2: `{user}@{host}:{cwd} (branch)` oder ähnlich
- Warp, Terminal.app: variiert je nach Shell-Konfiguration
- zsh + starship/powerlevel10k: oft Branch sichtbar

```rust
// Regex-Beispiele:
// \(([^)]+)\)$           → Branch in Klammern am Ende
// \[([^\]]+)\]           → Branch in eckigen Klammern
// git:(\S+)               → git:branch Pattern (starship)
```

**Strategie B — Shell-Hook (zuverlässig, braucht User-Setup):**

Kleines Script, das der User in `.zshrc` einbindet:

```bash
# ~/.timetrack/hook.sh
_timetrack_hook() {
  local cwd branch
  cwd=$(pwd)
  branch=$(git branch --show-current 2>/dev/null)
  echo "{\"cwd\":\"$cwd\",\"branch\":\"$branch\",\"ts\":$(date +%s)}" \
    >> ~/.timetrack/terminal-state.jsonl
}
precmd_functions+=(_timetrack_hook)  # zsh
# oder PROMPT_COMMAND für bash
```

Der Tracker liest die letzte Zeile aus `~/.timetrack/terminal-state.jsonl` wenn Terminal.app/iTerm/Warp aktiv ist.

**Empfehlung:** Beide Strategien implementieren — Hook hat Priorität, Titel-Parsing als Fallback.

**Terminal-Apps erkennen:**
- `com.apple.Terminal`
- `com.googlecode.iterm2`
- `dev.warp.Warp-Stable`
- `net.kovidgoyal.kitty`

### 5.3 Browser

**Nur aktiver Tab** (MVP):

| Browser | AX-Strategie |
|---------|--------------|
| Chrome, Brave, Edge, Arc, Opera | `AXTextField` via `AXDOMIdentifier` oder Placeholder |
| Safari | `AXURL` von `AXWebArea` Element |
| Firefox | `AXTextField` mit "address" in Description |

`mado` implementiert das bereits — nutzen oder als Referenz.

**Ergebnis pro Browser-Snapshot:**
```json
{
  "url": "https://github.com/user/repo/pull/42",
  "title": "Pull Request #42",
  "browser": "Google Chrome"
}
```

---

## 6. Datenmodell

### Activity (ein Segment in der Timeline)

```rust
struct Activity {
    id: i64,
    started_at: DateTime<Utc>,      // Segment-Start
    ended_at: Option<DateTime<Utc>>, // null = noch aktiv
    duration_secs: i64,

    // Basis
    app_name: String,               // "Zed", "Google Chrome"
    app_bundle_id: String,          // "dev.zed.Zed"
    window_title: String,

    // Kontext (optional, je nach App)
    context: ActivityContext,
}

struct ActivityContext {
    // Browser
    url: Option<String>,
    page_title: Option<String>,

    // Zed / IDE
    project: Option<String>,
    file: Option<String>,

    // Terminal
    cwd: Option<String>,
    git_branch: Option<String>,
}
```

### SQLite Schema

```sql
CREATE TABLE activities (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at      TEXT NOT NULL,          -- ISO 8601
    ended_at        TEXT,
    duration_secs   INTEGER NOT NULL DEFAULT 0,
    app_name        TEXT NOT NULL,
    app_bundle_id   TEXT NOT NULL,
    window_title    TEXT NOT NULL DEFAULT '',
    url             TEXT,
    page_title      TEXT,
    project         TEXT,
    file            TEXT,
    cwd             TEXT,
    git_branch      TEXT,
    is_idle         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_activities_started_at ON activities(started_at);
CREATE INDEX idx_activities_app ON activities(app_name);
```

### Segment-Logik

```
Bei jedem Poll (1–2s):
  snapshot = get_current_activity()

  if snapshot != last_snapshot:
    - schließe aktuelles Segment (ended_at = now, duration berechnen)
    - starte neues Segment mit snapshot

  if idle_detected (keine Input > threshold):
    - markiere Segment als is_idle = 1
    - oder schließe Segment und starte "Idle"-Segment
```

**Deduplizierung:** Gleiche App + gleicher Titel + gleiche URL → Segment verlängern, kein neues anlegen.

---

## 7. macOS-Berechtigungen

### Erforderlich

| Berechtigung | Warum | System Settings Pfad |
|--------------|-------|----------------------|
| **Bedienungshilfen (Accessibility)** | Fenstertitel, Browser-URL, Window-Focus | Systemeinstellungen → Datenschutz → Bedienungshilfen |

### Optional / nicht für MVP

| Berechtigung | Warum |
|--------------|-------|
| Automation (AppleScript) | Alternative zu AX für Browser — nicht nötig wenn AX funktioniert |
| Bildschirmaufnahme | Nur wenn Screenshot-OCR gewünscht — nicht MVP |

### UX für Berechtigungen

1. Beim ersten Start: Modal erklärt, **warum** Accessibility nötig ist
2. Button „Bedienungshilfen öffnen“ → `open x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`
3. Status-Check: Periodisch prüfen ob Berechtigung erteilt (`AXIsProcessTrusted()`)
4. Ohne Berechtigung: Nur App-Name tracken (via NSWorkspace), kein Fenstertitel/URL

### Info.plist

```xml
<key>NSAppleEventsUsageDescription</key>
<string>TimeTrack needs accessibility access to record which apps and browser tabs you use.</string>
```

---

## 8. Projektstruktur (Vorschlag)

```
timetrack/
├── Cargo.toml                    # Workspace
├── README.md
├── AGENT_BRIEF.md                # Dieses Dokument
│
├── crates/
│   ├── core/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── models.rs         # Activity, ActivityContext
│   │   │   ├── db.rs             # SQLite CRUD
│   │   │   ├── segment.rs        # Segment open/close/merge logic
│   │   │   └── parser/
│   │   │       ├── mod.rs
│   │   │       ├── zed.rs        # Zed title parser
│   │   │       ├── terminal.rs   # Git branch / cwd parser
│   │   │       └── browser.rs    # URL normalization
│   │   └── Cargo.toml
│   │
│   └── monitor/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── macos/
│       │   │   ├── mod.rs
│       │   │   ├── workspace.rs  # NSWorkspace app activation
│       │   │   ├── accessibility.rs  # AX window title + browser URL
│       │   │   └── idle.rs       # Idle detection (CGEventSourceSecondsSinceLastEventType)
│       │   └── snapshot.rs       # Unified ActivitySnapshot
│       └── Cargo.toml
│
├── src-tauri/                    # Tauri 2 App
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs           # Tauri IPC commands
│   │   └── tray.rs               # Menüleisten-Icon
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── Cargo.toml
│
└── ui/                           # Frontend (React/Svelte/Vanilla)
    ├── index.html
    ├── src/
    │   ├── App.tsx
    │   ├── Timeline.tsx          # Hauptansicht
    │   ├── ActivityRow.tsx
    │   └── PermissionsBanner.tsx
    └── package.json
```

---

## 9. UI-Konzept (MVP)

### Hauptansicht: Timeline

```
┌──────────────────────────────────────────────────────┐
│  TimeTrack                              ⚙️  ─  ✕    │
├──────────────────────────────────────────────────────┤
│  Heute, 28. Mai 2026                    Gesamt: 6h  │
│                                                      │
│  🔴 Bedienungshilfen nicht erteilt — [Jetzt aktivieren] │
│                                                      │
│  ┌─ 09:00 ─────────────────────────────────────────┐ │
│  │ 🟦 Zed · timetrack · main.rs          45 Min   │ │
│  │ 🌐 Chrome · github.com/user/repo      12 Min   │ │
│  │ ⬛ Terminal · feature/auth (main)      8 Min   │ │
│  │ 🟦 Zed · timetrack · lib.rs           1h 20m   │ │
│  │ 🌐 Safari · docs.rs/trait.Iterator    23 Min   │ │
│  │ 💤 Idle                                15 Min   │ │
│  └─────────────────────────────────────────────────┘ │
│                                                      │
│  [Heute] [Gestern] [Diese Woche]                     │
└──────────────────────────────────────────────────────┘
```

### Menüleisten-Icon

- Icon in der Menu Bar (Tray)
- Klick → Timeline-Fenster öffnen/schließen
- Kontextmenü: Pause, Einstellungen, Beenden

### Einstellungen (minimal)

- Idle-Timeout (Minuten, Default: 5)
- Tracking pausieren
- Daten löschen
- Terminal-Hook installieren (kopiert Script + Anleitung)

---

## 10. Implementierungs-Reihenfolge

Der Agent soll in dieser Reihenfolge vorgehen:

### Phase 1 — Fundament (Tag 1)
1. Cargo Workspace + `core` Crate mit Models + SQLite
2. Unit Tests für Zed-Parser, Terminal-Parser, Segment-Logik
3. `monitor` Crate: macOS Snapshot (App-Name via NSWorkspace)

### Phase 2 — Tracking (Tag 2)
4. Accessibility: Fenstertitel auslesen
5. Browser-URL-Extraktion (via `mado` oder eigene AX-Implementierung)
6. Segment-Service: Poll-Loop, open/close/merge
7. Idle-Erkennung

### Phase 3 — UI (Tag 3)
8. Tauri 2 App scaffolden
9. System Tray
10. Timeline-View (Activities aus DB laden)
11. Permissions-Banner

### Phase 4 — Polish (Tag 4)
12. Zed + Terminal Context-Parser einbinden
13. Terminal Shell-Hook (optional installieren)
14. README mit Setup-Anleitung
15. `cargo test` + manueller Test auf macOS

---

## 11. Wichtige Code-Snippets & APIs

### macOS: Aktive App (Rust via objc2)

```rust
// NSWorkspace.shared.frontmostApplication
// → localizedName, bundleIdentifier
```

### macOS: Accessibility-Berechtigung prüfen

```rust
// AXIsProcessTrusted() → bool
// AXIsProcessTrustedWithOptions({ kAXTrustedCheckOptionPrompt: true })
```

### macOS: Idle-Erkennung

```rust
// CGEventSourceSecondsSinceLastEventType(
//   kCGEventSourceStateCombinedSessionState,
//   kCGAnyInputEventType
// )
// → Sekunden seit letztem Input
```

### mado Integration (Alternative zu eigener AX-Implementierung)

```rust
use mado::{Monitor, MonitorConfig, Event};

let config = MonitorConfig {
    track_window_changes: true,
    include_browser_info: true,
    ..Default::default()
};

let monitor = Monitor::new(config);
monitor.on_event(|event| {
    match event {
        Event::AppActivated { app, .. } => { /* ... */ }
        Event::WindowChanged { app, window, browser, .. } => { /* ... */ }
    }
});
monitor.start();
```

---

## 12. Edge Cases & Hinweise

| Case | Verhalten |
|------|-----------|
| App-Wechsel < 2 Sekunden | Trotzdem erfassen, aber ggf. Mindestdauer-Filter (optional, z.B. <3s ignorieren) |
| Gleicher Titel, andere URL | Neues Segment (URL hat Priorität) |
| Browser ohne URL (AX fail) | Nur App-Name + Fenstertitel speichern |
| Zed mit mehreren Worktrees | Projekt aus Titel parsen; reicht für MVP |
| Terminal ohne Git-Repo | `git_branch = null`, nur `cwd` wenn Hook aktiv |
| App-Neustart | Offenes Segment mit `ended_at = null` beim Start schließen (Crash-Recovery) |
| Mehrere Displays | Irrelevant — es zählt nur das fokussierte Fenster |
| App im App Nap | Weiter tracken solange fokussiert |

---

## 13. Ideen für den Agent (Bonus)

Diese Ideen sind **nicht MVP**, aber gute Erweiterungen — der Agent kann sie als `// TODO` markieren oder in README unter „Roadmap“ listen:

1. **Smart Grouping:** Aufeinanderfolgende Chrome-Segmente mit gleicher Domain zusammenfassen (`github.com` statt voller URL)
2. **Projekt-Regeln:** User-definierte Regeln: „Wenn Zed-Projekt = timetrack → Projekt 'TimeTrack'“
3. **Favicon-Fetch:** Favicons für URLs in der Timeline (Cache lokal)
4. **Export:** CSV/JSON Export für Abrechnung
5. **LaunchAgent:** Auto-Start beim Login (`~/Library/LaunchAgents/com.timetrack.app.plist`)
6. **Native SwiftUI-Alternative:** Falls Tauri zu schwer — minimale SwiftUI-App nur für macOS
7. **Browser-Extension:** Fallback für Tab-Tracking wenn AX bricht (Chrome MV3 Extension → Native Messaging)
8. **Git-Branch aus IDE:** Wenn Zed-Fenster offen → `git branch` im Projekt-Pfad ausführen ( langsamer, aber genau)
9. **Weekly Summary:** „Diese Woche: 12h Zed, 3h Chrome, 1h Terminal“
10. **Exclude-Liste:** Apps wie 1Password, Slack privat never tracken

---

## 14. Abnahme-Kriterien (Definition of Done)

Der Agent ist fertig, wenn auf **macOS**:

- [ ] App startet und erscheint in der Menüleiste
- [ ] Accessibility-Berechtigung wird angefragt und Status angezeigt
- [ ] App-Wechsel werden in SQLite gespeichert
- [ ] Fenstertitel werden erfasst (mit Accessibility)
- [ ] Aktive Browser-URL wird bei Chrome/Safari/Firefox erfasst
- [ ] Zed-Projektname wird aus Fenstertitel geparst
- [ ] Timeline zeigt den heutigen Tag chronologisch
- [ ] Idle-Zeit wird erkannt und markiert
- [ ] README erklärt Installation, Berechtigungen, Terminal-Hook
- [ ] `cargo test` läuft grün (Parser + Segment-Logik)

---

## 15. Referenzen

| Ressource | URL |
|-----------|-----|
| mado (Rust macOS tracker lib) | https://crates.io/crates/mado |
| Tauri 2 Docs | https://v2.tauri.app/ |
| macOS Accessibility API | https://developer.apple.com/documentation/applicationservices/accessibility |
| Zed Window Title Issue | https://github.com/zed-industries/zed/issues/14534 |
| Zed Title Bar Source | https://github.com/zed-industries/zed/blob/main/crates/title_bar/src/title_bar.rs |
| MemTime (Referenz-Produkt) | https://www.memtime.com/ |
| ManicTime Git Branch Feature | https://www.manictime.com/features/specialized-automatic-tracking |
| StackOverflow: Active Window macOS | https://stackoverflow.com/questions/53186576 |
| StackOverflow: Browser URL via AX | https://stackoverflow.com/questions/53229924 |

---

## 16. Hinweis für den Agent

- **Entwicklungsumgebung des Users:** Windows — Code muss auf **macOS gebaut und getestet** werden. CI optional.
- **Kein Over-Engineering:** MVP first. Lieber 80% Features stabil als 100% fragil.
- **Privacy:** Alles lokal. Keine Telemetrie, kein Netzwerk (außer optional Favicon).
- **Keine Commits** erstellen, es sei denn der User fragt explizit danach.
- **Sprache UI:** Deutsch bevorzugt (User ist deutschsprachig), Code/Kommentare auf Englisch.

---

*Erstellt: 2026-05-28 — Basis: Konversation + Recherche zu MemTime/TimeBro/ManicTime, macOS AX API, Zed Title Bar, Browser-Tab-Tracking.*
