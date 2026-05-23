use crate::clients::UA;
use crate::models::HomeworkItem;
use chrono::{DateTime, Datelike, Local, NaiveDateTime};
use reqwest::blocking::Client;
use serde_json::Value;
use std::io;

pub struct KetangpaiClient {
    email: String,
    password: String,
    client: Client,
    token: Option<String>,
}

impl KetangpaiClient {
    const PLATFORM: &'static str = "课堂派";
    const BASE_URL: &'static str = "https://openapiv5.ketangpai.com";

    pub fn new(email: &str, password: &str) -> Self {
        let client = Client::builder()
            .user_agent(UA)
            .build()
            .expect("http client");
        Self {
            email: email.to_string(),
            password: password.to_string(),
            client,
            token: None,
        }
    }

    fn timestamp_ms() -> i64 {
        Local::now().timestamp_millis()
    }

    pub fn login(&mut self) -> io::Result<bool> {
        let body = serde_json::json!({
            "email": self.email,
            "password": self.password,
            "remember": "0",
            "code": "",
            "mobile": "",
            "type": "login",
            "reqtimestamp": Self::timestamp_ms(),
        });
        let resp = self
            .client
            .post(format!("{}/UserApi/login", Self::BASE_URL))
            .header("Content-Type", "application/json;charset=UTF-8")
            .json(&body)
            .send()
            .map_err(io::Error::other)?;
        let data: Value = resp.json().map_err(io::Error::other)?;
        if data.get("message").and_then(|v| v.as_str()) == Some("访问成功") {
            self.token = data
                .pointer("/data/token")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            crate::hw_log!("[课堂派] 登录成功");
            Ok(true)
        } else {
            let msg = data
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            crate::hw_log!("[课堂派] 登录失败: {msg}");
            Ok(false)
        }
    }

    fn get_courses(&self) -> io::Result<Vec<Value>> {
        let token = self.token.as_deref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "课堂派未登录")
        })?;
        let body = serde_json::json!({
            "isstudy": "1",
            "search": "",
            "semester": "",
            "term": "",
            "reqtimestamp": Self::timestamp_ms(),
        });
        let resp = self
            .client
            .post(format!("{}/CourseApi/semesterCourseList", Self::BASE_URL))
            .header("Content-Type", "application/json;charset=UTF-8")
            .header("token", token)
            .json(&body)
            .send()
            .map_err(io::Error::other)?;
        let data: Value = resp.json().map_err(io::Error::other)?;
        if data.get("message").and_then(|v| v.as_str()) != Some("访问成功") {
            crate::hw_log!("[课堂派] 获取课程列表失败");
            return Ok(vec![]);
        }
        let all = data
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let now = Local::now();
        let current_term = if (2..=7).contains(&now.month()) {
            format!("{}-{}", now.year() - 1, now.year())
        } else {
            format!("{}-{}", now.year(), now.year() + 1)
        };
        let courses: Vec<Value> = all
            .into_iter()
            .filter(|c| c.get("semester").and_then(|v| v.as_str()) == Some(current_term.as_str()))
            .collect();
        crate::hw_log!(
            "[课堂派] 当前学期 {current_term} 有 {} 门课程",
            courses.len()
        );
        Ok(courses)
    }

    pub fn get_homework(&self) -> io::Result<Vec<HomeworkItem>> {
        let token = self.token.as_deref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "课堂派未登录")
        })?;
        let mut homework = Vec::new();
        let courses = self.get_courses()?;
        for course in courses {
            let course_id = course.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if course_id.is_empty() {
                continue;
            }
            let body = serde_json::json!({
                "contenttype": 4,
                "dirid": 0,
                "lessonlink": [],
                "sort": [],
                "page": 1,
                "limit": 50,
                "desc": 3,
                "courserole": 0,
                "vtr_type": "",
                "courseid": course_id,
                "reqtimestamp": Self::timestamp_ms(),
            });
            let resp = match self
                .client
                .post(format!(
                    "{}/FutureV2/CourseMeans/getCourseContent",
                    Self::BASE_URL
                ))
                .header("Content-Type", "application/json;charset=UTF-8")
                .header("token", token)
                .json(&body)
                .send()
            {
                Ok(r) => r,
                Err(e) => {
                    crate::hw_log!("[课堂派] 请求失败: {e}");
                    continue;
                }
            };
            let data: Value = match resp.json() {
                Ok(v) => v,
                Err(e) => {
                    crate::hw_log!("[课堂派] 解析失败: {e}");
                    continue;
                }
            };
            if data.get("message").and_then(|v| v.as_str()) != Some("访问成功") {
                continue;
            }
            let list = data
                .pointer("/data/list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let course_name = course
                .get("coursename")
                .or_else(|| course.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("未知课程")
                .to_string();
            for item in list {
                let deadline = item
                    .get("endtime")
                    .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                    .and_then(|ts| DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc()));
                let submitted = item.get("mstatus").and_then(|v| v.as_i64()) == Some(1);
                if submitted
                    && deadline
                        .map(|d| d < Local::now().naive_local())
                        .unwrap_or(false)
                {
                    continue;
                }
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知作业")
                    .to_string();
                let url = format!(
                    "https://w.ketangpai.com/homework?id={id}&courseId={course_id}&courseRole=0"
                );
                homework.push(HomeworkItem {
                    title,
                    course: course_name.clone(),
                    deadline,
                    platform: Self::PLATFORM.to_string(),
                    submitted,
                    url,
                });
            }
        }
        homework.sort_by_key(|h| h.deadline.unwrap_or(NaiveDateTime::MAX));
        crate::hw_log!("[课堂派] 共获取到 {} 项作业", homework.len());
        Ok(homework)
    }
}
