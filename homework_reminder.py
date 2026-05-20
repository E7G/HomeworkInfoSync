# -*- coding: utf-8 -*-
import json
import os
import re
import sys
import webbrowser
from datetime import datetime, timedelta
from pathlib import Path

import requests
from bs4 import BeautifulSoup

CONFIG_PATH = Path(__file__).parent / "config.json"

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"


def load_config():
    if not CONFIG_PATH.exists():
        print(f"配置文件不存在: {CONFIG_PATH}")
        print("请参考 config.example.json 创建 config.json")
        sys.exit(1)
    with open(CONFIG_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


class HomeworkItem:
    def __init__(self, title, course, deadline, platform, submitted=False, url=""):
        self.title = title
        self.course = course
        self.deadline = deadline
        self.platform = platform
        self.submitted = submitted
        self.url = url

    @property
    def is_overdue(self):
        if self.deadline is None:
            return False
        return datetime.now() > self.deadline

    @property
    def urgency(self):
        if self.deadline is None:
            return "unknown"
        if self.is_overdue:
            return "overdue"
        delta = self.deadline - datetime.now()
        if delta <= timedelta(hours=6):
            return "urgent"
        if delta <= timedelta(days=1):
            return "soon"
        if delta <= timedelta(days=3):
            return "normal"
        return "relaxed"

    def __repr__(self):
        submitted_str = "已提交" if self.submitted else "未提交"
        deadline_str = self.deadline.strftime("%Y-%m-%d %H:%M") if self.deadline else "无截止时间"
        return f"[{self.platform}] {self.course} - {self.title} | 截止: {deadline_str} | {submitted_str}"


class ChaoXingClient:
    PLATFORM = "超星"

    API_COURSE_LIST = "https://mooc1-api.chaoxing.com/mycourse/backclazzdata?view=json&mcode="
    API_ACTIVITY_LIST = "https://mobilelearn.chaoxing.com/v2/apis/active/student/activelist"
    API_HOMEWORK_PAGE = "https://mooc1-api.chaoxing.com/work/stu-work"
    API_EXAM_PAGE = "https://mooc1-api.chaoxing.com/exam-ans/exam/phone/examcode"
    API_EXAM_LIST = "https://mooc1.chaoxing.com/exam-ans/exam/test/examcode/examlist?edition=1&nohead=0&fid="
    API_COURSE_VISIT = "https://mooc1.chaoxing.com/visit/stucoursemiddle"
    API_WORK_LIST = "https://mooc1.chaoxing.com/mooc2/work/list"

    ACTIVE_TYPE_MAP = {
        0: "签到", 2: "签到", 4: "抢答", 5: "主题讨论", 6: "投票",
        14: "问卷", 17: "直播", 23: "随堂练习", 35: "分组任务", 42: "随堂练习",
        43: "评分", 45: "拍照", 47: "作业", 64: "笔记",
    }

    def __init__(self, user, password):
        self.user = user
        self.password = password
        self.session = requests.Session()
        self.session.headers.update({"User-Agent": UA})
        self.courses = []

    def login(self):
        url = "https://passport2.chaoxing.com/api/login"
        params = {
            "name": self.user,
            "pwd": self.password,
            "verify": "0",
            "schoolid": "",
        }
        resp = self.session.get(url, params=params)
        data = resp.json()
        if data.get("result"):
            print(f"[超星] 登录成功")
            return True
        print(f"[超星] 登录失败: {data.get('msg', '未知错误')}")
        return False

    def get_courses(self):
        try:
            resp = self.session.get(self.API_COURSE_LIST)
            data = resp.json()
            if not data.get("channelList"):
                print("[超星] 课程列表为空")
                self.courses = []
                return []
            courses = []
            for channel in data["channelList"]:
                content = channel.get("content")
                if not content:
                    continue
                course_data = content.get("course", {}).get("data", [])
                if not course_data:
                    continue
                course_info = course_data[0]
                clazz_id = ""
                clazz_data = content.get("clazz", {}).get("data", [])
                if clazz_data:
                    clazz_id = str(clazz_data[0].get("id", ""))
                elif content.get("id"):
                    clazz_id = str(content["id"])
                elif channel.get("key"):
                    clazz_id = str(channel["key"])
                if content.get("state") == 1:
                    continue
                if course_info and clazz_id:
                    courses.append({
                        "courseId": str(course_info.get("id", "")),
                        "courseName": course_info.get("name", "未知课程"),
                        "clazzId": clazz_id,
                        "cpi": str(content.get("cpi", "")),
                    })
            self.courses = courses
            print(f"[超星] 获取到 {len(courses)} 门课程")
            return courses
        except Exception as e:
            print(f"[超星] 获取课程列表失败: {e}")
            self.courses = []
            return []

    def _get_activities(self, course):
        try:
            timestamp = int(datetime.now().timestamp() * 1000)
            url = f"{self.API_ACTIVITY_LIST}?fid=0&courseId={course['courseId']}&classId={course['clazzId']}&showNotStartedActive=0&_={timestamp}"
            resp = self.session.get(url)
            data = resp.json()
            active_list = None
            if isinstance(data.get("data"), dict):
                active_list = data["data"].get("activeList")
            elif isinstance(data.get("data"), list):
                active_list = data["data"]
            elif isinstance(data, list):
                active_list = data
            if not active_list:
                return []
            results = []
            for item in active_list:
                active_type = item.get("activeType", item.get("type", 0))
                type_name = self.ACTIVE_TYPE_MAP.get(active_type, f"类型{active_type}")
                is_ongoing = item.get("status") == 1
                is_ended = item.get("status") == 2
                deadline = None
                end_time = item.get("endTime")
                if end_time:
                    try:
                        deadline = datetime.fromtimestamp(int(end_time) / 1000)
                    except (ValueError, OSError):
                        try:
                            deadline = datetime.fromtimestamp(int(end_time))
                        except (ValueError, OSError):
                            pass
                results.append({
                    "title": item.get("nameOne", item.get("name", item.get("title", "未知任务"))),
                    "type": type_name,
                    "ongoing": is_ongoing,
                    "ended": is_ended,
                    "deadline": deadline,
                    "courseName": course["courseName"],
                    "courseId": course["courseId"],
                    "clazzId": course["clazzId"],
                    "activeType": active_type,
                })
            return results
        except Exception as e:
            print(f"[超星] 获取课程 '{course['courseName']}' 活动失败: {e}")
            return []

    def _extract_homework_from_html(self, html_text):
        soup = BeautifulSoup(html_text, "html.parser")
        items = []
        for li in soup.select("ul.nav > li"):
            option_el = li.select_one('div[role="option"]')
            if not option_el:
                continue
            p_el = option_el.select_one("p")
            title = p_el.get_text(strip=True) if p_el else ""
            spans = option_el.select("span")
            status = spans[0].get_text(strip=True) if len(spans) > 0 else ""
            uncommitted = len(spans) > 0 and "status" in (spans[0].get("class", []) if spans else [])
            course = spans[1].get_text(strip=True) if len(spans) > 1 else ""
            left_time_el = option_el.select_one(".fr")
            left_time = left_time_el.get_text(strip=True) if left_time_el else ""
            raw = li.get("data", "")
            work_id = ""
            course_id = ""
            clazz_id = ""
            if raw:
                try:
                    from urllib.parse import urlparse, parse_qs
                    parsed = urlparse(raw)
                    qs = parse_qs(parsed.query)
                    work_id = qs.get("taskrefId", [""])[0]
                    course_id = qs.get("courseId", [""])[0]
                    clazz_id = qs.get("clazzId", [""])[0]
                except Exception:
                    pass
            deadline = self._parse_left_time(left_time)
            items.append({
                "title": title,
                "course": course,
                "status": status,
                "uncommitted": uncommitted,
                "left_time": left_time,
                "deadline": deadline,
                "work_id": work_id,
                "course_id": course_id,
                "clazz_id": clazz_id,
                "raw": raw,
            })
        return items

    def _extract_exams_from_html(self, html_text):
        soup = BeautifulSoup(html_text, "html.parser")
        items = []
        for li in soup.select("ul.ks_list > li"):
            dl_el = li.select_one("dl")
            title = ""
            time_left = ""
            if dl_el:
                dt = dl_el.select_one("dt")
                dd = dl_el.select_one("dd")
                title = dt.get_text(strip=True) if dt else ""
                time_left = dd.get_text(strip=True) if dd else ""
            img_el = li.select_one("div.ks_pic > img")
            expired = False
            if img_el and img_el.get("src"):
                expired = "ks_02" in img_el["src"]
            status_el = li.select_one("span.ks_state")
            status = status_el.get_text(strip=True) if status_el else ""
            finished = "已完成" in status or "待批阅" in status
            raw = li.get("data", "")
            exam_id = ""
            course_id = ""
            class_id = ""
            if raw:
                try:
                    from urllib.parse import urlparse, parse_qs
                    full_raw = raw if raw.startswith("http") else f"https://mooc1.chaoxing.com{raw}"
                    full_raw = full_raw.replace("mooc1-api.chaoxing.com", "mooc1.chaoxing.com")
                    parsed = urlparse(full_raw)
                    qs = parse_qs(parsed.query)
                    exam_id = qs.get("taskrefId", [""])[0]
                    course_id = qs.get("courseId", [""])[0]
                    class_id = qs.get("classId", [""])[0]
                except Exception:
                    pass
            deadline = self._parse_left_time(time_left)
            items.append({
                "title": title,
                "time_left": time_left,
                "expired": expired,
                "status": status,
                "finished": finished,
                "deadline": deadline,
                "exam_id": exam_id,
                "course_id": course_id,
                "class_id": class_id,
                "raw": raw,
            })
        return items

    def _extract_exams_from_table(self, html_text):
        soup = BeautifulSoup(html_text, "html.parser")
        items = []
        for row in soup.select("table.dataTable tr.dataTr"):
            cells = row.select("td")
            if len(cells) < 9:
                continue
            title = cells[1].get_text(strip=True)
            time_range = cells[2].get_text(strip=True)
            exam_status = cells[4].get_text(strip=True)
            answer_status = cells[5].get_text(strip=True)
            expired = "已结束" in exam_status
            finished = "已完成" in answer_status or "待批阅" in answer_status
            action_link = cells[8].select_one("a")
            onclick_attr = action_link.get("onclick", "") if action_link else ""
            course_id = ""
            class_id = ""
            exam_id = ""
            mooc_match = re.search(r"moocId=(\d+)", onclick_attr)
            clazz_match = re.search(r"clazzid=(\d+)", onclick_attr)
            exam_id_match = re.search(r"examId=(\d+)", onclick_attr)
            if mooc_match:
                course_id = mooc_match.group(1)
            if clazz_match:
                class_id = clazz_match.group(1)
            if exam_id_match:
                exam_id = exam_id_match.group(1)
            deadline = None
            date_match = re.search(r"(\d{4}-\d{2}-\d{2}\s*\d{2}:\d{2})", time_range)
            if date_match:
                try:
                    deadline = datetime.strptime(date_match.group(1).strip(), "%Y-%m-%d %H:%M")
                except ValueError:
                    pass
            items.append({
                "title": title,
                "time_range": time_range,
                "expired": expired,
                "finished": finished,
                "deadline": deadline,
                "exam_id": exam_id,
                "course_id": course_id,
                "class_id": class_id,
            })
        return items

    def _parse_left_time(self, left_time):
        if not left_time:
            return None
        try:
            now = datetime.now()
            if "小时" in left_time:
                hours = int(re.search(r"(\d+)", left_time).group(1))
                return now + timedelta(hours=hours)
            if "天" in left_time:
                days = int(re.search(r"(\d+)", left_time).group(1))
                return now + timedelta(days=days)
            if "分钟" in left_time or "分" in left_time:
                minutes = int(re.search(r"(\d+)", left_time).group(1))
                return now + timedelta(minutes=minutes)
        except (ValueError, AttributeError):
            pass
        return None

    @staticmethod
    def _fix_url(url):
        if not url:
            return ""
        url = url.replace("mooc1-api.chaoxing.com", "mooc1.chaoxing.com")
        if not url.startswith("http"):
            url = f"https://mooc1.chaoxing.com{url}"
        return url

    def _build_homework_url(self, course_id, clazz_id):
        return f"{self.API_COURSE_VISIT}?ismooc2=1&courseid={course_id}&clazzid={clazz_id}&pageHeader=8"

    def _build_exam_url(self, course_id, class_id, exam_id):
        return f"https://mooc1.chaoxing.com/exam-ans/exam/test/examcode/examnotes?courseId={course_id}&classId={class_id}&examId={exam_id}"

    def _build_activity_url(self, course_id, clazz_id):
        return f"{self.API_COURSE_VISIT}?ismooc2=1&courseid={course_id}&clazzid={clazz_id}"

    def _get_work_enc_and_url(self, course):
        course_id = course["courseId"]
        clazz_id = course["clazzId"]
        cpi = course.get("cpi", "")
        course_name = course.get("courseName", "")
        try:
            resp = self.session.get(
                self.API_COURSE_VISIT,
                params={"ismooc2": "1", "courseid": course_id, "clazzid": clazz_id, "cpi": cpi},
            )
            soup = BeautifulSoup(resp.text, "html.parser")
            work_enc_input = soup.find("input", id="workEnc")
            if not work_enc_input or not work_enc_input.get("value"):
                print(f"[超星] 课程 {course_name}: 未找到 workEnc")
                return None
            work_enc = work_enc_input["value"]
            work_resp = self.session.get(
                self.API_WORK_LIST,
                params={
                    "courseId": course_id,
                    "classId": clazz_id,
                    "cpi": cpi,
                    "ut": "s",
                    "enc": work_enc,
                },
                headers={
                    "Host": "mooc1.chaoxing.com",
                    "Referer": f"https://mooc1.chaoxing.com/visit/stucoursemiddle?ismooc2=1&courseid={course_id}&clazzid={clazz_id}",
                },
            )
            work_soup = BeautifulSoup(work_resp.text, "html.parser")
            work_items = []
            for li in work_soup.find_all("li"):
                data_url = li.get("data", "")
                title_el = li.find("p")
                title = title_el.get_text(strip=True) if title_el else ""
                status_el = title_el.find_next("p") if title_el else None
                status = status_el.get_text(strip=True) if status_el else ""
                if data_url and title:
                    full_url = data_url if data_url.startswith("http") else f"https://mooc1.chaoxing.com{data_url}"
                    full_url = full_url.replace("mooc1-api.chaoxing.com", "mooc1.chaoxing.com")
                    work_id = ""
                    try:
                        from urllib.parse import urlparse, parse_qs
                        parsed = urlparse(data_url)
                        qs = parse_qs(parsed.query)
                        work_id = qs.get("workId", [""])[0]
                    except Exception:
                        pass
                    work_items.append({
                        "title": title,
                        "status": status,
                        "url": full_url,
                        "work_id": work_id,
                        "course_id": course_id,
                    })
            print(f"[超星] 课程 {course_name}: 获取到 {len(work_items)} 个作业URL")
            return work_items if work_items else None
        except Exception as e:
            print(f"[超星] 获取课程 {course_name} 作业列表失败: {e}")
            return None

    def get_homework(self):
        homework_list = []

        if not self.courses:
            self.get_courses()

        course_map = {c["courseId"]: c for c in self.courses if c.get("courseId")}

        try:
            resp = self.session.get(self.API_HOMEWORK_PAGE)
            hw_items = self._extract_homework_from_html(resp.text)
        except Exception as e:
            print(f"[超星] 获取作业页面失败: {e}")
            hw_items = []

        needed_course_ids = set()
        for item in hw_items:
            cid = item.get("course_id", "")
            wid = item.get("work_id", "")
            if cid and wid:
                needed_course_ids.add(cid)

        work_url_map = {}
        for cid in needed_course_ids:
            course = course_map.get(cid)
            if not course:
                continue
            work_items = self._get_work_enc_and_url(course)
            if work_items:
                for w in work_items:
                    key = f"{w['course_id']}_{w['work_id']}"
                    if key not in work_url_map:
                        work_url_map[key] = w["url"]

        def find_work_url(item):
            course_id = item.get("course_id", "")
            work_id = item.get("work_id", "")
            if course_id and work_id:
                key = f"{course_id}_{work_id}"
                if key in work_url_map:
                    return work_url_map[key]
            return None

        for item in hw_items:
            hw_url = find_work_url(item)
            if not hw_url:
                course_id = item.get("course_id", "")
                clazz_id = item.get("clazz_id", "")
                if course_id and clazz_id:
                    hw_url = self._build_homework_url(course_id, clazz_id)
                else:
                    hw_url = self._fix_url(item.get("raw", ""))
            homework_list.append(HomeworkItem(
                title=item["title"],
                course=item["course"],
                deadline=item["deadline"],
                platform=self.PLATFORM,
                submitted=not item["uncommitted"],
                url=hw_url,
            ))
        print(f"[超星] 从作业页面获取到 {len(hw_items)} 项作业")

        seen_exam_ids = set()
        try:
            resp = self.session.get(self.API_EXAM_PAGE)
            exam_items = self._extract_exams_from_html(resp.text)
            for item in exam_items:
                key = item["exam_id"] or item["title"]
                if key in seen_exam_ids:
                    continue
                seen_exam_ids.add(key)
                if not item["finished"] and not item["expired"]:
                    course_id = item.get("course_id", "")
                    class_id = item.get("class_id", "")
                    exam_id = item.get("exam_id", "")
                    if course_id and class_id and exam_id:
                        exam_url = self._build_exam_url(course_id, class_id, exam_id)
                    else:
                        exam_url = self._fix_url(item.get("raw", ""))
                    homework_list.append(HomeworkItem(
                        title=item["title"],
                        course="考试",
                        deadline=item["deadline"],
                        platform=self.PLATFORM,
                        submitted=False,
                        url=exam_url,
                    ))
            print(f"[超星] 从考试页面获取到 {len(exam_items)} 项考试")
        except Exception as e:
            print(f"[超星] 获取考试页面失败: {e}")

        try:
            resp = self.session.get(self.API_EXAM_LIST)
            table_items = self._extract_exams_from_table(resp.text)
            for item in table_items:
                key = item["exam_id"] or item["title"]
                if key in seen_exam_ids:
                    continue
                seen_exam_ids.add(key)
                if not item["finished"] and not item["expired"]:
                    exam_url = ""
                    if item["course_id"] and item["class_id"] and item["exam_id"]:
                        exam_url = self._build_exam_url(item["course_id"], item["class_id"], item["exam_id"])
                    homework_list.append(HomeworkItem(
                        title=item["title"],
                        course="考试",
                        deadline=item["deadline"],
                        platform=self.PLATFORM,
                        submitted=False,
                        url=exam_url,
                    ))
            print(f"[超星] 从考试列表获取到 {len(table_items)} 项考试")
        except Exception as e:
            print(f"[超星] 获取考试列表失败: {e}")

        try:
            for course in self.courses:
                activities = self._get_activities(course)
                for act in activities:
                    if act["ongoing"] and act["activeType"] not in (0, 2):
                        if act["deadline"] and act["deadline"] < datetime.now():
                            continue
                        act_url = self._build_activity_url(act["courseId"], act["clazzId"])
                        homework_list.append(HomeworkItem(
                            title=act["title"],
                            course=act["courseName"],
                            deadline=act["deadline"],
                            platform=self.PLATFORM,
                            submitted=act["ended"],
                            url=act_url,
                        ))
        except Exception as e:
            print(f"[超星] 获取课程活动失败: {e}")

        print(f"[超星] 共获取到 {len(homework_list)} 项作业/考试/任务")
        return homework_list


class KeTangPaiClient:
    PLATFORM = "课堂派"
    BASE_URL = "https://openapiv5.ketangpai.com"

    def __init__(self, email, password):
        self.email = email
        self.password = password
        self.token = None
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": UA,
            "Content-Type": "application/json;charset=UTF-8",
        })

    def login(self):
        url = f"{self.BASE_URL}/UserApi/login"
        body = {
            "email": self.email,
            "password": self.password,
            "remember": "0",
            "code": "",
            "mobile": "",
            "type": "login",
            "reqtimestamp": int(datetime.now().timestamp() * 1000),
        }
        resp = self.session.post(url, json=body)
        data = resp.json()
        if data.get("message") == "访问成功":
            self.token = data["data"]["token"]
            self.session.headers.update({"token": self.token})
            print(f"[课堂派] 登录成功")
            return True
        print(f"[课堂派] 登录失败: {data.get('message', '未知错误')}")
        return False

    def get_courses(self):
        url = f"{self.BASE_URL}/CourseApi/semesterCourseList"
        body = {
            "isstudy": "1",
            "search": "",
            "semester": "",
            "term": "",
            "reqtimestamp": int(datetime.now().timestamp() * 1000),
        }
        resp = self.session.post(url, json=body)
        data = resp.json()
        if data.get("message") == "访问成功":
            all_courses = data.get("data", [])
            current_year = datetime.now().year
            current_month = datetime.now().month
            if current_month >= 2 and current_month <= 7:
                current_term = f"{current_year-1}-{current_year}"
            else:
                current_term = f"{current_year}-{current_year+1}"
            courses = [c for c in all_courses if c.get("semester") == current_term]
            print(f"[课堂派] 获取到 {len(all_courses)} 门课程，当前学期 {current_term} 有 {len(courses)} 门")
            return courses
        print(f"[课堂派] 获取课程列表失败")
        return []

    def get_homework(self):
        homework_list = []
        courses = self.get_courses()
        for course in courses:
            try:
                url = f"{self.BASE_URL}/FutureV2/CourseMeans/getCourseContent"
                body = {
                    "contenttype": 4,
                    "dirid": 0,
                    "lessonlink": [],
                    "sort": [],
                    "page": 1,
                    "limit": 50,
                    "desc": 3,
                    "courserole": 0,
                    "vtr_type": "",
                    "courseid": course["id"],
                    "reqtimestamp": int(datetime.now().timestamp() * 1000),
                }
                resp = self.session.post(url, json=body)
                data = resp.json()
                if data.get("message") != "访问成功":
                    continue
                content_list = data.get("data", {}).get("list", [])
                for item in content_list:
                    deadline = None
                    endtime = item.get("endtime")
                    if endtime:
                        try:
                            deadline = datetime.fromtimestamp(int(endtime))
                        except (ValueError, OSError):
                            pass
                    submitted = item.get("mstatus") == 1
                    if submitted and deadline and deadline < datetime.now():
                        continue
                    course_name = course.get("coursename", course.get("name", "未知课程"))
                    ktp_url = f"https://w.ketangpai.com/homework?id={item.get('id', '')}&courseId={course['id']}&courseRole=0"
                    homework_list.append(HomeworkItem(
                        title=item.get("title", "未知作业"),
                        course=course_name,
                        deadline=deadline,
                        platform=self.PLATFORM,
                        submitted=submitted,
                        url=ktp_url,
                    ))
            except Exception as e:
                print(f"[课堂派] 获取课程 '{course.get('coursename', '')}' 作业时出错: {e}")
        homework_list.sort(key=lambda h: h.deadline or datetime.max)
        print(f"[课堂派] 共获取到 {len(homework_list)} 项作业")
        return homework_list


