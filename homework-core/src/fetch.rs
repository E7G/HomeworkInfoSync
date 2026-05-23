use crate::cache::save_homework_cache;
use crate::clients::{ChaoxingClient, KetangpaiClient, YuketangClient};
use crate::config::{load_config, save_config, AppConfig};
use crate::hw_log;
use crate::log::{begin_capture, end_capture};
use crate::models::HomeworkItem;
use std::io::{self, Write};

pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

pub struct FetchResult {
    pub items: Vec<HomeworkItem>,
    pub log: String,
}

pub fn fetch_all_homework(progress: Option<ProgressCallback>) -> FetchResult {
    begin_capture();
    let result = fetch_all_homework_inner(progress);
    let mut log = end_capture();
    match result {
        Ok(items) => FetchResult { items, log },
        Err(e) => {
            log.push_str(&format!("\n错误: {e}\n"));
            FetchResult {
                items: Vec::new(),
                log,
            }
        }
    }
}

fn fetch_all_homework_inner(progress: Option<ProgressCallback>) -> io::Result<Vec<HomeworkItem>> {
    let config = load_config();
    let mut all = Vec::new();
    let steps = [
        ("chaoxing", "超星"),
        ("ketangpai", "课堂派"),
        ("yuketang", "长江雨课堂"),
    ];

    for (i, (key, label)) in steps.iter().enumerate() {
        if let Some(ref cb) = progress {
            cb(i, steps.len(), &format!("正在获取{label}..."));
        }

        match *key {
            "chaoxing" => {
                let c = &config.chaoxing;
                if c.enabled && !c.user.is_empty() && !c.password.is_empty() {
                    let mut client = ChaoxingClient::new(&c.user, &c.password);
                    if client.login()? {
                        all.extend(client.get_homework()?);
                    }
                } else {
                    hw_log!("[{label}] 未配置或未启用，跳过");
                }
            }
            "ketangpai" => {
                let c = &config.ketangpai;
                if c.enabled && !c.email.is_empty() && !c.password.is_empty() {
                    let mut client = KetangpaiClient::new(&c.email, &c.password);
                    if client.login()? {
                        all.extend(client.get_homework()?);
                    }
                } else {
                    hw_log!("[{label}] 未配置或未启用，跳过");
                }
            }
            "yuketang" => {
                let c = &config.yuketang;
                let client = YuketangClient::new(
                    &c.csrftoken,
                    &c.sessionid,
                    &c.university_id,
                );
                if client.is_logged_in() {
                    all.extend(client.get_homework()?);
                } else if c.enabled {
                    hw_log!("[{label}] 未登录，请在配置页扫码登录");
                } else {
                    hw_log!("[{label}] 未配置或未启用，跳过");
                }
            }
            _ => {}
        }
    }

    if let Some(ref cb) = progress {
        cb(steps.len(), steps.len(), "获取完成");
    }

    if !all.is_empty() {
        let _ = save_homework_cache(&all);
    }

    let _ = save_config(&config);
    let _ = io::stdout().flush();
    Ok(all)
}

pub fn save_yuketang_session(config: &mut AppConfig, csrftoken: &str, sessionid: &str) -> io::Result<()> {
    config.yuketang.csrftoken = csrftoken.to_string();
    config.yuketang.sessionid = sessionid.to_string();
    config.yuketang.enabled = true;
    save_config(config)
}
