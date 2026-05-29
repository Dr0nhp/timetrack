use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use timetrack_core::Activity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    fn mime_filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Csv => ("CSV", &["csv"]),
            Self::Json => ("JSON", &["json"]),
        }
    }
}

impl TryFrom<&str> for ExportFormat {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            _ => Err("Format muss csv oder json sein.".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    Day,
    All,
}

impl TryFrom<&str> for ExportScope {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "day" => Ok(Self::Day),
            "all" => Ok(Self::All),
            _ => Err("Scope muss day oder all sein.".into()),
        }
    }
}

#[derive(Serialize)]
struct ExportRow {
    started_at: String,
    ended_at: Option<String>,
    duration_secs: i64,
    app_name: String,
    app_bundle_id: String,
    window_title: String,
    url: Option<String>,
    page_title: Option<String>,
    project: Option<String>,
    file: Option<String>,
    cwd: Option<String>,
    git_branch: Option<String>,
    is_idle: bool,
}

pub fn default_filename(format: ExportFormat, scope: ExportScope, day: NaiveDate) -> String {
    match scope {
        ExportScope::Day => format!("timetrack-{}.{}", day.format("%Y-%m-%d"), format.extension()),
        ExportScope::All => format!("timetrack-all.{}", format.extension()),
    }
}

pub fn serialize_activities(
    activities: &[Activity],
    format: ExportFormat,
    now: DateTime<Utc>,
) -> Result<String, String> {
    let rows: Vec<ExportRow> = activities
        .iter()
        .map(|activity| ExportRow {
            started_at: activity.started_at.to_rfc3339(),
            ended_at: activity.ended_at.map(|t| t.to_rfc3339()),
            duration_secs: activity.duration_secs_at(now),
            app_name: activity.app_name.clone(),
            app_bundle_id: activity.app_bundle_id.clone(),
            window_title: activity.window_title.clone(),
            url: activity.context.url.clone(),
            page_title: activity.context.page_title.clone(),
            project: activity.context.project.clone(),
            file: activity.context.file.clone(),
            cwd: activity.context.cwd.clone(),
            git_branch: activity.context.git_branch.clone(),
            is_idle: activity.is_idle,
        })
        .collect();

    match format {
        ExportFormat::Csv => activities_to_csv(&rows),
        ExportFormat::Json => serde_json::to_string_pretty(&rows).map_err(|e| e.to_string()),
    }
}

fn activities_to_csv(rows: &[ExportRow]) -> Result<String, String> {
    let mut out = String::from(
        "started_at,ended_at,duration_secs,app_name,app_bundle_id,window_title,url,page_title,project,file,cwd,git_branch,is_idle\n",
    );

    for row in rows {
        let fields = [
            row.started_at.as_str(),
            row.ended_at.as_deref().unwrap_or(""),
            &row.duration_secs.to_string(),
            row.app_name.as_str(),
            row.app_bundle_id.as_str(),
            row.window_title.as_str(),
            row.url.as_deref().unwrap_or(""),
            row.page_title.as_deref().unwrap_or(""),
            row.project.as_deref().unwrap_or(""),
            row.file.as_deref().unwrap_or(""),
            row.cwd.as_deref().unwrap_or(""),
            row.git_branch.as_deref().unwrap_or(""),
            if row.is_idle { "true" } else { "false" },
        ];
        out.push_str(&fields
            .iter()
            .map(|field| csv_escape(field))
            .collect::<Vec<_>>()
            .join(","));
        out.push('\n');
    }

    Ok(out)
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn format_filter(format: ExportFormat) -> (&'static str, &'static [&'static str]) {
    format.mime_filter()
}