class YuKeTangClient:
    PLATFORM = "长江雨课堂"

    WORK_STATUS = {0: "未提交", 1: "未提交", 2: "已批改", 3: "已批改", 5: "已提交", 6: "缺考"}

    def __init__(self, csrftoken="", sessionid="", university_id="3078"):
        self.csrftoken = csrftoken
        self.sessionid = sessionid
        self.university_id = university_id
        self.session = requests.Session()
        self.session.headers.update({"User-Agent": UA})
        self._logged_in = False
        if csrftoken and sessionid:
            self._apply_cookies()
            self._logged_in = True

    def _apply_cookies(self):
        self.session.cookies.set("csrftoken", self.csrftoken)
        self.session.cookies.set("sessionid", self.sessionid)
        self.session.cookies.set("university_id", self.university_id)
        self.session.cookies.set("platform_id", "3")

    def login_qrcode(self, qrcode_callback=None):
        import websocket
        import json as _json
        import threading

        self.session.get("https://changjiang.yuketang.cn/web")
        result = {"user_id": None, "auth": None, "success": False}
        event = threading.Event()

        def on_message(ws, message):
            data = _json.loads(message)
            if "qrcode" in data:
                if qrcode_callback:
                    qrcode_callback(data["qrcode"])
                else:
                    try:
                        import qrcode as qr_mod
                        qr = qr_mod.QRCode()
                        qr.add_data(data["qrcode"])
                        qr.print_ascii(invert=True)
                    except Exception:
                        print(f"[雨课堂] 请扫描二维码: {data['qrcode']}")
                print("[雨课堂] 请使用微信雨课堂小程序扫描二维码登录")
            elif data.get("subscribe_status") is True:
                result["user_id"] = data["UserID"]
                result["auth"] = data["Auth"]
                result["success"] = True
                name = data.get("Name", "")
                school = data.get("School", "")
                print(f"[雨课堂] 扫码登录成功！姓名: {name}，学校: {school}")
                ws.close()
                event.set()

        def on_error(ws, error):
            print(f"[雨课堂] WebSocket 错误: {error}")
            event.set()

        def on_close(ws, close_status_code, close_msg):
            event.set()

        def on_open(ws):
            ws.send(_json.dumps({
                "op": "requestlogin",
                "role": "web",
                "version": 1.4,
                "type": "qrcode",
                "from": "web",
            }))

        uri = "wss://changjiang.yuketang.cn/wsapp/"
        ws = websocket.WebSocketApp(
            uri,
            on_open=on_open,
            on_message=on_message,
            on_error=on_error,
            on_close=on_close,
        )
        ws_thread = threading.Thread(target=ws.run_forever, daemon=True)
        ws_thread.start()
        event.wait(timeout=120)

        if result["success"]:
            login_resp = self.session.post(
                "https://changjiang.yuketang.cn/pc/web_login",
                data=_json.dumps({"UserID": result["user_id"], "Auth": result["auth"]}),
            )
            if login_resp.status_code == 200:
                self.csrftoken = self.session.cookies.get("csrftoken", "")
                self.sessionid = self.session.cookies.get("sessionid", "")
                self.university_id = self.session.cookies.get("university_id", self.university_id)
                self._logged_in = True
                print("[雨课堂] 登录凭证获取成功")
                return True
            else:
                print("[雨课堂] 登录凭证获取失败")
                return False
        else:
            print("[雨课堂] 扫码登录超时或失败")
            return False

    def is_logged_in(self):
        return self._logged_in

    def _api_headers(self, classroom_id=""):
        headers = {
            "X-Csrftoken": self.csrftoken,
            "xtbz": "ykt",
            "xt-agent": "web",
            "Referer": "https://changjiang.yuketang.cn/",
        }
        if classroom_id:
            headers["classroom-id"] = str(classroom_id)
            headers["Cookie"] = (
                f"csrftoken={self.csrftoken}; "
                f"sessionid={self.sessionid}; "
                f"classroom_id={classroom_id}; "
                f"classroomId={classroom_id}; "
                f"university_id={self.university_id}; "
                f"platform_id=3"
            )
        else:
            headers["Cookie"] = (
                f"csrftoken={self.csrftoken}; "
                f"sessionid={self.sessionid}; "
                f"university_id={self.university_id}; "
                f"platform_id=3"
            )
        return headers

    def get_courses(self):
        url = "https://changjiang.yuketang.cn/v2/api/web/courses/list?identity=2"
        try:
            resp = self.session.get(url, headers=self._api_headers())
            data = resp.json()
            courses = []
            now = datetime.now()
            for item in data.get("data", {}).get("list", []):
                course_info = item.get("course", {})
                classroom_id = item.get("classroom_id")
                if not course_info or not classroom_id:
                    continue
                course_end_time = item.get("end_time") or course_info.get("end_time")
                if course_end_time:
                    try:
                        end_val = int(course_end_time)
                        if end_val > 1e10:
                            end_val = end_val // 1000
                        end_dt = datetime.fromtimestamp(end_val)
                        if end_dt < now:
                            continue
                    except (ValueError, OSError):
                        pass
                course_time = item.get("time")
                if course_time:
                    try:
                        time_val = int(course_time)
                        if time_val > 1e10:
                            time_val = time_val // 1000
                        start_dt = datetime.fromtimestamp(time_val)
                        if (now - start_dt).days > 180:
                            continue
                    except (ValueError, OSError):
                        pass
                courses.append({
                    "course_name": course_info.get("name", "未知课程"),
                    "classroom_id": str(classroom_id),
                    "course_id": str(course_info.get("id", "")),
                    "teacher": item.get("teacher", {}).get("name", ""),
                })
            print(f"[长江雨课堂] 获取到 {len(courses)} 门课程")
            return courses
        except Exception as e:
            print(f"[长江雨课堂] 获取课程列表失败: {e}")
            return []

    def _parse_timestamp(self, ts):
        if not ts:
            return None
        try:
            val = int(ts)
            if val > 1e10:
                val = val // 1000
            return datetime.fromtimestamp(val)
        except (ValueError, OSError):
            return None

    def get_homework(self):
        homework_list = []
        courses = self.get_courses()
        for course in courses:
            try:
                classroom_id = course["classroom_id"]
                url = f"https://changjiang.yuketang.cn/v2/api/web/logs/learn/{classroom_id}?actype=5&page=0&offset=50&sort=-1"
                resp = self.session.get(url, headers=self._api_headers(classroom_id))
                data = resp.json()
                activities = data.get("data", {}).get("activities", [])
                for act in activities:
                    status = act.get("status", 0)
                    submitted = status in (2, 3, 5)
                    title = act.get("title", "未知作业")
                    courseware_id = act.get("courseware_id", "")
                    deadline = self._parse_timestamp(act.get("end_time"))
                    if not deadline:
                        deadline = self._parse_timestamp(act.get("close_time"))
                    if not deadline:
                        deadline = self._parse_timestamp(act.get("deadline"))
                    if not deadline:
                        begin_time = self._parse_timestamp(act.get("begin_time"))
                        duration = act.get("duration")
                        if begin_time and duration:
                            try:
                                deadline = begin_time + timedelta(seconds=int(duration))
                            except (ValueError, OSError):
                                pass
                    if submitted and deadline and deadline < datetime.now():
                        continue
                    ykt_url = f"https://changjiang.yuketang.cn/v2/web/studentLog/{classroom_id}"
                    if courseware_id:
                        ykt_url = f"https://changjiang.yuketang.cn/v2/web/trans/{classroom_id}/{courseware_id}?status=1"
                    homework_list.append(HomeworkItem(
                        title=title,
                        course=course["course_name"],
                        deadline=deadline,
                        platform=self.PLATFORM,
                        submitted=submitted,
                        url=ykt_url,
                    ))
            except Exception as e:
                print(f"[长江雨课堂] 获取课程 '{course.get('course_name', '')}' 作业时出错: {e}")
        print(f"[长江雨课堂] 共获取到 {len(homework_list)} 项作业")
        return homework_list


