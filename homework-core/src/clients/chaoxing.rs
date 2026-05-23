use crate::clients::UA;
use crate::models::HomeworkItem;
use chrono::{Duration, Local, NaiveDateTime};
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::io;
use url::Url;

pub struct ChaoxingClient {
    user: String,
    password: String,
    client: Client,
    courses: Vec<CxCourse>,
}

#[derive(Clone)]
struct CxCourse {
    course_id: String,
    #[allow(dead_code)]
    course_name: String,
    clazz_id: String,
    cpi: String,
}

struct HwRow {
    title: String,
    course: String,
    uncommitted: bool,
    deadline: Option<NaiveDateTime>,
    work_id: String,
    course_id: String,
    clazz_id: String,
    raw: String,
}

struct ExamRow {
    title: String,
    expired: bool,
    finished: bool,
    deadline: Option<NaiveDateTime>,
    exam_id: String,
    course_id: String,
    class_id: String,
    raw: String,
}

impl ChaoxingClient {
    const PLATFORM: &'static str = "超星";

    pub fn new(user: &str, password: &str) -> Self {
        let client = Client::builder()
            .user_agent(UA)
            .cookie_store(true)
            .build()
            .expect("http client");
        Self {
            user: user.to_string(),
            password: password.to_string(),
            client,
            courses: Vec::new(),
        }
    }

    pub fn login(&self) -> io::Result<bool> {
        let resp = self
            .client
            .get("https://passport2.chaoxing.com/api/login")
            .query(&[
                ("name", self.user.as_str()),
                ("pwd", self.password.as_str()),
                ("verify", "0"),
                ("schoolid", ""),
            ])
            .send()
            .map_err(io::Error::other)?;
        let data: serde_json::Value = resp.json().map_err(io::Error::other)?;
        if data.get("result").and_then(|v| v.as_bool()) == Some(true) {
            crate::hw_log!("[超星] 登录成功");
            Ok(true)
        } else {
            let msg = data
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            crate::hw_log!("[超星] 登录失败: {msg}");
            Ok(false)
        }
    }

