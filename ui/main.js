const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

const timelineEl = document.getElementById("timeline");
const emptyStateEl = document.getElementById("empty-state");
const totalLabelEl = document.getElementById("total-label");
const dayLabelEl = document.getElementById("day-label");
const permissionBannerEl = document.getElementById("permission-banner");
const permissionBannerPathEl = document.getElementById("permission-banner-path");
const permissionBannerHintEl = document.getElementById("permission-banner-hint");
const pauseBtn = document.getElementById("pause-btn");
const dayPickerEl = document.getElementById("day-picker");
const workHoursEnabledEl = document.getElementById("work-hours-enabled");
const workHoursStatusEl = document.getElementById("work-hours-status");
const workWeekEl = document.getElementById("work-week");
const saveWorkHoursBtnEl = document.getElementById("save-work-hours-btn");
const terminalHookStatusEl = document.getElementById("terminal-hook-status");
const capturePreviewStatusEl = document.getElementById("capture-preview-status");

const WEEKDAYS = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
const settingsOverlayEl = document.getElementById("settings-overlay");
const settingsPanelEl = document.getElementById("settings-panel");
const openSettingsBtnEl = document.getElementById("open-settings-btn");
const settingsBodyEl = settingsPanelEl.querySelector(".sheet-body");
const updateAvailableBannerEl = document.getElementById("update-available-banner");
const updateAvailableVersionEl = document.getElementById("update-available-version");

const REFRESH_INTERVAL_MS = 2000;
const BUCKET_MINUTES = 15;
const SAVE_FEEDBACK_MS = 2200;
let refreshTimer = null;
let scrollLockDepth = 0;
let workHoursSaveFeedbackTimer = null;

let selectedDateIso = isoToday();
let trackingPaused = false;
let latestTrackerStatus = null;
let lastActivities = [];
const expandedBuckets = new Set();

function isoToday() {
  return formatIsoDate(new Date());
}

function isoYesterday() {
  const date = new Date();
  date.setDate(date.getDate() - 1);
  return formatIsoDate(date);
}

