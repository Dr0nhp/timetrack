# TimeTrack

**TimeTrack** ist ein automatischer Time Tracker für macOS — ähnlich wie MemTime, TimeBro oder ManicTime.

Die App läuft im Hintergrund und erfasst passiv, womit du arbeitest. Du musst keine Timer starten oder stoppen. Am Ende des Tages (oder jederzeit) siehst du eine chronologische Timeline deiner Aktivitäten.

Alle Daten bleiben **lokal auf deinem Mac**. Keine Cloud, kein Tracking durch Dritte.

---

## Was macht TimeTrack?

TimeTrack beantwortet die Frage: *„Wofür habe ich heute meine Zeit am Rechner verbracht?“*

Dazu beobachtet ein Hintergrund-Dienst alle **1,5 Sekunden**, welche App gerade im Vordergrund ist. Wenn sich etwas Relevantes ändert (andere App, anderer Fenstertitel, andere Browser-URL), wird ein neues **Aktivitäts-Segment** in einer lokalen Datenbank gespeichert.

### Was wird erfasst?

| Kontext | Was TimeTrack speichert | Beispiel |
|---------|-------------------------|----------|
| **Beliebige App** | App-Name, Fenstertitel, Dauer | `Slack · #general` — 12 Min |
| **Browser** | Aktiver Tab: URL + Seitentitel | `Chrome · github.com/user/repo` |
| **Zed Editor** | Projekt + Datei aus Fenstertitel | `Zed · timetrack · main.rs` |
| **Terminal** | Git-Branch (+ optional Pfad via Hook) | `Terminal · feature/auth` |
| **Inaktivität** | Idle-Zeit (Standard: ab 5 Min) | `Idle` — 15 Min |

### Was wird *nicht* erfasst?

- Keine Screenshots oder Tastatureingaben
- Keine Cloud-Synchronisation
- Nicht alle offenen Browser-Tabs — nur der **aktive** Tab
- Keine Zeiterfassung auf Projekte/Rechnungen (noch nicht — siehe Roadmap)

---

## Wie funktioniert das technisch?

```
┌─────────────────────────────────────────┐
│  Menüleisten-Icon + Timeline-Fenster    │  ← UI (Tauri)
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│  Tracker-Thread (Poll alle 1,5 s)       │
│  · Aktive App erkennen                  │
│  · Fenstertitel + Browser-URL lesen     │
│  · Zed/Terminal-Kontext parsen          │
│  · Idle erkennen                        │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│  SQLite-Datenbank (timeline.db)         │
└─────────────────────────────────────────┘
```

### App- und Fenster-Erkennung

- **Aktive App:** Über macOS Window-APIs (`CGWindowList`) und `NSWorkspace`
- **Fenstertitel:** Über die **Bedienungshilfen**-API (Accessibility)
- **Browser-URL:** Accessibility-UI-Baum des Browsers (Chrome, Safari, Firefox, Brave, Edge, Arc, Opera)

Ohne Bedienungshilfen-Berechtigung werden nur **App-Namen** erfasst — keine Fenstertitel und keine URLs.

### Zed-Projekt-Erkennung

Zed zeigt im Fenstertitel typischerweise:

```
main.rs — timetrack
```

TimeTrack parst das Muster `{datei} — {projekt}` und speichert Projekt und Datei separat.

### Terminal & Git-Branch

Zwei Strategien (Hook hat Priorität):

1. **Shell-Hook (empfohlen):** Ein kleines Script in `~/.zshrc` meldet bei jedem Prompt den aktuellen Pfad und Git-Branch
2. **Fenstertitel-Parsing (Fallback):** Branch wird aus dem Terminal-Fenstertitel geraten (z.B. `(main)`, `[feature/auth]`, `git:branch`)

### Idle-Erkennung

Wenn keine Maus- oder Tastatur-Eingabe für **5 Minuten** erkannt wird, wird die Zeit als **Idle** markiert — nicht der zuletzt fokussierten App zugeschrieben.

---

## Benutzeroberfläche

Die App erscheint in der **Menüleiste** (Tray-Icon).

| Aktion | Verhalten |
|--------|-----------|
| Linksklick auf Tray-Icon | Timeline-Fenster öffnen/schließen |
| Rechtsklick / Menü | Timeline öffnen, Tracking pausieren, Beenden |
| **Heute / Gestern** | Tag in der Timeline wechseln |
| **Tracking pausieren** | Erfassung vorübergehend stoppen |
| **Terminal-Hook** | Installiert `~/.timetrack/hook.sh` + Anleitung |
| **Daten löschen** | Gesamte Timeline unwiderruflich leeren |

