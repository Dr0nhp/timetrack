# TimeTrack Release — Anleitung für GitHub & Updates

Diese Anleitung erklärt Schritt für Schritt, wie du eine neue Version veröffentlichst und wie Nutzer sie per OTA-Update bekommen.

---

## Was passiert überhaupt?

```
Du bumpst Version → pushst Tag v0.3.0
        ↓
GitHub Actions baut die App (macOS, signiert)
        ↓
GitHub Release mit DMG + Update-Dateien
        ↓
Nutzer: Hilfe → „Nach Updates suchen…“
        ↓
App lädt Update von GitHub und installiert es (OTA)
```

**Wichtig:** Ein normaler Push auf `main` reicht **nicht**. Es braucht immer einen **Git-Tag** (`v0.2.0`, `v0.3.0`, …).

---

## Einmalig einrichten (nur 1× nötig)

### Schritt 1: Signing-Key in GitHub hinterlegen

Der private Key liegt auf deinem Mac (nicht in Git):

```
src-tauri/.tauri/timetrack.key
```

1. Öffne https://github.com/Dr0nhp/timetrack/settings/secrets/actions
2. Klicke **New repository secret**
3. **Name:** `TAURI_SIGNING_PRIVATE_KEY`
4. **Secret:** Inhalt der Datei `timetrack.key` komplett reinkopieren (eine lange Zeile)
5. **Add secret**

**Falls du beim Key-Generate ein Passwort gesetzt hast**, zusätzlich:

| Name | Wert |
|------|------|
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | dein Passwort |

Ohne Passwort beim Key: Secret weglassen oder leer lassen.

Fertig. Mehr musst du in GitHub nicht konfigurieren — der Workflow `.github/workflows/release.yml` ist schon da.

### Schritt 2: Erste Installation (einmal pro Mac)

Nutzer (auch du) brauchen **einmal** eine installierte App:

- DMG aus dem GitHub Release laden, **oder**
- lokal bauen: `npm run tauri build` und `TimeTrack.app` nach `/Applications/` kopieren

Danach gehen Updates über **Hilfe → Nach Updates suchen…**.

---

## Jedes Release (Checkliste)

### 1. Version erhöhen

Diese **drei Dateien** müssen die **gleiche** Versionsnummer haben (ohne `v`):

| Datei | Beispiel |
|-------|----------|
| `src-tauri/tauri.conf.json` | `"version": "0.3.0"` |
| `Cargo.toml` (ganz oben) | `version = "0.3.0"` |
| `package.json` | `"version": "0.3.0"` |

Regel: Config = `0.3.0`, Git-Tag = `v0.3.0`.

### 2. Änderungen committen und pushen

```bash
cd /Users/daniel/projects/timetrack

git add .
git commit -m "Release 0.3.0"
git push origin main
```

### 3. Tag setzen und pushen (startet den Build)

```bash
git tag v0.3.0
git push origin v0.3.0
```

**Genau dieser Schritt** triggert GitHub Actions.

### 4. Warten und prüfen

1. https://github.com/Dr0nhp/timetrack/actions — Workflow **Release** sollte grün werden (~5–10 Min.)
2. https://github.com/Dr0nhp/timetrack/releases — neues Release mit:
   - `TimeTrack_…_aarch64.dmg` (Erstinstallation)
   - `TimeTrack.app.tar.gz` (OTA-Update)
   - `TimeTrack.app.tar.gz.sig`
   - `latest.json` (Update-Infos)

Wenn der Workflow **rot** ist: Log öffnen — meist fehlt das Secret oder der Key stimmt nicht.

### 5. Update testen

Auf dem Mac, wo **noch die alte Version** installiert ist:

1. TimeTrack öffnen
2. **Hilfe → Nach Updates suchen…**
3. Dialog bestätigen → App lädt, installiert, startet neu

---

## Häufige Fragen

### Muss ich lokal `npm run tauri build` machen?

| Wer | Wann |
|-----|------|
| **Nutzer** | Nein — Update kommt OTA |
| **Du für Release** | Nein — GitHub Actions baut |
| **Du zum Testen** | Ja — oder DMG aus Release installieren |

### Kommt das Update automatisch beim App-Start?

**Nein.** Nutzer müssen **Hilfe → Nach Updates suchen…** klicken.

### Was, wenn „Kein Update verfügbar“?

- Installierte Version ist schon aktuell, **oder**
- Release/Tag ist noch nicht fertig gebaut, **oder**
- Du testest mit einem **Dev-Build** (Updater funktioniert nur mit Release-App aus `/Applications/`)

### Tag schon gepusht, Version war falsch?

Neuen Tag mit höherer Version verwenden (z. B. `v0.2.1`). Alte Tags nicht überschreiben.

### Key verloren?

Neuen Key generieren (`npm run tauri signer generate`), `pubkey` in `tauri.conf.json` anpassen, Secret in GitHub tauschen. **Alle Nutzer** brauchen dann einmal neu installieren (alter Key passt nicht mehr).

---

## Copy-Paste-Vorlage für Release 0.3.0

```bash
# 1. Version in tauri.conf.json, Cargo.toml, package.json auf 0.3.0 setzen

git add .
git commit -m "Release 0.3.0"
git push origin main

git tag v0.3.0
git push origin v0.3.0

# 2. Actions abwarten, dann in der App: Hilfe → Nach Updates suchen
```

---

## Kurz-Glossar

| Begriff | Bedeutung |
|---------|-----------|
| **OTA** | Over-the-Air — Update ohne DMG, direkt in der App |
| **Tag** | Git-Marker wie `v0.2.0`, startet den CI-Build |
| **latest.json** | Kleine Datei auf GitHub, sagt der App „es gibt Version X“ |
| **Signing Key** | Beweist, dass das Update wirklich von dir kommt |