function formatIsoDate(date) {
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const dd = String(date.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

function parseIsoDate(iso) {
  const [year, month, day] = iso.split("-").map(Number);
  return new Date(year, month - 1, day);
}

function formatDayLabel(iso) {
  const date = parseIsoDate(iso);
  const formatted = date.toLocaleDateString("de-DE", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });

  if (iso === isoToday()) {
    return `Heute, ${formatted}`;
  }
  if (iso === isoYesterday()) {
    return `Gestern, ${formatted}`;
  }
  return formatted;
}

function formatDayLabelShort(iso) {
  return parseIsoDate(iso).toLocaleDateString("de-DE", {
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

function syncDayPickerLimits() {
  dayPickerEl.max = isoToday();
}

function syncChipState() {
  document.querySelectorAll(".chip[data-day]").forEach((chip) => {
    const isToday = chip.dataset.day === "today" && selectedDateIso === isoToday();
    const isYesterday =
      chip.dataset.day === "yesterday" && selectedDateIso === isoYesterday();
    chip.classList.toggle("active", isToday || isYesterday);
  });
}

function setSelectedDate(iso) {
  selectedDateIso = iso;
  dayPickerEl.value = iso;
  syncChipState();
}

function formatTime(date) {
  return date.toLocaleTimeString("de-DE", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTimeRange(startMs, endMs) {
  return `${formatTime(new Date(startMs))}–${formatTime(new Date(endMs))}`;
}

function formatDuration(secs) {
  const rounded = Math.round(secs);
  if (rounded < 60) {
    return `${rounded}s`;
  }
  const mins = Math.round(rounded / 60);
  if (mins < 60) {
    return `${mins} Min`;
  }
  const hours = Math.floor(mins / 60);
  const rem = mins % 60;
  if (rem === 0) {
    return `${hours}h`;
  }
  return `${hours}h ${rem}m`;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function activityEndMs(activity) {
  if (activity.ended_at) {
    return new Date(activity.ended_at).getTime();
  }
  return Date.now();
}

function activityLabel(activity) {
  if (activity.is_idle) {
    return "Idle";
  }

  if (activity.subtitle && activity.subtitle !== activity.app_name) {
    return `${activity.app_name} · ${activity.subtitle}`;
  }

  return activity.app_name;
}

function activityGroupKey(activity) {
  return [
    activity.is_idle ? "1" : "0",
    activity.app_name,
    activity.subtitle ?? "",
    activity.url ?? "",
  ].join("|");
}

function floorToBucket(ms) {
  const date = new Date(ms);
  date.setSeconds(0, 0);
  date.setMinutes(date.getMinutes() - (date.getMinutes() % BUCKET_MINUTES));
  return date.getTime();
}

function normalizePercents(items) {
  const totalSeconds = items.reduce((sum, item) => sum + item.seconds, 0);
  if (totalSeconds <= 0) {
    return items.map((item) => ({ ...item, percent: 0 }));
  }

  let assigned = 0;
  return items.map((item, index) => {
    if (index === items.length - 1) {
      return { ...item, percent: Math.max(0, 100 - assigned) };
    }
    const percent = Math.round((item.seconds / totalSeconds) * 100);
    assigned += percent;
    return { ...item, percent };
  });
}

function buildBuckets(activities) {
  const bucketMs = BUCKET_MINUTES * 60 * 1000;
  const bucketMap = new Map();

  for (const activity of activities) {
    const startMs = new Date(activity.started_at).getTime();
    const endMs = activityEndMs(activity);
    const label = activityLabel(activity);
    const key = activityGroupKey(activity);

    let cursor = floorToBucket(startMs);

    while (cursor < endMs) {
      const bucketEnd = cursor + bucketMs;
      const overlapStart = Math.max(startMs, cursor);
      const overlapEnd = Math.min(endMs, bucketEnd);
      const seconds = (overlapEnd - overlapStart) / 1000;

      if (seconds >= 1) {
        if (!bucketMap.has(cursor)) {
          bucketMap.set(cursor, new Map());
        }

        const breakdown = bucketMap.get(cursor);
        const entry = breakdown.get(key) ?? {
          label,
          seconds: 0,
          is_idle: activity.is_idle,
          app_name: activity.app_name,
          url: activity.url ?? null,
        };
        entry.seconds += seconds;
        breakdown.set(key, entry);
      }

      cursor = bucketEnd;
    }
  }

  return Array.from(bucketMap.entries())
    .map(([startMs, breakdownMap]) => {
      const items = normalizePercents(
        Array.from(breakdownMap.values()).sort((a, b) => b.seconds - a.seconds)
      );
      const totalSeconds = items.reduce((sum, item) => sum + item.seconds, 0);

      return {
        startMs,
        endMs: startMs + bucketMs,
        totalSeconds,
        items,
      };
    })
    .sort((a, b) => a.startMs - b.startMs);
}

function appColor(appName, isIdle) {
  if (isIdle) {
    return "hsl(var(--idle))";
  }

  let hash = 0;
  for (const char of appName) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  const hue = hash % 360;
  return `hsl(${hue} 55% 52%)`;
}

function renderBucketBar(items) {
  return items
    .map(
      (item) =>
        `<span class="bucket-bar-segment" style="flex-grow:${item.seconds}; background:${appColor(
          item.app_name,
          item.is_idle
        )}"></span>`
    )
    .join("");
}

function renderBreakdownRow(item) {
  const url = item.url?.trim();
  const hasUrl = Boolean(url);
  const rowClass = `breakdown-row${item.is_idle ? " idle" : ""}${hasUrl ? " has-url" : ""}`;
  const urlBlock = hasUrl
    ? `<span class="breakdown-url hidden" aria-hidden="true">${escapeHtml(url)}</span>`
    : "";

  if (hasUrl) {
    return `
      <button type="button" class="${rowClass}" aria-label="${escapeHtml(`${item.percent}% ${item.label}`)}" aria-expanded="false">
        <span class="breakdown-dot" style="background:${appColor(item.app_name, item.is_idle)}"></span>
        <span class="breakdown-label-wrap">
          <span class="breakdown-label">${escapeHtml(`${item.percent}% ${item.label}`)}</span>
          ${urlBlock}
        </span>
        <span class="breakdown-duration">${escapeHtml(formatDuration(item.seconds))}</span>
      </button>
    `;
  }

  return `
    <div class="${rowClass}">
      <span class="breakdown-dot" style="background:${appColor(item.app_name, item.is_idle)}"></span>
      <span class="breakdown-label">${escapeHtml(`${item.percent}% ${item.label}`)}</span>
      <span class="breakdown-duration">${escapeHtml(formatDuration(item.seconds))}</span>
    </div>
  `;
}

function renderBucketSummary(items, maxItems = 3) {
  const visible = items.slice(0, maxItems);
  const summary = visible
    .map((item) => `${item.percent}% ${item.label}`)
    .join(" · ");

  if (items.length > maxItems) {
    return `${summary} · …`;
  }

  return summary;
}

function renderBuckets(buckets) {
  timelineEl.innerHTML = "";

  if (!buckets.length) {
    emptyStateEl.classList.add("visible");
    return;
  }

  emptyStateEl.classList.remove("visible");

  for (const bucket of buckets) {
    const bucketKey = String(bucket.startMs);
    const expanded = expandedBuckets.has(bucketKey);

    const row = document.createElement("article");
    row.className = `bucket${expanded ? " expanded" : ""}`;
    row.dataset.bucketStart = bucketKey;

    row.innerHTML = `
      <button type="button" class="bucket-header" aria-expanded="${expanded}">
        <div class="bucket-time">${formatTimeRange(bucket.startMs, bucket.endMs)}</div>
        <div class="bucket-main">
          <div class="bucket-bar">${renderBucketBar(bucket.items)}</div>
          <div class="bucket-summary">${escapeHtml(renderBucketSummary(bucket.items))}</div>
        </div>
        <div class="bucket-duration">${escapeHtml(formatDuration(bucket.totalSeconds))}</div>
        <span class="bucket-chevron" aria-hidden="true"></span>
      </button>
      <div class="bucket-details">
        ${bucket.items.map((item) => renderBreakdownRow(item)).join("")}
      </div>
    `;

    row.querySelector(".bucket-header").addEventListener("click", () => {
      if (expandedBuckets.has(bucketKey)) {
        expandedBuckets.delete(bucketKey);
      } else {
        expandedBuckets.add(bucketKey);
      }
      renderBuckets(buildBuckets(lastActivities));
    });

    row.querySelectorAll(".breakdown-row.has-url").forEach((entry) => {
      entry.addEventListener("click", (event) => {
        event.stopPropagation();
        const showing = entry.classList.toggle("url-visible");
        const urlEl = entry.querySelector(".breakdown-url");
        if (urlEl) {
          urlEl.classList.toggle("hidden", !showing);
          urlEl.setAttribute("aria-hidden", showing ? "false" : "true");
        }
        entry.setAttribute("aria-expanded", showing ? "true" : "false");
      });
    });

    timelineEl.appendChild(row);
  }
}

function renderActivities(activities) {
  lastActivities = activities;
  renderBuckets(buildBuckets(activities));
}

function formatTerminalHookStatus(status) {
  if (!status.hook_script_installed) {
    return "Hook: Script fehlt — zuerst „Terminal-Hook installieren“ klicken.";
  }
  if (!status.shell_configured) {
    return "Hook: ~/.zshrc enthält noch kein source ~/.timetrack/hook.sh — danach Terminal neu starten.";
  }
  if (!status.state_file_exists) {
    return "Hook: In Terminal einmal Enter drücken — dann sollte ~/.timetrack/terminal-state.jsonl entstehen.";
  }
  if (!status.latest_branch && !status.latest_cwd) {
    return "Hook: State-Datei vorhanden, aber noch ohne Branch/CWD — in einem Git-Repo arbeiten.";
  }
  const parts = [];
  if (status.latest_cwd) {
    parts.push(status.latest_cwd);
  }
  if (status.latest_branch) {
    parts.push(`Branch: ${status.latest_branch}`);
  }
  return `Hook aktiv — zuletzt: ${parts.join(" · ")}. In der Timeline: „Terminal · Branch: …“.`;
}

function formatCapturePreview(preview) {
  if (!preview.accessibility_trusted) {
    return "Titles: Bedienungshilfen fehlen für diese App-Binary — gelber Banner oben beachten.";
  }
  if (!preview.window_title && !preview.url) {
    return `Titles: Berechtigung da, aber kein Fenstertitel von „${preview.frontmost_app}“ — App neu starten.`;
  }
  if (preview.url) {
    return `Titles OK — ${preview.frontmost_app}: ${preview.window_title || preview.url}`;
  }
  return `Titles OK — ${preview.frontmost_app}: ${preview.window_title}`;
}

async function refreshSettingsDiagnostics() {
  const [hookStatus, capturePreview] = await Promise.all([
    invoke("get_terminal_hook_status"),
    invoke("get_capture_preview"),
  ]);
  terminalHookStatusEl.textContent = formatTerminalHookStatus(hookStatus);
  capturePreviewStatusEl.textContent = formatCapturePreview(capturePreview);
}

function todayWeekdayIndex() {
  return (new Date().getDay() + 6) % 7;
}

function renderWorkWeek(days) {
  workWeekEl.innerHTML = WEEKDAYS.map((label, index) => {
    const day = days[index] ?? { enabled: true, start: "09:00", end: "18:00" };
    return `
      <div class="work-day-row" data-weekday="${index}">
        <label class="work-day-name">
          <input
            type="checkbox"
            class="work-day-enabled"
            ${day.enabled ? "checked" : ""}
          />
          <span>${label}</span>
        </label>
        <input type="time" class="work-day-start" value="${day.start}" />
        <span class="work-day-sep" aria-hidden="true">–</span>
        <input type="time" class="work-day-end" value="${day.end}" />
      </div>
    `;
  }).join("");
}

function collectWorkWeek() {
  return WEEKDAYS.map((_, index) => {
    const row = workWeekEl.querySelector(`[data-weekday="${index}"]`);
    return {
      enabled: row.querySelector(".work-day-enabled").checked,
      start: row.querySelector(".work-day-start").value,
      end: row.querySelector(".work-day-end").value,
    };
  });
}

function formatWorkHoursStatus(status) {
  if (!status.work_hours_enabled) {
    return "Arbeitszeiten sind deaktiviert — es wird rund um die Uhr getrackt.";
  }

  const todayIndex = todayWeekdayIndex();
  const todayName = WEEKDAYS[todayIndex];
  const today = status.work_hours_week[todayIndex];

  if (!today?.enabled) {
    return `Heute (${todayName}): freier Tag — kein Tracking.`;
  }

  const windowLabel = `${today.start}–${today.end}`;
  if (status.work_hours_active) {
    return `Heute (${todayName}): ${windowLabel} — aktiv.`;
  }
  return `Heute (${todayName}): ${windowLabel} — außerhalb der Zeit.`;
}

function clearWorkHoursSaveFeedback() {
  if (workHoursSaveFeedbackTimer) {
    clearTimeout(workHoursSaveFeedbackTimer);
    workHoursSaveFeedbackTimer = null;
  }
  workHoursStatusEl.classList.remove("is-success");
  saveWorkHoursBtnEl.classList.remove("success");
  saveWorkHoursBtnEl.disabled = false;
  saveWorkHoursBtnEl.textContent = "Arbeitszeiten speichern";
}

function showWorkHoursSaveSuccess() {
  clearWorkHoursSaveFeedback();
  saveWorkHoursBtnEl.textContent = "Gespeichert";
  saveWorkHoursBtnEl.classList.add("success");
  workHoursStatusEl.textContent = "Arbeitszeiten gespeichert.";
  workHoursStatusEl.classList.add("is-success");

  workHoursSaveFeedbackTimer = setTimeout(() => {
    workHoursStatusEl.classList.remove("is-success");
    if (latestTrackerStatus) {
      workHoursStatusEl.textContent = formatWorkHoursStatus(latestTrackerStatus);
    }
    saveWorkHoursBtnEl.classList.remove("success");
    saveWorkHoursBtnEl.textContent = "Arbeitszeiten speichern";
    saveWorkHoursBtnEl.disabled = false;
    workHoursSaveFeedbackTimer = null;
  }, SAVE_FEEDBACK_MS);
}

async function refresh() {
  const [activities, status] = await Promise.all([
    invoke("get_activities", { day: selectedDateIso }),
    invoke("get_tracker_status", { day: selectedDateIso }),
  ]);

  renderActivities(activities);
  totalLabelEl.textContent = status.total_today_label;
  dayLabelEl.textContent = formatDayLabel(selectedDateIso);
  latestTrackerStatus = status;
  trackingPaused = status.tracking_paused;
  pauseBtn.textContent = trackingPaused
    ? "Tracking fortsetzen"
    : "Tracking pausieren";
  updateSettingsBadges(status);

  if (status.accessibility_granted) {
    permissionBannerEl.classList.add("hidden");
  } else {
    permissionBannerEl.classList.remove("hidden");
    if (status.app_binary_path) {
      permissionBannerPathEl.textContent = status.app_binary_path;
      const isDevBuild = status.app_binary_path.includes("/target/debug/");
      permissionBannerHintEl.textContent = isDevBuild
        ? "Dev-Modus: In den Systemeinstellungen timetrack aktivieren (nicht Cursor). Danach App beenden und neu starten."
        : "In den Systemeinstellungen unter Bedienungshilfen TimeTrack aktivieren und danach neu starten.";
    }
  }

  if (settingsOverlayEl.classList.contains("hidden")) {
    workHoursEnabledEl.checked = status.work_hours_enabled;
    renderWorkWeek(status.work_hours_week);
  }
  if (!workHoursSaveFeedbackTimer) {
    workHoursStatusEl.textContent = formatWorkHoursStatus(status);
  }
}

document.querySelectorAll(".chip[data-day]").forEach((chip) => {
  chip.addEventListener("click", async () => {
    const iso = chip.dataset.day === "yesterday" ? isoYesterday() : isoToday();
    setSelectedDate(iso);
    expandedBuckets.clear();
    await refresh();
  });
});

dayPickerEl.addEventListener("change", async () => {
  if (!dayPickerEl.value) {
    return;
  }
  setSelectedDate(dayPickerEl.value);
  expandedBuckets.clear();
  await refresh();
});

document.getElementById("request-access-btn").addEventListener("click", async () => {
  await invoke("request_accessibility");
  await refresh();
});

document.getElementById("open-accessibility-settings-btn").addEventListener("click", async () => {
  await invoke("open_accessibility_settings_cmd");
});

function setScrollLocked(locked) {
  if (locked) {
    scrollLockDepth += 1;
    if (scrollLockDepth === 1) {
      document.documentElement.classList.add("scroll-locked");
    }
    return;
  }

  scrollLockDepth = Math.max(0, scrollLockDepth - 1);
  if (scrollLockDepth === 0) {
    document.documentElement.classList.remove("scroll-locked");
  }
}

function trackingStatusKind(status) {
  if (status.tracking_error) {
    return "error";
  }
  if (
    status.tracking_paused ||
    (status.work_hours_enabled && !status.work_hours_active)
  ) {
    return "paused";
  }
  return "active";
}

function updateSettingsBadges(status) {
  const kind = trackingStatusKind(status);
  openSettingsBtnEl.classList.remove(
    "status-dot-active",
    "status-dot-paused",
    "status-dot-error",
    "has-update-dot"
  );
  openSettingsBtnEl.classList.add(`status-dot-${kind}`);

  const trackingTitles = {
    active: "Tracking aktiv",
    paused: "Tracking pausiert",
    error: status.tracking_error || "Tracking-Fehler",
  };
  let title = trackingTitles[kind];
  let ariaLabel = "Einstellungen";

  if (status.update_available) {
    openSettingsBtnEl.classList.add("has-update-dot");
    updateAvailableVersionEl.textContent = status.update_available;
    updateAvailableBannerEl.classList.remove("hidden");
    title += ` · Update ${status.update_available} verfügbar`;
    ariaLabel = `Einstellungen, Update ${status.update_available} verfügbar`;
  } else {
    updateAvailableBannerEl.classList.add("hidden");
  }

  openSettingsBtnEl.title = title;
  openSettingsBtnEl.setAttribute("aria-label", ariaLabel);
}

function openSettingsOverlay() {
  settingsOverlayEl.classList.remove("hidden");
  settingsOverlayEl.setAttribute("aria-hidden", "false");
  setScrollLocked(true);
  if (latestTrackerStatus) {
    workHoursEnabledEl.checked = latestTrackerStatus.work_hours_enabled;
    renderWorkWeek(latestTrackerStatus.work_hours_week);
    workHoursStatusEl.textContent = formatWorkHoursStatus(latestTrackerStatus);
  }
  refreshSettingsDiagnostics().catch(() => {});
  document.getElementById("close-settings-btn").focus();
}

function closeSettingsOverlay() {
  if (settingsOverlayEl.classList.contains("hidden")) {
    return;
  }
  settingsOverlayEl.classList.add("hidden");
  settingsOverlayEl.setAttribute("aria-hidden", "true");
  setScrollLocked(false);
  openSettingsBtnEl.focus();
}

document.getElementById("open-settings-btn").addEventListener("click", openSettingsOverlay);
document.getElementById("close-settings-btn").addEventListener("click", closeSettingsOverlay);
document.getElementById("settings-backdrop").addEventListener("click", closeSettingsOverlay);

document.getElementById("settings-backdrop").addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
  },
  { passive: false }
);

settingsBodyEl.addEventListener(
  "wheel",
  (event) => {
    event.stopPropagation();
  },
  { passive: true }
);

document.addEventListener(
  "wheel",
  (event) => {
    if (scrollLockDepth === 0) {
      return;
    }
    if (settingsBodyEl.contains(event.target)) {
      return;
    }
    event.preventDefault();
  },
  { passive: false }
);

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !settingsOverlayEl.classList.contains("hidden")) {
    closeSettingsOverlay();
  }
});

pauseBtn.addEventListener("click", async () => {
  await invoke("set_tracking_paused", { paused: !trackingPaused });
  await refresh();
});

document.getElementById("save-work-hours-btn").addEventListener("click", async () => {
  clearWorkHoursSaveFeedback();
  saveWorkHoursBtnEl.disabled = true;
  saveWorkHoursBtnEl.textContent = "Speichern…";

  try {
    await invoke("set_work_schedule", {
      enabled: workHoursEnabledEl.checked,
      days: collectWorkWeek(),
    });
    await refresh();
    showWorkHoursSaveSuccess();
  } catch (err) {
    clearWorkHoursSaveFeedback();
    alert(`Arbeitszeiten konnten nicht gespeichert werden: ${err}`);
  }
});

async function exportActivities(format, scope) {
  try {
    const message = await invoke("export_activities", {
      format,
      scope,
      day: scope === "day" ? selectedDateIso : null,
    });
    alert(message);
  } catch (err) {
    alert(`Export fehlgeschlagen: ${err}`);
  }
}

document.getElementById("export-day-csv-btn").addEventListener("click", () => {
  exportActivities("csv", "day");
});
document.getElementById("export-day-json-btn").addEventListener("click", () => {
  exportActivities("json", "day");
});
document.getElementById("export-all-csv-btn").addEventListener("click", () => {
  exportActivities("csv", "all");
});
document.getElementById("export-all-json-btn").addEventListener("click", () => {
  exportActivities("json", "all");
});

document.getElementById("hook-btn").addEventListener("click", async () => {
  const message = await invoke("install_terminal_hook");
  alert(message);
  await refreshSettingsDiagnostics();
});

const updateOverlayEl = document.getElementById("update-overlay");
const updateOverlayTitleEl = document.getElementById("update-overlay-title");
const updateOverlayMessageEl = document.getElementById("update-overlay-message");
const updateOverlayProgressEl = document.getElementById("update-overlay-progress");

function showUpdateOverlay(title, message, progress = null) {
  updateOverlayTitleEl.textContent = title;
  updateOverlayMessageEl.textContent = message;
  if (progress === null) {
    updateOverlayProgressEl.classList.add("hidden");
    updateOverlayProgressEl.removeAttribute("value");
  } else {
    updateOverlayProgressEl.classList.remove("hidden");
    updateOverlayProgressEl.value = progress;
  }
  updateOverlayEl.classList.remove("hidden");
  setScrollLocked(true);
  closeSettingsOverlay();
}

function hideUpdateOverlay() {
  if (updateOverlayEl.classList.contains("hidden")) {
    return;
  }
  updateOverlayEl.classList.add("hidden");
  setScrollLocked(false);
}

listen("update-progress", ({ payload }) => {
  const titles = {
    checking: "Update wird vorbereitet",
    downloading: "Update wird heruntergeladen",
    installing: "Update wird installiert",
    restarting: "Neustart",
  };
  const title = titles[payload.phase] || "Update";
  let progress = null;
  if (payload.phase === "downloading" && payload.total) {
    progress = Math.min(
      100,
      Math.round((payload.downloaded / payload.total) * 100)
    );
  } else if (payload.phase === "installing") {
    progress = 100;
  }
  showUpdateOverlay(title, payload.message, progress);
});

async function focusMainWindow() {
  const window = getCurrentWebviewWindow();
  await window.show();
  await window.unminimize();
  await window.setFocus();
}

async function waitForNextFrame() {
  await new Promise((resolve) => requestAnimationFrame(resolve));
  await new Promise((resolve) => requestAnimationFrame(resolve));
}

async function checkForUpdates() {
  try {
    await focusMainWindow();
    const result = await invoke("check_for_updates");
    if (!result.available) {
      alert(`TimeTrack ${result.current_version} ist aktuell.`);
      return;
    }

    const notes = result.notes ? `\n\n${result.notes}` : "";
    if (
      confirm(
        `Update ${result.version} ist verfügbar.${notes}\n\nJetzt herunterladen und installieren? Die App startet danach neu.\n\nHinweis: macOS kann nach dem Download einen Passwort-Dialog anzeigen.`
      )
    ) {
      showUpdateOverlay(
        "Update wird heruntergeladen",
        "Bitte warten… Ein macOS-Passwort-Dialog kann erscheinen."
      );
      await waitForNextFrame();
      await invoke("install_update");
    }
  } catch (err) {
    hideUpdateOverlay();
    alert(`Update fehlgeschlagen: ${err}`);
  }
}

document.getElementById("install-update-btn").addEventListener("click", () => {
  checkForUpdates();
});

listen("update-available-changed", () => {
  if (!document.hidden) {
    refresh();
  }
});

listen("timeline-changed", () => {
  if (!document.hidden) {
    refresh();
  }
});

function syncAutoRefresh() {
  if (document.hidden) {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
    return;
  }

  if (!refreshTimer) {
    refresh();
    refreshTimer = setInterval(refresh, REFRESH_INTERVAL_MS);
  }
}

document.addEventListener("visibilitychange", syncAutoRefresh);

document.getElementById("delete-day-btn").addEventListener("click", async () => {
  const label = formatDayLabelShort(selectedDateIso);
  if (
    !confirm(
      `Alle Aktivitäten vom ${label} unwiderruflich löschen?\n\nDer Rest der Timeline bleibt erhalten.`
    )
  ) {
    return;
  }

  const deleted = await invoke("delete_day_data", { day: selectedDateIso });
  alert(`${deleted} Einträge gelöscht.`);
  expandedBuckets.clear();
  await refresh();
});

document.getElementById("delete-all-btn").addEventListener("click", async () => {
  if (
    !confirm(
      "Alle erfassten Aktivitäten unwiderruflich löschen?\n\nDie komplette Timeline-Datenbank wird geleert."
    )
  ) {
    return;
  }

  const deleted = await invoke("delete_all_data");
  alert(`${deleted} Einträge gelöscht.`);
  expandedBuckets.clear();
  await refresh();
});

syncDayPickerLimits();
setSelectedDate(selectedDateIso);
syncAutoRefresh();