Die Timeline zeigt pro Eintrag: Uhrzeit, App-Name, Kontext (Projekt, URL, Branch) und Dauer.

---

## Installation & Start

### Voraussetzungen

- macOS 10.15+
- [Rust](https://rustup.rs/)
- Xcode Command Line Tools: `xcode-select --install`
- Node.js (für Tauri CLI)

### Entwicklung

```bash
cd timetrack
npm install
npm run tauri dev
```

### Release-Build

```bash
npm run tauri build
```

Die fertige App liegt unter:

```
src-tauri/target/release/bundle/macos/TimeTrack.app
```

## Tests (Parser & Datenbank)

```bash
# Alle Core-Tests (Unit + Integration)
cargo test -p timetrack-core

# Nur Integrationstests
cargo test -p timetrack-core --test timeline_flow

# Monitor-Tests (plattformabhängig)
cargo test -p timetrack-monitor
```

> **Hinweis:** Der gesamte macOS-Teil (Monitor, Tauri-App) wurde bisher **noch nicht auf einem Mac kompiliert oder getestet**. Siehe [`TODO_AGENT.md`](TODO_AGENT.md) für offene Aufgaben.

---

## Bedienungshilfen aktivieren

TimeTrack braucht **Bedienungshilfen** (Accessibility), um Fenstertitel und Browser-URLs anderer Apps zu lesen.

1. App starten
2. Im Banner auf **Berechtigung anfragen** klicken
3. TimeTrack in den Systemeinstellungen aktivieren

Manuell öffnen:

```bash
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
```

---

## Terminal-Hook einrichten (optional)

Für zuverlässige Git-Branch-Erfassung:

1. In der App auf **Terminal-Hook** klicken
2. In `~/.zshrc` einfügen:

```bash
source "$HOME/.timetrack/hook.sh"
```

3. Terminal neu starten

Der Hook schreibt bei jedem Prompt nach `~/.timetrack/terminal-state.jsonl`:

```json
{"cwd":"/Users/you/projects/timetrack","branch":"feature/auth","ts":1716892800}
```

---

## Datenspeicherung

| Was | Wo |
|-----|-----|
| Timeline-Datenbank | `~/Library/Application Support/timetrack/timeline.db` |
| Terminal-Hook-State | `~/.timetrack/terminal-state.jsonl` |
| Terminal-Hook-Script | `~/.timetrack/hook.sh` |

Die Datenbank ist eine SQLite-Datei. Du kannst sie jederzeit in der App löschen oder manuell entfernen.

---

## Projektstruktur

```
timetrack/
├── crates/core/        # Modelle, SQLite, Parser, Segment-Logik
├── crates/monitor/     # macOS Activity Capture (Accessibility, Idle)
├── src-tauri/          # Tauri-App, Tracker-Service, Menüleisten-Icon
├── ui/                 # Timeline-Frontend (Vanilla JS, Deutsch)
├── AGENT_BRIEF.md      # Ursprüngliche Spezifikation
└── TODO_AGENT.md       # Offene Aufgaben für macOS-Fertigstellung
```

---

## Bekannte Einschränkungen

- **macOS only** im MVP
- Browser-URL-Extraktion kann bei Browser-Updates brechen
- Git-Branch aus Terminal-Titel ist heuristisch — Hook ist zuverlässiger
- Nur der **aktive** Browser-Tab, nicht alle offenen Tabs
- App-Icon ist ein Platzhalter
- Noch **nicht auf macOS verifiziert** (Build + manuelle Tests ausstehend)

---

## Roadmap

- [ ] Smart Grouping nach Domain (z.B. `github.com` statt voller URL)
- [ ] CSV/JSON-Export für Abrechnung
- [ ] Auto-Start beim Login (LaunchAgent)
- [ ] Ausschlussliste für Apps/URLs
- [ ] App-Icons in der Timeline
- [ ] Suche/Filter in der Timeline
- [ ] Windows/Linux-Port

---

## Weitere Dokumente

| Datei | Inhalt |
|-------|--------|
| [`TODO_AGENT.md`](TODO_AGENT.md) | Was auf macOS noch getan werden muss |
| [`AGENT_BRIEF.md`](AGENT_BRIEF.md) | Vollständige technische Spezifikation |

---

## Lizenz

MIT
