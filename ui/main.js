const { invoke } = window.__TAURI__.core;

const timelineEl = document.getElementById("timeline");
const emptyStateEl = document.getElementById("empty-state");
const totalLabelEl = document.getElementById("total-label");
const dayLabelEl = document.getElementById("day-label");
const permissionBannerEl = document.getElementById("permission-banner");
const pauseBtn = document.getElementById("pause-btn");
const dayPickerEl = document.getElementById("day-picker");

const BUCKET_MINUTES = 15;

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
  return new Date(activity.started_at).getTime() + activity.duration_secs * 1000;
}

function activityLabel(activity) {
  if (activity.is_idle) {
    return "Idle";
  }

  const detail =
    activity.subtitle && activity.subtitle !== activity.app_name
      ? activity.subtitle
      : activity.window_title;

  if (detail && detail !== activity.app_name) {
    return `${activity.app_name} · ${detail}`;
  }

  return activity.app_name;
}

function activityGroupKey(activity) {
  return [
    activity.is_idle ? "1" : "0",
    activity.app_name,
    activity.subtitle ?? "",
    activity.window_title ?? "",
    activity.url ?? "",
    activity.project ?? "",
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
    return "var(--idle)";
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

document.getElementById("refresh-btn").addEventListener("click", refresh);

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

document.getElementById("hook-btn").addEventListener("click", async () => {
  const message = await invoke("install_terminal_hook");
  alert(message);
});

document.getElementById("check-update-btn").addEventListener("click", async () => {
  const btn = document.getElementById("check-update-btn");
  btn.disabled = true;
  btn.textContent = "Suche…";

  try {
    const result = await invoke("check_for_updates");
    if (!result.available) {
      alert(`TimeTrack ${result.current_version} ist aktuell.`);
      return;
    }

    const notes = result.notes ? `\n\n${result.notes}` : "";
    if (
      confirm(
        `Update ${result.version} ist verfügbar.${notes}\n\nJetzt herunterladen und installieren? Die App startet danach neu.`
      )
    ) {
      btn.textContent = "Installiere…";
      await invoke("install_update");
    }
  } catch (err) {
    alert(`Update fehlgeschlagen: ${err}`);
  } finally {
    btn.disabled = false;
    btn.textContent = "Nach Updates suchen";
  }
});

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
refresh();
setInterval(refresh, 5000);