    fn get_courses(&mut self) -> io::Result<Vec<CxCourse>> {
        if !self.courses.is_empty() {
            return Ok(self.courses.clone());
        }
        let resp = self
            .client
            .get("https://mooc1-api.chaoxing.com/mycourse/backclazzdata?view=json&mcode=")
            .send()
            .map_err(io::Error::other)?;
        let data: serde_json::Value = resp.json().map_err(io::Error::other)?;
        let Some(channels) = data.get("channelList").and_then(|v| v.as_array()) else {
            crate::hw_log!("[超星] 课程列表为空");
            return Ok(vec![]);
        };
        let mut courses = Vec::new();
        for channel in channels {
            let content = match channel.get("content") {
                Some(c) => c,
                None => continue,
            };
            if content.get("state").and_then(|v| v.as_i64()) == Some(1) {
                continue;
            }
            let course_data = content
                .pointer("/course/data/0")
                .or_else(|| content.pointer("/course/data"));
            let course_info = match course_data {
                Some(v) if v.is_object() => v,
                Some(v) if v.is_array() => match v.get(0) {
                    Some(c) => c,
                    None => continue,
                },
                _ => continue,
            };
            let clazz_id = content
                .pointer("/clazz/data/0/id")
                .or_else(|| content.get("id"))
                .or_else(|| channel.get("key"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_default();
            if clazz_id.is_empty() {
                continue;
            }
            let course_id = course_info
                .get("id")
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_default();
            let course_name = course_info
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("未知课程")
                .to_string();
            let cpi = content
                .get("cpi")
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_default();
            if !course_id.is_empty() {
                courses.push(CxCourse {
                    course_id,
                    course_name,
                    clazz_id,
                    cpi,
                });
            }
        }
        crate::hw_log!("[超星] 获取到 {} 门课程", courses.len());
        self.courses = courses.clone();
        Ok(courses)
    }

    fn parse_left_time(left_time: &str) -> Option<NaiveDateTime> {
        if left_time.is_empty() {
            return None;
        }
        let now = Local::now().naive_local();
        let re = Regex::new(r"(\d+)").ok()?;
        if left_time.contains("小时") {
            let hours: i64 = re.captures(left_time)?.get(1)?.as_str().parse().ok()?;
            return now.checked_add_signed(Duration::hours(hours));
        }
        if left_time.contains("天") {
            let days: i64 = re.captures(left_time)?.get(1)?.as_str().parse().ok()?;
            return now.checked_add_signed(Duration::days(days));
        }
        if left_time.contains("分钟") || left_time.contains("分") {
            let minutes: i64 = re.captures(left_time)?.get(1)?.as_str().parse().ok()?;
            return now.checked_add_signed(Duration::minutes(minutes));
        }
        None
    }

    fn fix_url(url: &str) -> String {
        if url.is_empty() {
            return String::new();
        }
        let url = url.replace("mooc1-api.chaoxing.com", "mooc1.chaoxing.com");
        if url.starts_with("http") {
            url
        } else {
            format!("https://mooc1.chaoxing.com{url}")
        }
    }

    fn query_params(raw: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let full = if raw.starts_with("http") {
            raw.to_string()
        } else {
            format!("https://mooc1.chaoxing.com{raw}")
        };
        if let Ok(parsed) = Url::parse(&full) {
            for (k, v) in parsed.query_pairs() {
                map.insert(k.into_owned(), v.into_owned());
            }
        }
        map
    }

    fn extract_homework(html: &str) -> Vec<HwRow> {
        let doc = Html::parse_document(html);
        let li_sel = Selector::parse("ul.nav > li").unwrap();
        let option_sel = Selector::parse(r#"div[role="option"]"#).unwrap();
        let mut items = Vec::new();
        for li in doc.select(&li_sel) {
            let option = match li.select(&option_sel).next() {
                Some(o) => o,
                None => continue,
            };
            let title = option
                .select(&Selector::parse("p").unwrap())
                .next()
                .map(|p| p.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let spans: Vec<_> = option
                .select(&Selector::parse("span").unwrap())
                .collect();
            let uncommitted = spans.first().map(|s| {
                s.value()
                    .attr("class")
                    .map(|c| c.contains("status"))
                    .unwrap_or(false)
            }).unwrap_or(false);
            let course = spans
                .get(1)
                .map(|s| s.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let left_time = option
                .select(&Selector::parse(".fr").unwrap())
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let raw = li.value().attr("data").unwrap_or("").to_string();
            let qs = Self::query_params(&raw);
            items.push(HwRow {
                title,
                course,
                uncommitted,
                deadline: Self::parse_left_time(&left_time),
                work_id: qs.get("taskrefId").cloned().unwrap_or_default(),
                course_id: qs.get("courseId").cloned().unwrap_or_default(),
                clazz_id: qs.get("clazzId").cloned().unwrap_or_default(),
                raw,
            });
        }
        items
    }

    fn extract_exams(html: &str) -> Vec<ExamRow> {
        let doc = Html::parse_document(html);
        let li_sel = Selector::parse("ul.ks_list > li").unwrap();
        let mut items = Vec::new();
        for li in doc.select(&li_sel) {
            let dl = li.select(&Selector::parse("dl").unwrap()).next();
            let (title, time_left) = if let Some(dl) = dl {
                let title = dl
                    .select(&Selector::parse("dt").unwrap())
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                let time_left = dl
                    .select(&Selector::parse("dd").unwrap())
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                (title, time_left)
            } else {
                (String::new(), String::new())
            };
            let expired = li
                .select(&Selector::parse("div.ks_pic > img").unwrap())
                .next()
                .and_then(|img| img.value().attr("src"))
                .map(|s| s.contains("ks_02"))
                .unwrap_or(false);
            let status = li
                .select(&Selector::parse("span.ks_state").unwrap())
                .next()
                .map(|e| e.text().collect::<String>())
                .unwrap_or_default();
            let finished = status.contains("已完成") || status.contains("待批阅");
            let raw = li.value().attr("data").unwrap_or("").to_string();
            let full_raw = if raw.starts_with("http") {
                raw.clone()
            } else {
                format!("https://mooc1.chaoxing.com{raw}")
            }
            .replace("mooc1-api.chaoxing.com", "mooc1.chaoxing.com");
            let qs = Self::query_params(&full_raw);
            items.push(ExamRow {
                title,
                expired,
                finished,
                deadline: Self::parse_left_time(&time_left),
                exam_id: qs.get("taskrefId").cloned().unwrap_or_default(),
                course_id: qs.get("courseId").cloned().unwrap_or_default(),
                class_id: qs.get("classId").cloned().unwrap_or_default(),
                raw,
            });
        }
        items
    }

    fn extract_exams_table(html: &str) -> Vec<ExamRow> {
        let doc = Html::parse_document(html);
        let row_sel = Selector::parse("table.dataTable tr.dataTr").unwrap();
        let re_mooc = Regex::new(r"moocId=(\d+)").unwrap();
        let re_clazz = Regex::new(r"clazzid=(\d+)").unwrap();
        let re_exam = Regex::new(r"examId=(\d+)").unwrap();
        let re_date = Regex::new(r"(\d{4}-\d{2}-\d{2}\s*\d{2}:\d{2})").unwrap();
        let mut items = Vec::new();
        for row in doc.select(&row_sel) {
            let cells: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
            if cells.len() < 9 {
                continue;
            }
            let title = cells[1].text().collect::<String>().trim().to_string();
            let time_range = cells[2].text().collect::<String>();
            let exam_status = cells[4].text().collect::<String>();
            let answer_status = cells[5].text().collect::<String>();
            let expired = exam_status.contains("已结束");
            let finished = answer_status.contains("已完成") || answer_status.contains("待批阅");
            let onclick = cells[8]
                .select(&Selector::parse("a").unwrap())
                .next()
                .and_then(|a| a.value().attr("onclick"))
                .unwrap_or("");
            let course_id = re_mooc
                .captures(onclick)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
            let class_id = re_clazz
                .captures(onclick)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
            let exam_id = re_exam
                .captures(onclick)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
            let deadline = re_date.captures(&time_range).and_then(|c| {
                NaiveDateTime::parse_from_str(c[1].trim(), "%Y-%m-%d %H:%M").ok()
            });
            items.push(ExamRow {
                title,
                expired,
                finished,
                deadline,
                exam_id,
                course_id,
                class_id,
                raw: String::new(),
            });
        }
        items
    }

    fn build_homework_url(course_id: &str, clazz_id: &str) -> String {
        format!(
            "https://mooc1.chaoxing.com/visit/stucoursemiddle?ismooc2=1&courseid={course_id}&clazzid={clazz_id}&pageHeader=8"
        )
    }

    fn build_exam_url(course_id: &str, class_id: &str, exam_id: &str) -> String {
        format!(
            "https://mooc1.chaoxing.com/exam-ans/exam/test/examcode/examnotes?courseId={course_id}&classId={class_id}&examId={exam_id}"
        )
    }

    fn get_work_urls(&self, course: &CxCourse) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let visit = match self
            .client
            .get("https://mooc1.chaoxing.com/visit/stucoursemiddle")
            .query(&[
                ("ismooc2", "1"),
                ("courseid", course.course_id.as_str()),
                ("clazzid", course.clazz_id.as_str()),
                ("cpi", course.cpi.as_str()),
            ])
            .send()
        {
            Ok(r) => r,
            Err(_) => return map,
        };
        let html = visit.text().unwrap_or_default();
        let doc = Html::parse_document(&html);
        let work_enc = doc
            .select(&Selector::parse("#workEnc").unwrap())
            .next()
            .and_then(|e| e.value().attr("value"))
            .unwrap_or("");
        if work_enc.is_empty() {
            return map;
        }
        let work_resp = match self
            .client
            .get("https://mooc1.chaoxing.com/mooc2/work/list")
            .query(&[
                ("courseId", course.course_id.as_str()),
                ("classId", course.clazz_id.as_str()),
                ("cpi", course.cpi.as_str()),
                ("ut", "s"),
                ("enc", work_enc),
            ])
            .header(
                "Referer",
                format!(
                    "https://mooc1.chaoxing.com/visit/stucoursemiddle?ismooc2=1&courseid={}&clazzid={}",
                    course.course_id, course.clazz_id
                ),
            )
            .send()
        {
            Ok(r) => r,
            Err(_) => return map,
        };
        let work_html = work_resp.text().unwrap_or_default();
        let work_doc = Html::parse_document(&work_html);
        for li in work_doc.select(&Selector::parse("li").unwrap()) {
            let data_url = li.value().attr("data").unwrap_or("");
            if data_url.is_empty() {
                continue;
            }
            let title = li
                .select(&Selector::parse("p").unwrap())
                .next()
                .map(|p| p.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let full_url = Self::fix_url(data_url);
            let qs = Self::query_params(data_url);
            let work_id = qs.get("workId").cloned().unwrap_or_default();
            if !work_id.is_empty() {
                let key = format!("{}_{}", course.course_id, work_id);
                map.entry(key).or_insert(full_url);
            }
        }
        map
    }

    pub fn get_homework(&mut self) -> io::Result<Vec<HomeworkItem>> {
        let courses = self.get_courses()?;
        let course_map: HashMap<String, CxCourse> = courses
            .iter()
            .map(|c| (c.course_id.clone(), c.clone()))
            .collect();
        let mut homework = Vec::new();

        let hw_html = self
            .client
            .get("https://mooc1-api.chaoxing.com/work/stu-work")
            .send()
            .map_err(io::Error::other)?
            .text()
            .map_err(io::Error::other)?;
        let hw_items = Self::extract_homework(&hw_html);
        crate::hw_log!("[超星] 从作业页面获取到 {} 项作业", hw_items.len());

        let mut needed = HashSet::new();
        for item in &hw_items {
            if !item.course_id.is_empty() && !item.work_id.is_empty() {
                needed.insert(item.course_id.clone());
            }
        }
        let mut work_url_map = HashMap::new();
        for cid in needed {
            if let Some(course) = course_map.get(&cid) {
                work_url_map.extend(self.get_work_urls(course));
            }
        }

        for item in hw_items {
            let key = format!("{}_{}", item.course_id, item.work_id);
            let hw_url = work_url_map.get(&key).cloned().or_else(|| {
                if !item.course_id.is_empty() && !item.clazz_id.is_empty() {
                    Some(Self::build_homework_url(&item.course_id, &item.clazz_id))
                } else {
                    Some(Self::fix_url(&item.raw))
                }
            }).unwrap_or_default();
            homework.push(HomeworkItem {
                title: item.title,
                course: item.course,
                deadline: item.deadline,
                platform: Self::PLATFORM.to_string(),
                submitted: !item.uncommitted,
                url: hw_url,
            });
        }

        let mut seen_exams = HashSet::new();
        if let Ok(exam_html) = self
            .client
            .get("https://mooc1-api.chaoxing.com/exam-ans/exam/phone/examcode")
            .send()
            .and_then(|r| r.text())
        {
            for item in Self::extract_exams(&exam_html) {
                let key = if item.exam_id.is_empty() {
                    item.title.clone()
                } else {
                    item.exam_id.clone()
                };
                if !seen_exams.insert(key) || item.finished || item.expired {
                    continue;
                }
                let url = if !item.course_id.is_empty()
                    && !item.class_id.is_empty()
                    && !item.exam_id.is_empty()
                {
                    Self::build_exam_url(&item.course_id, &item.class_id, &item.exam_id)
                } else {
                    Self::fix_url(&item.raw)
                };
                homework.push(HomeworkItem {
                    title: item.title,
                    course: "考试".to_string(),
                    deadline: item.deadline,
                    platform: Self::PLATFORM.to_string(),
                    submitted: false,
                    url,
                });
            }
        }

        if let Ok(list_html) = self
            .client
            .get("https://mooc1.chaoxing.com/exam-ans/exam/test/examcode/examlist?edition=1&nohead=0&fid=")
            .send()
            .and_then(|r| r.text())
        {
            for item in Self::extract_exams_table(&list_html) {
                let key = if item.exam_id.is_empty() {
                    item.title.clone()
                } else {
                    item.exam_id.clone()
                };
                if !seen_exams.insert(key) || item.finished || item.expired {
                    continue;
                }
                let url = if !item.course_id.is_empty()
                    && !item.class_id.is_empty()
                    && !item.exam_id.is_empty()
                {
                    Self::build_exam_url(&item.course_id, &item.class_id, &item.exam_id)
                } else {
                    String::new()
                };
                homework.push(HomeworkItem {
                    title: item.title,
                    course: "考试".to_string(),
                    deadline: item.deadline,
                    platform: Self::PLATFORM.to_string(),
                    submitted: false,
                    url,
                });
            }
        }

        crate::hw_log!("[超星] 共获取到 {} 项作业/考试/任务", homework.len());
        Ok(homework)
    }
}
