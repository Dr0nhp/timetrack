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
const workHoursStartEl = document.getElementById("work-hours-start");
const workHoursEndEl = document.getElementById("work-hours-end");
const workHoursStatusEl = document.getElementById("work-hours-status");

const REFRESH_INTERVAL_MS = 2000;
const BUCKET_MINUTES = 15;
let refreshTimer = null;

let selectedDateIso = isoToday();
let trackingPaused = false;
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
        )}" title="${escapeHtml(`${item.percent}% ${item.label}`)}"></span>`
    )
    .join("");
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
        ${bucket.items
          .map(
            (item) => `
          <div class="breakdown-row${item.is_idle ? " idle" : ""}">
            <span class="breakdown-dot" style="background:${appColor(item.app_name, item.is_idle)}"></span>
            <span class="breakdown-label">${escapeHtml(`${item.percent}% ${item.label}`)}</span>
            <span class="breakdown-duration">${escapeHtml(formatDuration(item.seconds))}</span>
          </div>
        `
          )
          .join("")}
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

    timelineEl.appendChild(row);
  }
}

function renderActivities(activities) {
  lastActivities = activities;
  renderBuckets(buildBuckets(activities));
}

async function refresh() {
  const [activities, status] = await Promise.all([
    invoke("get_activities", { day: selectedDateIso }),
    invoke("get_tracker_status", { day: selectedDateIso }),
  ]);

  renderActivities(activities);
  totalLabelEl.textContent = status.total_today_label;
  dayLabelEl.textContent = formatDayLabel(selectedDateIso);
  trackingPaused = status.tracking_paused;
  pauseBtn.textContent = trackingPaused
    ? "Tracking fortsetzen"
    : "Tracking pausieren";

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

  workHoursEnabledEl.checked = status.work_hours_enabled;
  workHoursStartEl.value = status.work_hours_start;
  workHoursEndEl.value = status.work_hours_end;
  if (!status.work_hours_enabled) {
    workHoursStatusEl.textContent = "Arbeitszeiten sind deaktiviert — es wird rund um die Uhr getrackt.";
  } else if (status.work_hours_active) {
    workHoursStatusEl.textContent = `Tracking aktiv (${status.work_hours_start}–${status.work_hours_end}).`;
  } else {
    workHoursStatusEl.textContent = `Außerhalb der Arbeitszeit (${status.work_hours_start}–${status.work_hours_end}) — pausiert.`;
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

document.getElementById("open-settings-btn").addEventListener("click", async () => {
  await invoke("open_accessibility_settings_cmd");
});

pauseBtn.addEventListener("click", async () => {
  await invoke("set_tracking_paused", { paused: !trackingPaused });
  await refresh();
});

document.getElementById("save-work-hours-btn").addEventListener("click", async () => {
  await invoke("set_work_hours", {
    enabled: workHoursEnabledEl.checked,
    start: workHoursStartEl.value,
    end: workHoursEndEl.value,
  });
  await refresh();
});

document.getElementById("hook-btn").addEventListener("click", async () => {
  const message = await invoke("install_terminal_hook");
  alert(message);
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
}

function hideUpdateOverlay() {
  updateOverlayEl.classList.add("hidden");
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
