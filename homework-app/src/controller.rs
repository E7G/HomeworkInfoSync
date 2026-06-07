use crate::ffi::{
    ui_on_fetch_done, ui_on_log, ui_on_progress, ui_on_qr_png, ui_on_status, ui_on_ykt_status,
    ui_set_refresh_enabled, HwItemC, HwStatsC,
};
use homework_core::{
    fetch_all_homework, homework_stats_debug_report, load_config, load_homework_cache,
    pending_sorted_by_deadline, render_qr_png, save_config, save_yuketang_session, AppConfig,
    FetchResult, HomeworkItem, Urgency, YuketangClient,
};
use std::collections::VecDeque;
use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::mpsc;
use std::thread;

enum WorkerMsg {
    Progress(usize, usize, String),
    FetchDone(Vec<HomeworkItem>, String, bool),
    QrPng(Vec<u8>),
    YktStatus(String, bool),
}

pub struct AppController {
    window: *mut c_void,
    inbox: Option<mpsc::Receiver<WorkerMsg>>,
    pending_strings: VecDeque<CString>,
    started: bool,
}

impl AppController {
    pub fn new() -> Self {
        Self {
            window: std::ptr::null_mut(),
            inbox: None,
            pending_strings: VecDeque::new(),
            started: false,
        }
    }

    pub fn set_window(&mut self, window: *mut c_void) {
        self.window = window;
    }

    pub fn poll(&mut self) {
        if !self.started {
            self.started = true;
            self.load_cache_ui();
            if load_config().should_auto_refresh() {
                self.refresh(1);
            }
            return;
        }

        let mut messages = Vec::new();
        let mut disconnected = false;
        if let Some(rx) = self.inbox.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(msg) => messages.push(msg),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if disconnected {
            self.inbox = None;
        }

        let window = self.window;
        for msg in messages {
            match msg {
                WorkerMsg::Progress(step, total, text) => {
                    self.pending_strings
                        .push_back(CString::new(text).unwrap_or_default());
                    let ptr = self.pending_strings.back().unwrap().as_ptr();
                    unsafe {
                        ui_on_progress(window, step as i32, total as i32, ptr);
                    }
                }
                WorkerMsg::FetchDone(items, log, ykt_expired) => {
                    if ykt_expired {
                        self.pending_strings.push_back(
                            CString::new("凭证已过期，请重新扫码登录").unwrap_or_default(),
                        );
                        let ptr = self.pending_strings.back().unwrap().as_ptr();
                        unsafe {
                            ui_on_ykt_status(window, ptr);
                        }
                    }
                    self.apply_fetch_done(items, log, ykt_expired);
                }
                WorkerMsg::QrPng(png) => unsafe {
                    ui_on_qr_png(window, png.as_ptr(), png.len() as i32);
                },
                WorkerMsg::YktStatus(text, _saved) => {
                    self.pending_strings
                        .push_back(CString::new(text).unwrap_or_default());
                    let ptr = self.pending_strings.back().unwrap().as_ptr();
                    unsafe {
                        ui_on_ykt_status(window, ptr);
                    }
                }
            }
        }
    }

    pub fn refresh(&mut self, silent: i32) {
        if self.inbox.is_some() {
            return;
        }
        let status = if silent == 0 {
            "正在获取作业..."
        } else {
            "后台刷新中..."
        };
        self.pending_strings.push_back(CString::new(status).unwrap());
        let status_ptr = self.pending_strings.back().unwrap().as_ptr();
        self.pending_strings.push_back(CString::new("准备中...").unwrap());
        let progress_ptr = self.pending_strings.back().unwrap().as_ptr();
        let window = self.window;
        unsafe {
            ui_set_refresh_enabled(window, 0);
            ui_on_status(window, status_ptr);
            ui_on_progress(window, 0, 3, progress_ptr);
        }

        let (tx, rx) = mpsc::channel();
        self.inbox = Some(rx);
        thread::spawn(move || {
            let progress_tx = tx.clone();
            let progress = Some(Box::new(move |step, total, msg: &str| {
                let _ = progress_tx.send(WorkerMsg::Progress(step, total, msg.to_string()));
            }) as Box<dyn Fn(usize, usize, &str) + Send>);
            let FetchResult {
                items,
                log,
                yuketang_session_expired,
            } = fetch_all_homework(progress);
            let _ = tx.send(WorkerMsg::FetchDone(items, log, yuketang_session_expired));
        });
    }

    pub fn save_config_json(&mut self, json: &str) {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(json) {
            if save_config(&cfg).is_ok() {
                self.pending_strings
                    .push_back(CString::new("配置已保存").unwrap_or_default());
                let ptr = self.pending_strings.back().unwrap().as_ptr();
                unsafe {
                    ui_on_status(self.window, ptr);
                }
            }
        }
    }

