use crate::clients::UA;
use crate::models::HomeworkItem;
use chrono::{DateTime, Duration, Local, NaiveDateTime};
use futures_util::{SinkExt, StreamExt};
use reqwest::blocking::Client;
use serde_json::Value;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Parse API numeric fields that may be JSON numbers or numeric strings.
fn json_int(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .or_else(|| v.as_bool().map(i64::from))
}

fn json_timestamp_positive(v: Option<&Value>) -> bool {
    json_int(v).is_some_and(|n| n > 0)
}

/// Student submission state for learn-log homework (`actype=5` activities).
///
/// Aligns with [Raincourse `WorkStatus`](https://github.com/aglorice/Raincourse/blob/master/utils/schema.py):
/// `0`/`1` = not submitted, `2`/`3` = graded, `5` = committed, `6` = absent.
///
/// `status == 5` alone is not enough: partial answers are autosaved with `status` 5 but no
/// final submit timestamp (web UI treats submission as `!!problem.user.submit_time`).
fn yuketang_is_submitted(act: &Value) -> bool {
    match json_int(act.get("status")) {
        Some(2 | 3) => true,
        Some(5) => yuketang_has_final_submit(act),
        _ => false,
    }
}

fn yuketang_has_final_submit(act: &Value) -> bool {
    if act.get("is_submitted").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    for key in ["submit_time", "submitTime", "commit_time", "commitTime"] {
        if json_timestamp_positive(act.get(key)) {
            return true;
        }
    }
    if let Some(user) = act.get("user") {
        if json_timestamp_positive(user.get("submit_time")) || json_timestamp_positive(user.get("submitTime"))
        {
            return true;
        }
    }
    false
}

pub struct YuketangClient {
    csrftoken: String,
    sessionid: String,
    university_id: String,
    client: Client,
    logged_in: bool,
}

impl YuketangClient {
    const PLATFORM: &'static str = "长江雨课堂";

    pub fn new(csrftoken: &str, sessionid: &str, university_id: &str) -> Self {
        let client = Client::builder()
            .user_agent(UA)
            .cookie_store(true)
            .build()
            .expect("http client");
        let logged_in = !csrftoken.is_empty() && !sessionid.is_empty();
        let ykt = Self {
            csrftoken: csrftoken.to_string(),
            sessionid: sessionid.to_string(),
            university_id: university_id.to_string(),
            client,
            logged_in,
        };
        if logged_in {
            ykt.apply_cookies();
        }
        ykt
    }

    pub fn csrftoken(&self) -> &str {
        &self.csrftoken
    }

    pub fn sessionid(&self) -> &str {
        &self.sessionid
    }

    pub fn university_id(&self) -> &str {
        &self.university_id
    }

    pub fn is_logged_in(&self) -> bool {
        self.logged_in
    }

    fn apply_cookies(&self) {
        let _ = self.client.get("https://changjiang.yuketang.cn/web");
    }

    fn api_headers(&self, classroom_id: &str) -> Vec<(&'static str, String)> {
        let cookie = if classroom_id.is_empty() {
            format!(
                "csrftoken={}; sessionid={}; university_id={}; platform_id=3",
                self.csrftoken, self.sessionid, self.university_id
            )
        } else {
            format!(
                "csrftoken={}; sessionid={}; classroom_id={}; classroomId={}; university_id={}; platform_id=3",
                self.csrftoken, self.sessionid, classroom_id, classroom_id, self.university_id
            )
        };
        let mut headers = vec![
            ("X-Csrftoken", self.csrftoken.clone()),
            ("xtbz", "ykt".to_string()),
            ("xt-agent", "web".to_string()),
            ("Referer", "https://changjiang.yuketang.cn/".to_string()),
            ("Cookie", cookie),
        ];
        if !classroom_id.is_empty() {
            headers.push(("classroom-id", classroom_id.to_string()));
        }
        headers
    }

    pub fn login_qrcode<F>(&mut self, on_qrcode: F) -> io::Result<bool>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let _ = self
            .client
            .get("https://changjiang.yuketang.cn/web")
            .send()
            .map_err(io::Error::other)?;

        let result = Arc::new(Mutex::new(QrLoginResult::default()));
        let result_ws = Arc::clone(&result);
        let on_qrcode = Arc::new(on_qrcode);

