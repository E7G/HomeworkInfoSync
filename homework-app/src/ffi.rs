#![allow(non_snake_case)]

use std::os::raw::{c_char, c_void};

#[repr(C)]
pub struct HwItemC {
    pub title: *const c_char,
    pub course: *const c_char,
    pub platform: *const c_char,
    pub deadline: *const c_char,
    pub remain: *const c_char,
    pub color: *const c_char,
    pub bg_color: *const c_char,
    pub urgency_label: *const c_char,
    pub url: *const c_char,
}

#[repr(C)]
pub struct HwStatsC {
    pub total: i32,
    pub pending: i32,
    pub urgent: i32,
    pub done: i32,
}

pub type OnWindowFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub type PollFn = unsafe extern "C" fn(*mut c_void);
pub type RefreshFn = unsafe extern "C" fn(*mut c_void, i32);
pub type SaveConfigFn = unsafe extern "C" fn(*mut c_void, *const c_char);
pub type YktQrFn = unsafe extern "C" fn(*mut c_void);
pub type GetConfigJsonFn = unsafe extern "C" fn(*mut c_void, *mut c_char, i32) -> i32;

#[repr(C)]
pub struct UiCallbacks {
    pub ctx: *mut c_void,
    pub on_window: Option<OnWindowFn>,
    pub poll: Option<PollFn>,
    pub refresh: Option<RefreshFn>,
    pub save_config: Option<SaveConfigFn>,
    pub ykt_qr_login: Option<YktQrFn>,
    pub get_config_json: Option<GetConfigJsonFn>,
}

extern "C" {
    pub fn ui_on_progress(window: *mut c_void, step: i32, total: i32, msg: *const c_char);
    pub fn ui_on_fetch_done(
        window: *mut c_void,
        items: *const HwItemC,
        count: i32,
        stats: HwStatsC,
    );
    pub fn ui_on_status(window: *mut c_void, msg: *const c_char);
    pub fn ui_on_log(window: *mut c_void, msg: *const c_char);
    pub fn ui_on_qr_png(window: *mut c_void, data: *const u8, len: i32);
    pub fn ui_on_ykt_status(window: *mut c_void, msg: *const c_char);
    pub fn ui_set_refresh_enabled(window: *mut c_void, enabled: i32);
    pub fn ui_run(cb: UiCallbacks, argc: i32, argv: *mut *mut c_char) -> i32;
}
