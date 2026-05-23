use chrono::{DateTime, Duration, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Same rules as the desktop app `pack_items` headline stats.
pub fn homework_headline_counts(items: &[HomeworkItem]) -> (usize, usize, usize, usize) {
    let pending = pending_sorted_by_deadline(items);
    let urgent = pending
        .iter()
        .filter(|h| h.urgency() == Urgency::Urgent)
        .count();
    let done = items.iter().filter(|h| h.submitted).count();
    (items.len(), pending.len(), urgent, done)
}

/// Explains headline stats, list vs totals, overdue gap, urgency mix, per-platform breakdown.
pub fn homework_stats_debug_report(items: &[HomeworkItem]) -> String {
    let (total, pending_len, urgent, done) = homework_headline_counts(items);
    let mut overdue_unsubmitted_items: Vec<&HomeworkItem> = items
        .iter()
        .filter(|h| !h.submitted && h.is_overdue())
        .collect();
    overdue_unsubmitted_items.sort_by_key(|h| h.deadline_sort_key());
    let overdue_unsubmitted = overdue_unsubmitted_items.len();
    let pending_refs = pending_sorted_by_deadline(items);

    let mut urgency_on_list: BTreeMap<&str, usize> = BTreeMap::new();
    for h in &pending_refs {
        let label = h.urgency().label();
        *urgency_on_list.entry(label).or_insert(0) += 1;
    }

    let mut per_platform: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    for h in items {
        let bucket = per_platform.entry(h.platform.as_str()).or_insert((0, 0, 0));
        bucket.0 += 1;
        if h.submitted {
            bucket.2 += 1;
        } else if !h.is_overdue() {
            bucket.1 += 1;
        }
    }

    let sum_buckets = done + pending_len + overdue_unsubmitted;
    let reconcile = if sum_buckets == total {
        format!("分项核对：已完成 + 未提交(列表) + 已过期未交 = {}", total)
    } else {
        format!(
            "分项核对异常：已完成({}) + 未提交({}) + 已过期未交({}) = {}，但总作业为 {}",
            done, pending_len, overdue_unsubmitted, sum_buckets, total
        )
    };

    let mut lines: Vec<String> = vec![
        "──────── 统计说明（与首页数字一致）────────".to_string(),
        String::new(),
        "四个数字的计算方式：".to_string(),
        "· 总作业：本次拉取、各平台合并后的条目总数。".to_string(),
        "· 未提交：未标记已提交且尚未过截止时间的条目（主页列表只展示这部分）。".to_string(),
        "· 紧急：在「未提交」中，截止时间距当前时刻还剩 ≤6 小时的条目。".to_string(),
        "· 已完成：已标记提交的条目数。".to_string(),
        String::new(),
        format!(
            "当前：总={}, 未提交={}, 紧急={}, 已完成={}",
            total, pending_len, urgent, done
        ),
        String::new(),
        reconcile,
        format!(
            "说明：已过截止但未交的 {} 条会计入「总作业」，但不会出现在卡片列表（列表仅「未提交」）。",
            overdue_unsubmitted
        ),
        String::new(),
    ];

    if pending_len > 0 {
        lines.push("列表中「未提交」条的紧迫度分布：".to_string());
        for (label, n) in &urgency_on_list {
            lines.push(format!("  · {}：{}", label, n));
        }
        lines.push(String::new());
    }

    if !per_platform.is_empty() {
        lines.push("按平台（总数 / 主页列表可数未提交 / 已完成）：".to_string());
        for (name, (t, pend, dn)) in &per_platform {
            lines.push(format!("  · {}：{} / {} / {}", name, t, pend, dn));
        }
        lines.push(String::new());
    }

    if overdue_unsubmitted > 0 {
        lines.push("已过截止但未交（调试明细）：".to_string());
        const OVERDUE_DETAIL_LIMIT: usize = 30;
        for h in overdue_unsubmitted_items.iter().take(OVERDUE_DETAIL_LIMIT) {
            let remain = h.remain_text().unwrap_or_else(|| "已过期".to_string());
            lines.push(format!(
                "  · [{}] {} | {} | 截止 {} | {}",
                h.platform,
                h.course,
                h.title,
                h.deadline_display(),
                remain
            ));
        }
        if overdue_unsubmitted > OVERDUE_DETAIL_LIMIT {
            lines.push(format!(
                "  · ... 其余 {} 条已省略",
                overdue_unsubmitted - OVERDUE_DETAIL_LIMIT
            ));
        }
    }

    lines.join("\n")
}

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
