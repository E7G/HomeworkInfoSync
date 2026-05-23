use std::cell::RefCell;

thread_local! {
    static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn begin_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
}

pub fn end_capture() -> String {
    CAPTURE.with(|c| c.borrow_mut().take().unwrap_or_default())
}

pub fn log_line(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    eprintln!("{msg}");
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            buf.push_str(msg);
            if !msg.ends_with('\n') {
                buf.push('\n');
            }
        }
    });
}