    pub fn get_config_json(&mut self, buf: &mut [u8]) -> i32 {
        let text = serde_json::to_string(&load_config()).unwrap_or_else(|_| "{}".to_string());
        let bytes = text.as_bytes();
        let n = bytes.len().min(buf.len().saturating_sub(1));
        buf[..n].copy_from_slice(&bytes[..n]);
        buf[n] = 0;
        n as i32
    }

    pub fn ykt_qr_login(&mut self) {
        if self.inbox.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.inbox = Some(rx);
        thread::spawn(move || {
            let mut client = YuketangClient::new("", "", "3078");
            let qr_tx = tx.clone();
            let ok = client.login_qrcode(move |url| {
                if let Ok(png) = render_qr_png(url) {
                    let _ = qr_tx.send(WorkerMsg::QrPng(png));
                }
            });
            if ok.unwrap_or(false) {
                let mut cfg = load_config();
                let _ = save_yuketang_session(
                    &mut cfg,
                    client.csrftoken(),
                    client.sessionid(),
                );
                let _ = tx.send(WorkerMsg::YktStatus("登录成功！凭证已保存".to_string(), true));
            } else {
                let _ = tx.send(WorkerMsg::YktStatus("登录失败或超时".to_string(), false));
            }
        });
    }

    fn load_cache_ui(&mut self) {
        let (items, updated_at) = load_homework_cache();
        if !items.is_empty() {
            self.apply_fetch_done(items, String::new(), false);
            if let Some(ts) = updated_at {
                let msg = format!("已加载缓存（{}）", ts.format("%m-%d %H:%M"));
                self.pending_strings
                    .push_back(CString::new(msg).unwrap_or_default());
                let ptr = self.pending_strings.back().unwrap().as_ptr();
                unsafe {
                    ui_on_status(self.window, ptr);
                }
            }
        }
    }

    fn apply_fetch_done(&mut self, items: Vec<HomeworkItem>, log_text: String, ykt_expired: bool) {
        let (c_items, stats) = pack_items(&mut self.pending_strings, &items);
        let status_msg = if ykt_expired {
            format!(
                "获取完成 - 共 {} 项作业（长江雨课堂凭证已过期，请重新扫码）",
                items.len()
            )
        } else {
            format!("获取完成 - 共 {} 项作业", items.len())
        };
        self.pending_strings
            .push_back(CString::new(status_msg).unwrap_or_default());
        let status_ptr = self.pending_strings.back().unwrap().as_ptr();
        let base_log = if log_text.trim().is_empty() {
            "（无详细日志）".to_string()
        } else {
            log_text
        };
        let stats_block = homework_stats_debug_report(&items);
        let log_display = format!("{}\n\n{}", base_log.trim_end(), stats_block);
        self.pending_strings
            .push_back(CString::new(log_display).unwrap_or_default());
        let log_ptr = self.pending_strings.back().unwrap().as_ptr();
        let window = self.window;
        unsafe {
            ui_on_fetch_done(window, c_items.as_ptr(), c_items.len() as i32, stats);
            ui_on_log(window, log_ptr);
            ui_on_status(window, status_ptr);
            ui_set_refresh_enabled(window, 1);
        }
    }

}

fn pack_items(store: &mut VecDeque<CString>, items: &[HomeworkItem]) -> (Vec<HwItemC>, HwStatsC) {
    let pending = pending_sorted_by_deadline(items);
    let urgent = pending
        .iter()
        .filter(|h| h.urgency() == Urgency::Urgent)
        .count();
    let done = items.iter().filter(|h| h.submitted).count();

    let mut out = Vec::with_capacity(pending.len());
    for h in &pending {
        store.push_back(CString::new(h.title.as_str()).unwrap_or_default());
        let title = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.course.as_str()).unwrap_or_default());
        let course = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.platform.as_str()).unwrap_or_default());
        let platform = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.deadline_display()).unwrap_or_default());
        let deadline = store.back().unwrap().as_ptr();
        let remain_s = h.remain_text().unwrap_or_default();
        store.push_back(CString::new(remain_s).unwrap_or_default());
        let remain = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.urgency().color()).unwrap_or_default());
        let color = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.urgency().bg_color()).unwrap_or_default());
        let bg_color = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.urgency().label()).unwrap_or_default());
        let urgency_label = store.back().unwrap().as_ptr();
        store.push_back(CString::new(h.url.as_str()).unwrap_or_default());
        let url = store.back().unwrap().as_ptr();
        out.push(HwItemC {
            title,
            course,
            platform,
            deadline,
            remain,
            color,
            bg_color,
            urgency_label,
            url,
        });
    }

    (
        out,
        HwStatsC {
            total: items.len() as i32,
            pending: pending.len() as i32,
            urgent: urgent as i32,
            done: done as i32,
        },
    )
}
