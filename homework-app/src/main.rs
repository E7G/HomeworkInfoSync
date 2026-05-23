mod controller;
mod ffi;

use controller::AppController;
use ffi::{GetConfigJsonFn, OnWindowFn, PollFn, RefreshFn, SaveConfigFn, UiCallbacks, YktQrFn};
use std::env;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;

unsafe extern "C" fn on_window_cb(ctx: *mut c_void, window: *mut c_void) {
    let ctrl = &mut *(ctx as *mut AppController);
    ctrl.set_window(window);
}

unsafe extern "C" fn poll_cb(ctx: *mut c_void) {
    let ctrl = &mut *(ctx as *mut AppController);
    ctrl.poll();
}

unsafe extern "C" fn refresh_cb(ctx: *mut c_void, silent: i32) {
    let ctrl = &mut *(ctx as *mut AppController);
    ctrl.refresh(silent);
}

unsafe extern "C" fn save_config_cb(ctx: *mut c_void, json: *const c_char) {
    let ctrl = &mut *(ctx as *mut AppController);
    let text = if json.is_null() {
        ""
    } else {
        std::ffi::CStr::from_ptr(json).to_str().unwrap_or("")
    };
    ctrl.save_config_json(text);
}

unsafe extern "C" fn ykt_qr_cb(ctx: *mut c_void) {
    let ctrl = &mut *(ctx as *mut AppController);
    ctrl.ykt_qr_login();
}

unsafe extern "C" fn get_config_json_cb(ctx: *mut c_void, buf: *mut c_char, cap: i32) -> i32 {
    if buf.is_null() || cap <= 0 {
        return 0;
    }
    let ctrl = &mut *(ctx as *mut AppController);
    let slice = std::slice::from_raw_parts_mut(buf as *mut u8, cap as usize);
    ctrl.get_config_json(slice)
}

fn main() {
    let mut controller = AppController::new();
    let ctx = &mut controller as *mut AppController as *mut c_void;

    let cbs = UiCallbacks {
        ctx,
        on_window: Some(on_window_cb as OnWindowFn),
        poll: Some(poll_cb as PollFn),
        refresh: Some(refresh_cb as RefreshFn),
        save_config: Some(save_config_cb as SaveConfigFn),
        ykt_qr_login: Some(ykt_qr_cb as YktQrFn),
        get_config_json: Some(get_config_json_cb as GetConfigJsonFn),
    };

    let args: Vec<CString> = env::args()
        .map(|a| CString::new(a).unwrap_or_default())
        .collect();
    let argc = args.len() as i32;
    let mut argv: Vec<*mut c_char> = args
        .iter()
        .map(|a| a.as_ptr() as *mut c_char)
        .collect();
    argv.push(ptr::null_mut());

    // controller must stay alive for entire event loop
    let code = unsafe { ffi::ui_run(cbs, argc, argv.as_mut_ptr()) };
    std::process::exit(code);
}
