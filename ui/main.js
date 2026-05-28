const { invoke } = window.__TAURI__.core;

const timelineEl = document.getElementById("timeline");
const emptyStateEl = document.getElementById("empty-state");
const totalLabelEl = document.getElementById("total-label");
const dayLabelEl = document.getElementById("day-label");
const permissionBannerEl = document.getElementById("permission-banner");
const pauseBtn = document.getElementById("pause-btn");

let selectedDay = "today";
let trackingPaused = false;

function formatDayLabel(dayKey) {
  const now = new Date();
  if (dayKey === "today") {
    return `Heute, ${now.toLocaleDateString("de-DE", {
      day: "numeric",
      month: "long",
      year: "numeric",
    })}`;
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  return `Gestern, ${yesterday.toLocaleDateString("de-DE", {
    day: "numeric",
    month: "long",
    year: "numeric",
  })}`;
}

function dayParam(dayKey) {
  const now = new Date();
  const target = new Date(now);
  if (dayKey === "yesterday") {
    target.setDate(now.getDate() - 1);
  }
  const yyyy = target.getFullYear();
  const mm = String(target.getMonth() + 1).padStart(2, "0");
  const dd = String(target.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

function formatTime(iso) {
  return new Date(iso).toLocaleTimeString("de-DE", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function renderActivities(activities) {
  timelineEl.innerHTML = "";

  if (!activities.length) {
    emptyStateEl.classList.add("visible");
    return;
  }

  emptyStateEl.classList.remove("visible");

  for (const activity of activities) {
    const row = document.createElement("article");
    row.className = `activity${activity.is_idle ? " idle" : ""}`;

    row.innerHTML = `
      <div class="activity-time">${formatTime(activity.started_at)}</div>
      <div>
        <div class="activity-title">${escapeHtml(
          activity.is_idle ? "Idle" : activity.app_name
        )}</div>
        <div class="activity-subtitle">${escapeHtml(activity.subtitle)}</div>
      </div>
      <div class="activity-duration">${escapeHtml(activity.duration_label)}</div>
    `;

    timelineEl.appendChild(row);
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function refresh() {
  const [activities, status] = await Promise.all([
    invoke("get_activities", { day: dayParam(selectedDay) }),
    invoke("get_tracker_status"),
  ]);

  renderActivities(activities);
  totalLabelEl.textContent = status.total_today_label;
  dayLabelEl.textContent = formatDayLabel(selectedDay);
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
    document.querySelectorAll(".chip[data-day]").forEach((el) => {
      el.classList.toggle("active", el === chip);
    });
    selectedDay = chip.dataset.day;
    await refresh();
  });
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

document.getElementById("delete-btn").addEventListener("click", async () => {
  if (!confirm("Alle erfassten Aktivitäten unwiderruflich löschen?")) {
    return;
  }
  await invoke("delete_all_data");
  await refresh();
});

refresh();
setInterval(refresh, 5000);