        let rt = Runtime::new().map_err(io::Error::other)?;
        let ok = rt.block_on(async move {
            let (ws, _) = connect_async("wss://changjiang.yuketang.cn/wsapp/")
                .await
                .map_err(io::Error::other)?;
            let (mut write, mut read) = ws.split();

            write
                .send(Message::Text(
                    serde_json::json!({
                        "op": "requestlogin",
                        "role": "web",
                        "version": 1.4,
                        "type": "qrcode",
                        "from": "web",
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .map_err(io::Error::other)?;

            let deadline = tokio::time::Instant::now() + StdDuration::from_secs(120);
            loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let msg = tokio::time::timeout(remaining, read.next())
                    .await
                    .map_err(io::Error::other)?;
                let Some(Ok(Message::Text(text))) = msg else {
                    break;
                };
                let data: Value = serde_json::from_str(&text).map_err(io::Error::other)?;
                if let Some(url) = data.get("qrcode").and_then(|v| v.as_str()) {
                    on_qrcode(url);
                    crate::hw_log!("[雨课堂] 请使用微信雨课堂小程序扫描二维码登录");
                }
                if data.get("subscribe_status").and_then(|v| v.as_bool()) == Some(true) {
                    let mut guard = result_ws.lock().unwrap();
                    guard.success = true;
                    guard.user_id = data
                        .get("UserID")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    guard.auth = data.get("Auth").and_then(|v| v.as_str()).map(str::to_string);
                    let name = data.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    let school = data.get("School").and_then(|v| v.as_str()).unwrap_or("");
                    crate::hw_log!("[雨课堂] 扫码登录成功！姓名: {name}，学校: {school}");
                    break;
                }
            }
            Ok::<(), io::Error>(())
        });

        if ok.is_err() {
            crate::hw_log!("[雨课堂] WebSocket 连接失败");
            return Ok(false);
        }

        let login_data = result.lock().unwrap().clone();
        if !login_data.success {
            crate::hw_log!("[雨课堂] 扫码登录超时或失败");
            return Ok(false);
        }

        let body = serde_json::json!({
            "UserID": login_data.user_id,
            "Auth": login_data.auth,
        });
        let resp = self
            .client
            .post("https://changjiang.yuketang.cn/pc/web_login")
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            crate::hw_log!("[雨课堂] 登录凭证获取失败");
            return Ok(false);
        }

        for cookie in resp.cookies() {
            match cookie.name() {
                "csrftoken" => self.csrftoken = cookie.value().to_string(),
                "sessionid" => self.sessionid = cookie.value().to_string(),
                "university_id" => self.university_id = cookie.value().to_string(),
                _ => {}
            }
        }
        self.logged_in = true;
        crate::hw_log!("[雨课堂] 登录凭证获取成功");
        Ok(true)
    }

    fn parse_timestamp(ts: Option<&Value>) -> Option<NaiveDateTime> {
        let raw = ts?.as_i64().or_else(|| ts.and_then(|v| v.as_str()?.parse().ok()))?;
        let val = if raw > 1_000_000_000_000 {
            raw / 1000
        } else {
            raw
        };
        DateTime::from_timestamp(val, 0).map(|dt| dt.naive_utc())
    }

    fn get_courses(&self) -> io::Result<Vec<YktCourse>> {
        let url = "https://changjiang.yuketang.cn/v2/api/web/courses/list?identity=2";
        let mut req = self.client.get(url);
        for (k, v) in self.api_headers("") {
            req = req.header(k, v);
        }
        let resp = req.send().map_err(io::Error::other)?;
        let data: Value = resp.json().map_err(io::Error::other)?;
        let now = Local::now().naive_local();
        let mut courses = Vec::new();
        for item in data
            .pointer("/data/list")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let course_info = item.get("course");
            let classroom_id = item
                .get("classroom_id")
                .map(|v| v.to_string().trim_matches('"').to_string())
                .filter(|s| !s.is_empty());
            let (Some(course_info), Some(classroom_id)) = (course_info, classroom_id) else {
                continue;
            };
            if let Some(end_time) = item
                .get("end_time")
                .or_else(|| course_info.get("end_time"))
            {
                if let Some(end_dt) = Self::parse_timestamp(Some(end_time)) {
                    if end_dt < now {
                        continue;
                    }
                }
            }
            if let Some(course_time) = item.get("time") {
                if let Some(start_dt) = Self::parse_timestamp(Some(course_time)) {
                    if (now - start_dt).num_days() > 180 {
                        continue;
                    }
                }
            }
            let course_name = course_info
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("未知课程")
                .to_string();
            courses.push(YktCourse {
                course_name,
                classroom_id,
            });
        }
        crate::hw_log!("[长江雨课堂] 获取到 {} 门课程", courses.len());
        Ok(courses)
    }

