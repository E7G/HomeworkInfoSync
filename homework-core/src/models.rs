use chrono::{DateTime, Duration, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Overdue,
    Urgent,
    Soon,
    Normal,
    Relaxed,
    Unknown,
}

impl Urgency {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overdue => "已过期",
            Self::Urgent => "6小时内",
            Self::Soon => "1天内",
            Self::Normal => "3天内",
            Self::Relaxed => "3天后",
            Self::Unknown => "无截止",
        }
    }

    pub fn color(self) -> &'static str {
        match self {
            Self::Overdue => "#ef5350",
            Self::Urgent => "#ffa726",
            Self::Soon => "#42a5f5",
            Self::Normal => "#66bb6a",
            Self::Relaxed => "#78909c",
            Self::Unknown => "#90a4ae",
        }
    }

    pub fn bg_color(self) -> &'static str {
        match self {
            Self::Overdue => "rgba(239,83,80,0.12)",
            Self::Urgent => "rgba(255,167,38,0.12)",
            Self::Soon => "rgba(66,165,245,0.12)",
            Self::Normal => "rgba(102,187,106,0.12)",
            Self::Relaxed => "rgba(120,144,156,0.08)",
            Self::Unknown => "rgba(144,164,174,0.08)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeworkItem {
    pub title: String,
    pub course: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<NaiveDateTime>,
    pub platform: String,
    #[serde(default)]
    pub submitted: bool,
    #[serde(default)]
    pub url: String,
}

impl HomeworkItem {
    pub fn is_overdue(&self) -> bool {
        self.deadline
            .map(|d| Local::now().naive_local() > d)
            .unwrap_or(false)
    }

    pub fn urgency(&self) -> Urgency {
        let Some(deadline) = self.deadline else {
            return Urgency::Unknown;
        };
        if self.is_overdue() {
            return Urgency::Overdue;
        }
        let now = Local::now().naive_local();
        let delta = deadline - now;
        if delta <= Duration::hours(6) {
            Urgency::Urgent
        } else if delta <= Duration::days(1) {
            Urgency::Soon
        } else if delta <= Duration::days(3) {
            Urgency::Normal
        } else {
            Urgency::Relaxed
        }
    }

    pub fn remain_text(&self) -> Option<String> {
        let deadline = self.deadline?;
        let now = Local::now().naive_local();
        let delta = deadline - now;
        if self.is_overdue() {
            let days = delta.num_days().unsigned_abs();
            Some(format!("已过期 {days}天"))
        } else if delta.num_days() > 0 {
            Some(format!("剩余 {}天", delta.num_days()))
        } else {
            let hours = delta.num_seconds() / 3600;
            let minutes = (delta.num_seconds() % 3600) / 60;
            Some(format!("剩余 {hours}时{minutes}分"))
        }
    }

    pub fn deadline_display(&self) -> String {
        self.deadline
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "无截止时间".to_string())
    }

    /// Matches Python `h.deadline or datetime.max` for stable ascending sort.
    pub fn deadline_sort_key(&self) -> NaiveDateTime {
        self.deadline.unwrap_or(NaiveDateTime::MAX)
    }
}

/// Pending (not submitted, not overdue) items sorted by deadline, earliest first.
pub fn pending_sorted_by_deadline(items: &[HomeworkItem]) -> Vec<&HomeworkItem> {
    let mut pending: Vec<_> = items
        .iter()
        .filter(|h| !h.submitted && !h.is_overdue())
        .collect();
    pending.sort_by_key(|h| h.deadline_sort_key());
    pending
}

pub fn parse_iso_datetime(raw: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.naive_local())
        .or_else(|| NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").ok())
        .or_else(|| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").ok())
}
