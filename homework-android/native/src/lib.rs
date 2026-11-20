use homework_core::{
    homework_headline_counts, render_qr_png, AppConfig, ChaoxingClient, HomeworkItem,
    KetangpaiClient, YuketangClient,
};
use jni::{
    objects::{JClass, JObject, JString, JValue},
    sys::jstring,
    JNIEnv,
};
use serde_json::{json, Value};
use std::{io, panic::AssertUnwindSafe};

fn run_json(mut env: JNIEnv, run: impl FnOnce(&mut JNIEnv) -> Option<Value>) -> jstring {
    let value = std::panic::catch_unwind(AssertUnwindSafe(|| run(&mut env)))
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({"error": "本次操作未完成，请重试。"}));
    env.new_string(value.to_string())
        .map(JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn sync(platform: &str, config: AppConfig) -> io::Result<Vec<HomeworkItem>> {
    let login_error = || io::Error::new(io::ErrorKind::PermissionDenied, "login");
    match platform {
        "chaoxing" if config.chaoxing.enabled => {
            let c = config.chaoxing;
            let mut client = ChaoxingClient::new(&c.user, &c.password);
            if !client.login()? {
                return Err(login_error());
            }
            client.get_homework()
        }
        "ketangpai" if config.ketangpai.enabled => {
            let c = config.ketangpai;
            let mut client = KetangpaiClient::new(&c.email, &c.password);
            if !client.login()? {
                return Err(login_error());
            }
            client.get_homework_for_sync()
        }
        "yuketang" if config.yuketang.enabled => {
            let c = config.yuketang;
            let client = YuketangClient::new(&c.csrftoken, &c.sessionid, &c.university_id);
            if !client.is_logged_in() {
                return Err(login_error());
            }
            client.get_homework_for_sync()
        }
        _ => Err(io::Error::other("platform disabled")),
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_e7g_homeworksync_NativeBridge_fetch(
    env: JNIEnv,
    _class: JClass,
    platform: JString,
    config: JString,
) -> jstring {
    run_json(env, |env| {
        let platform: String = env.get_string(&platform).ok()?.into();
        let config: String = env.get_string(&config).ok()?.into();
        let result = match sync(&platform, serde_json::from_str(&config).ok()?) {
            Ok(items) => json!({"items": items}),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => json!({
                "error": if platform == "yuketang" {
                    "登录已失效，请重新扫码。"
                } else {
                    "登录失败，请检查账号和密码。"
                }
            }),
            // Upstream request errors may include query-string credentials.
            Err(_) => json!({"error": "暂时无法获取作业，请检查网络或稍后重试。"}),
        };
        Some(result)
    })
}

#[no_mangle]
pub extern "system" fn Java_io_github_e7g_homeworksync_NativeBridge_snapshot(
    env: JNIEnv,
    _class: JClass,
    items: JString,
) -> jstring {
    run_json(env, |env| {
        let items: String = env.get_string(&items).ok()?.into();
        Some(snapshot(serde_json::from_str(&items).ok()?))
    })
}

fn snapshot(mut items: Vec<HomeworkItem>) -> Value {
    items.sort_by_key(HomeworkItem::deadline_sort_key);
    let (total, pending, urgent, done) = homework_headline_counts(&items);
    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            let urgency = item.urgency();
            json!({
                "title": item.title, "course": item.course, "platform": item.platform,
                "deadline": item.deadline_display(), "remain": item.remain_text(),
                "state": if item.submitted { "done" } else if item.is_overdue() { "overdue" } else { "pending" },
                "label": urgency.label(), "url": item.url, "urgent": urgency == homework_core::Urgency::Urgent
            })
        })
        .collect();
    json!({"total": total, "pending": pending, "urgent": urgent, "done": done, "rows": rows})
}

#[no_mangle]
pub extern "system" fn Java_io_github_e7g_homeworksync_NativeBridge_qrLogin(
    env: JNIEnv,
    _class: JClass,
    university_id: JString,
    callback: JObject,
) -> jstring {
    run_json(env, |env| {
        let university_id: String = env.get_string(&university_id).ok()?.into();
        let vm = env.get_java_vm().ok()?;
        let vm_for_qr = env.get_java_vm().ok()?;
        let callback = env.new_global_ref(callback).ok()?;
        let callback_for_qr = callback.clone();
        let mut client = YuketangClient::new("", "", &university_id);
        let login = client.login_qrcode_with_cancel(
            move |value| {
                if let (Ok(png), Ok(mut env)) =
                    (render_qr_png(value), vm_for_qr.attach_current_thread())
                {
                    let _ = env.with_local_frame(8, |env| -> jni::errors::Result<()> {
                        let bytes = env.byte_array_from_slice(&png)?;
                        let _ = env.call_method(
                            callback_for_qr.as_obj(),
                            "onQr",
                            "([B)V",
                            &[JValue::Object(&bytes)],
                        );
                        if env.exception_check()? {
                            env.exception_clear()?;
                        }
                        Ok(())
                    });
                }
            },
            || {
                let Ok(mut env) = vm.attach_current_thread() else {
                    return true;
                };
                let result = env.call_method(callback.as_obj(), "isCancelled", "()Z", &[]);
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                result.and_then(|v| v.z()).unwrap_or(true)
            },
        );
        Some(if matches!(login, Ok(true)) {
            json!({
                "enabled": true, "university_id": client.university_id(),
                "csrftoken": client.csrftoken(), "sessionid": client.sessionid()
            })
        } else {
            json!({"error": "扫码登录未完成，请检查网络后重试。"})
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_counts_and_sorts_cached_items() {
        let item = |title: &str, deadline: Option<&str>, submitted| {
            json!({
                "title": title, "platform": "课堂派", "course": "课程",
                "deadline": deadline, "submitted": submitted,
                "url": "https://w.ketangpai.com/"
            })
        };
        let data = json!([
            item("不限时", None, false),
            item("未来", Some("9999-01-01T12:00:00"), false),
            item("已交", Some("2000-02-01T12:00:00"), true),
            item("已过期", Some("2000-01-01T12:00:00"), false),
        ]);
        let result = snapshot(serde_json::from_value(data).unwrap());
        assert_eq!(result["total"], 4);
        assert_eq!(result["pending"], 2);
        assert_eq!(result["done"], 1);
        assert_eq!(result["rows"][0]["state"], "overdue");
        assert_eq!(result["rows"][1]["state"], "done");
        assert_eq!(result["rows"][2]["title"], "未来");
        assert_eq!(result["rows"][3]["title"], "不限时");
    }

    #[test]
    fn disabled_platform_does_not_contact_network() {
        assert!(sync("yuketang", AppConfig::default()).is_err());
        assert!(sync("chaoxing", AppConfig::default()).is_err());
        assert!(sync("ketangpai", AppConfig::default()).is_err());
    }
}