    pub fn get_homework(&self) -> io::Result<Vec<HomeworkItem>> {
        let mut homework = Vec::new();
        for course in self.get_courses()? {
            let url = format!(
                "https://changjiang.yuketang.cn/v2/api/web/logs/learn/{}?actype=5&page=0&offset=50&sort=-1",
                course.classroom_id
            );
            let mut req = self.client.get(&url);
            for (k, v) in self.api_headers(&course.classroom_id) {
                req = req.header(k, v);
            }
            let resp = match req.send() {
                Ok(r) => r,
                Err(e) => {
                    crate::hw_log!(
                        "[长江雨课堂] 获取课程 '{}' 作业时出错: {e}",
                        course.course_name
                    );
                    continue;
                }
            };
            let data: Value = match resp.json() {
                Ok(v) => v,
                Err(e) => {
                    crate::hw_log!(
                        "[长江雨课堂] 获取课程 '{}' 作业时出错: {e}",
                        course.course_name
                    );
                    continue;
                }
            };
            for act in data
                .pointer("/data/activities")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                let submitted = yuketang_is_submitted(act);
                let title = act
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知作业")
                    .to_string();
                let courseware_id = act
                    .get("courseware_id")
                    .map(|v| v.to_string().trim_matches('"').to_string())
                    .unwrap_or_default();
                let mut deadline = Self::parse_timestamp(act.get("end_time"))
                    .or_else(|| Self::parse_timestamp(act.get("close_time")))
                    .or_else(|| Self::parse_timestamp(act.get("deadline")));
                if deadline.is_none() {
                    if let (Some(begin), Some(duration)) = (
                        Self::parse_timestamp(act.get("begin_time")),
                        act.get("duration").and_then(|v| v.as_i64()),
                    ) {
                        deadline = begin.checked_add_signed(Duration::seconds(duration));
                    }
                }
                if submitted
                    && deadline
                        .map(|d| d < Local::now().naive_local())
                        .unwrap_or(false)
                {
                    continue;
                }
                let url = if courseware_id.is_empty() {
                    format!(
                        "https://changjiang.yuketang.cn/v2/web/studentLog/{}",
                        course.classroom_id
                    )
                } else {
                    format!(
                        "https://changjiang.yuketang.cn/v2/web/trans/{}/{}?status=1",
                        course.classroom_id, courseware_id
                    )
                };
                homework.push(HomeworkItem {
                    title,
                    course: course.course_name.clone(),
                    deadline,
                    platform: Self::PLATFORM.to_string(),
                    submitted,
                    url,
                });
            }
        }
        crate::hw_log!("[长江雨课堂] 共获取到 {} 项作业", homework.len());
        Ok(homework)
    }
}

#[derive(Clone, Default)]
struct QrLoginResult {
    success: bool,
    user_id: Option<String>,
    auth: Option<String>,
}

struct YktCourse {
    course_name: String,
    classroom_id: String,
}

pub fn render_qr_png(url: &str) -> io::Result<Vec<u8>> {
    use image::Luma;
    use qrcode::QrCode;
    let code = QrCode::new(url.as_bytes()).map_err(io::Error::other)?;
    let image = code.render::<Luma<u8>>().min_dimensions(200, 200).build();
    let mut buf = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        )
        .map_err(io::Error::other)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_not_submitted_is_pending() {
        assert!(!yuketang_is_submitted(&json!({ "status": 0 })));
        assert!(!yuketang_is_submitted(&json!({ "status": 1 })));
    }

    #[test]
    fn status_graded_is_submitted() {
        assert!(yuketang_is_submitted(&json!({ "status": 2 })));
        assert!(yuketang_is_submitted(&json!({ "status": 3 })));
    }

    #[test]
    fn status_five_without_submit_time_is_partial() {
        let partial = json!({ "status": 5, "title": "quiz" });
        assert!(!yuketang_is_submitted(&partial));
    }

    #[test]
    fn status_five_with_submit_time_is_submitted() {
        let done = json!({ "status": 5, "submit_time": 1_700_000_000_000_i64 });
        assert!(yuketang_is_submitted(&done));
    }

    #[test]
    fn status_five_with_is_submitted_flag() {
        let done = json!({ "status": 5, "is_submitted": true });
        assert!(yuketang_is_submitted(&done));
    }

    #[test]
    fn status_absent_is_pending() {
        assert!(!yuketang_is_submitted(&json!({ "status": 6 })));
    }
}