URGENCY_STYLE = {
    "overdue": "\033[91m",
    "urgent": "\033[93m",
    "soon": "\033[96m",
    "normal": "\033[92m",
    "relaxed": "\033[90m",
    "unknown": "\033[37m",
}
RESET = "\033[0m"
BOLD = "\033[1m"


def print_reminder(homework_list):
    pending = [h for h in homework_list if not h.submitted and not h.is_overdue]
    pending.sort(key=lambda h: h.deadline or datetime.max)

    print(f"\n{'=' * 70}")
    print(f"{BOLD}  📋 作业提醒 Note{RESET}")
    print(f"  生成时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"  待完成作业: {len(pending)} / 总计: {len(homework_list)}")
    print(f"{'=' * 70}\n")

    if not pending:
        print(f"  {BOLD}🎉 没有待完成的作业！{RESET}\n")
        return

    current_urgency = None
    for h in pending:
        style = URGENCY_STYLE.get(h.urgency, "")
        if h.urgency != current_urgency:
            current_urgency = h.urgency
            label = {
                "overdue": "⏰ 已过期",
                "urgent": "🔴 6小时内截止",
                "soon": "🟠 1天内截止",
                "normal": "🟡 3天内截止",
                "relaxed": "🟢 3天后截止",
                "unknown": "⚪ 无截止时间",
            }.get(h.urgency, "")
            print(f"  {style}{BOLD}{label}{RESET}")
            print(f"  {'-' * 50}")

        deadline_str = h.deadline.strftime("%m-%d %H:%M") if h.deadline else "无"
        if h.deadline:
            delta = h.deadline - datetime.now()
            if h.is_overdue:
                remain = f"已过期 {abs(delta).days}天"
            elif delta.days > 0:
                remain = f"剩余 {delta.days}天"
            else:
                hours = delta.seconds // 3600
                minutes = (delta.seconds % 3600) // 60
                remain = f"剩余 {hours}时{minutes}分"
        else:
            remain = ""
        print(f"  {style}  [{h.platform}] {h.course}{RESET}")
        print(f"  {style}    📝 {h.title}{RESET}")
        print(f"  {style}    📅 截止: {deadline_str}  ({remain}){RESET}")
        print()

    print(f"{'=' * 70}")
    urgent_count = sum(1 for h in pending if h.urgency == "urgent")
    if urgent_count:
        print(f"  {URGENCY_STYLE['urgent']}{BOLD}⚠ {urgent_count} 项作业即将截止！{RESET}")
    print(f"{'=' * 70}\n")


def fetch_all_homework(progress_callback=None):
    config = load_config()
    all_homework = []
    steps = [
        ("chaoxing", "超星"),
        ("ketangpai", "课堂派"),
        ("yuketang", "长江雨课堂"),
    ]
    total = len(steps)

    for i, (key, label) in enumerate(steps):
        if progress_callback:
            progress_callback(i, total, f"正在获取{label}...")

        cfg = config.get(key, {})
        if key == "chaoxing":
            if cfg.get("enabled") and cfg.get("user") and cfg.get("password"):
                cx = ChaoXingClient(user=cfg["user"], password=cfg["password"])
                if cx.login():
                    all_homework.extend(cx.get_homework())
            else:
                print(f"[{label}] 未配置或未启用，跳过")
        elif key == "ketangpai":
            if cfg.get("enabled") and cfg.get("email") and cfg.get("password"):
                ktp = KeTangPaiClient(email=cfg["email"], password=cfg["password"])
                if ktp.login():
                    all_homework.extend(ktp.get_homework())
            else:
                print(f"[{label}] 未配置或未启用，跳过")
        elif key == "yuketang":
            ykt = YuKeTangClient(
                csrftoken=cfg.get("csrftoken", ""),
                sessionid=cfg.get("sessionid", ""),
                university_id=cfg.get("university_id", "3078"),
            )
            if ykt.is_logged_in():
                all_homework.extend(ykt.get_homework())
            elif cfg.get("enabled"):
                if ykt.login_qrcode():
                    cfg["csrftoken"] = ykt.csrftoken
                    cfg["sessionid"] = ykt.sessionid
                    save_config(config)
                    all_homework.extend(ykt.get_homework())
            else:
                print(f"[{label}] 未配置或未启用，跳过")

    if progress_callback:
        progress_callback(total, total, "获取完成")

    return all_homework


def main():
    all_homework = fetch_all_homework()
    print_reminder(all_homework)


if __name__ == "__main__":
    main()
