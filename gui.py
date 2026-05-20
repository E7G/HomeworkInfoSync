# -*- coding: utf-8 -*-
import json
import sys
import threading
import io
import webbrowser
from pathlib import Path
from datetime import datetime

from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QLineEdit, QCheckBox, QPushButton,
    QScrollArea, QFrame, QTextEdit, QStackedWidget,
    QSizePolicy, QSpacerItem, QGridLayout, QProgressBar,
)
from PyQt6.QtCore import Qt, pyqtSignal, QObject, QSize, QTimer, QPropertyAnimation, QEasingCurve
from PyQt6.QtGui import QFont, QColor, QIcon, QPainter, QPen, QBrush, QPixmap, QGradient, QCursor

def _resource_dir():
    if getattr(sys, 'frozen', False):
        return Path(sys._MEIPASS)
    return Path(__file__).parent


def _app_dir():
    if getattr(sys, 'frozen', False):
        return Path(sys.executable).parent
    return Path(__file__).parent


CONFIG_PATH = _app_dir() / "config.json"

DEFAULT_CONFIG = {
    "chaoxing": {"enabled": False, "user": "", "password": ""},
    "ketangpai": {"enabled": False, "email": "", "password": ""},
    "yuketang": {"enabled": False, "csrftoken": "", "sessionid": "", "university_id": "3078"},
}

PLATFORM_LABELS = {
    "chaoxing": "超星/学习通",
    "ketangpai": "课堂派",
    "yuketang": "长江雨课堂",
}

PLATFORM_FIELDS = {
    "chaoxing": [
        {"key": "user", "label": "账号", "secret": False},
        {"key": "password", "label": "密码", "secret": True},
    ],
    "ketangpai": [
        {"key": "email", "label": "邮箱", "secret": False},
        {"key": "password", "label": "密码", "secret": True},
    ],
    "yuketang": [],
}

PLATFORM_HINTS = {
    "chaoxing": "使用超星/学习通的手机号或学号登录",
    "ketangpai": "使用课堂派注册邮箱登录",
    "yuketang": "点击扫码登录按钮，使用微信长江雨课堂小程序扫描二维码\n登录成功后凭证会自动保存",
}

URGENCY_COLORS = {
    "overdue": "#ef5350",
    "urgent": "#ffa726",
    "soon": "#42a5f5",
    "normal": "#66bb6a",
    "relaxed": "#78909c",
    "unknown": "#90a4ae",
}

URGENCY_BG = {
    "overdue": "rgba(239,83,80,0.12)",
    "urgent": "rgba(255,167,38,0.12)",
    "soon": "rgba(66,165,245,0.12)",
    "normal": "rgba(102,187,106,0.12)",
    "relaxed": "rgba(120,144,156,0.08)",
    "unknown": "rgba(144,164,174,0.08)",
}

URGENCY_LABELS = {
    "overdue": "已过期",
    "urgent": "6小时内",
    "soon": "1天内",
    "normal": "3天内",
    "relaxed": "3天后",
    "unknown": "无截止",
}

NAV_ITEMS = [
    {"id": "home", "icon": "\u2302", "label": "作业"},
    {"id": "config", "icon": "\u2699", "label": "配置"},
    {"id": "log", "icon": "\u2261", "label": "日志"},
]


def load_config():
    if CONFIG_PATH.exists():
        with open(CONFIG_PATH, "r", encoding="utf-8") as f:
            cfg = json.load(f)
        for platform, defaults in DEFAULT_CONFIG.items():
            if platform not in cfg:
                cfg[platform] = defaults
            else:
                for k, v in defaults.items():
                    cfg[platform].setdefault(k, v)
        return cfg
    return json.loads(json.dumps(DEFAULT_CONFIG))


def save_config(cfg):
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)


class WorkerSignals(QObject):
    finished = pyqtSignal(str, list)
    progress = pyqtSignal(int, int, str)


class HomeworkWorker(threading.Thread):
    def __init__(self, signals):
        super().__init__(daemon=True)
        self.signals = signals

    def run(self):
        from homework_reminder import fetch_all_homework

        buf = io.StringIO()
        old_stdout = sys.stdout
        sys.stdout = buf

        try:
            def on_progress(step, total, msg):
                self.signals.progress.emit(step, total, msg)

            all_homework = fetch_all_homework(progress_callback=on_progress)
            self.signals.finished.emit(buf.getvalue(), all_homework)
        except Exception as e:
            self.signals.finished.emit(buf.getvalue() + f"\n错误: {e}", [])
        finally:
            sys.stdout = old_stdout


class NavButton(QPushButton):
    def __init__(self, icon_char, label, parent=None):
        super().__init__(parent)
        self.icon_char = icon_char
        self.label_text = label
        self._active = False
        self.setCheckable(True)
        self.setFixedHeight(48)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setObjectName("navBtn")
        self.setText(f"  {icon_char}  {label}")
        self.setFont(QFont("Segoe UI", 10))

    @property
    def active(self):
        return self._active

    @active.setter
    def active(self, val):
        self._active = val
        self.setChecked(val)
        self.style().unpolish(self)
        self.style().polish(self)


class Sidebar(QFrame):
    page_changed = pyqtSignal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("sidebar")
        self.setFixedWidth(180)
        self.buttons = []
        self._build()

    def _build(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        logo = QLabel("  HomeworkSync")
        logo.setObjectName("sidebarLogo")
        logo.setFixedHeight(56)
        logo.setFont(QFont("Segoe UI", 11, QFont.Weight.Bold))
        layout.addWidget(logo)

        sep = QFrame()
        sep.setFrameShape(QFrame.Shape.HLine)
        sep.setObjectName("sidebarSep")
        sep.setFixedHeight(1)
        layout.addWidget(sep)

        layout.addSpacing(8)

        for item in NAV_ITEMS:
            btn = NavButton(item["icon"], item["label"])
            btn.clicked.connect(lambda checked, id=item["id"]: self._on_nav(id))
            self.buttons.append((item["id"], btn))
            layout.addWidget(btn)

        layout.addStretch()

        ver = QLabel("  v0.1.0")
        ver.setObjectName("sidebarVersion")
        ver.setFixedHeight(32)
        layout.addWidget(ver)

        if self.buttons:
            self.buttons[0][1].active = True

    def _on_nav(self, page_id):
        for bid, btn in self.buttons:
            btn.active = (bid == page_id)
        self.page_changed.emit(page_id)


class HomeworkCard(QFrame):
    clicked = pyqtSignal()

    def __init__(self, homework, parent=None):
        super().__init__(parent)
        self.homework = homework
        self.setObjectName("hwCard")
        if homework.url:
            self.setCursor(Qt.CursorShape.PointingHandCursor)
        self._build()

    def _build(self):
        h = self.homework
        color = URGENCY_COLORS.get(h.urgency, "#90a4ae")
        bg = URGENCY_BG.get(h.urgency, "rgba(144,164,174,0.08)")
        urgency_label = URGENCY_LABELS.get(h.urgency, "")

        self.setStyleSheet(f"""
            QFrame#hwCard {{
                background-color: #1e2330;
                border: 1px solid #2a3040;
                border-radius: 12px;
                border-left: 4px solid {color};
                padding: 2px;
            }}
            QFrame#hwCard:hover {{
                border: 1px solid #3a4050;
                border-left: 4px solid {color};
                background-color: #222838;
            }}
        """)

        layout = QVBoxLayout(self)
        layout.setSpacing(6)
        layout.setContentsMargins(18, 14, 18, 14)

        top = QHBoxLayout()
        top.setSpacing(10)

        platform_badge = QLabel(f"  {h.platform}  ")
        platform_badge.setObjectName("platformBadge")
        platform_badge.setStyleSheet(f"""
            background-color: {bg};
            color: {color};
            border-radius: 10px;
            padding: 2px 10px;
            font-size: 11px;
            font-weight: bold;
        """)
        top.addWidget(platform_badge)

        course_label = QLabel(h.course)
        course_label.setStyleSheet("color: #e0e0e0; font-size: 13px; font-weight: 600;")
        top.addWidget(course_label)

        top.addStretch()

        urgency_lbl = QLabel(urgency_label)
        urgency_lbl.setStyleSheet(f"""
            color: {color};
            font-size: 12px;
            font-weight: bold;
            background-color: {bg};
            border-radius: 10px;
            padding: 2px 10px;
        """)
        top.addWidget(urgency_lbl)

        if h.url:
            link_icon = QLabel("↗")
            link_icon.setStyleSheet(f"color: #5b8af5; font-size: 16px; font-weight: bold;")
            link_icon.setToolTip("点击打开作业页面")
            top.addWidget(link_icon)

        layout.addLayout(top)

        title_label = QLabel(h.title)
        title_label.setStyleSheet("color: #ffffff; font-size: 15px; font-weight: 500;")
        layout.addWidget(title_label)

        bottom = QHBoxLayout()
        deadline_str = h.deadline.strftime("%Y-%m-%d %H:%M") if h.deadline else "无截止时间"
        deadline_label = QLabel(f"截止: {deadline_str}")
        deadline_label.setStyleSheet("color: #7a8599; font-size: 12px;")
        bottom.addWidget(deadline_label)

        bottom.addStretch()

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
            remain_label = QLabel(remain)
            remain_label.setStyleSheet(f"color: {color}; font-size: 12px; font-weight: bold;")
            bottom.addWidget(remain_label)

        layout.addLayout(bottom)

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()
        super().mousePressEvent(event)

    def _open_url(self):
        if self.homework.url:
            webbrowser.open(self.homework.url)


class HomePage(QWidget):
    refresh_requested = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.homework_list = []
        self.log_text = ""
        self._build()

    def _build(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 24, 28, 20)
        layout.setSpacing(16)

        header_row = QHBoxLayout()

        title = QLabel("作业提醒")
        title.setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;")
        header_row.addWidget(title)

        header_row.addStretch()

        self.refresh_btn = QPushButton("刷新")
        self.refresh_btn.setObjectName("accentBtn")
        self.refresh_btn.setFixedSize(80, 36)
        self.refresh_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.refresh_btn.clicked.connect(self.refresh_requested.emit)
        header_row.addWidget(self.refresh_btn)

        layout.addLayout(header_row)

        self.stats_frame = QFrame()
        self.stats_frame.setObjectName("statsFrame")
        stats_layout = QHBoxLayout(self.stats_frame)
        stats_layout.setContentsMargins(0, 0, 0, 0)
        stats_layout.setSpacing(12)

        self.stat_total = self._stat_card("总作业", "0", "#42a5f5")
        self.stat_unsub = self._stat_card("未提交", "0", "#ffa726")
        self.stat_urgent = self._stat_card("紧急", "0", "#ef5350")
        self.stat_done = self._stat_card("已完成", "0", "#66bb6a")

        stats_layout.addWidget(self.stat_total)
        stats_layout.addWidget(self.stat_unsub)
        stats_layout.addWidget(self.stat_urgent)
        stats_layout.addWidget(self.stat_done)

        layout.addWidget(self.stats_frame)

        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setFrameShape(QFrame.Shape.NoFrame)
        self.scroll.setObjectName("homeScroll")

        self.cards_container = QWidget()
        self.cards_layout = QVBoxLayout(self.cards_container)
        self.cards_layout.setSpacing(10)
        self.cards_layout.setContentsMargins(0, 0, 0, 0)
        self.cards_layout.addStretch()

        self.scroll.setWidget(self.cards_container)
        layout.addWidget(self.scroll)

        self.progress_bar = QProgressBar()
        self.progress_bar.setObjectName("fetchProgress")
        self.progress_bar.setFixedHeight(4)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.setMaximum(3)
        self.progress_bar.setValue(0)
        self.progress_bar.hide()
        layout.addWidget(self.progress_bar)

    def set_progress(self, step, total, msg):
        self.progress_bar.show()
        self.progress_bar.setMaximum(total)
        self.progress_bar.setValue(step)
        if step >= total:
            QTimer.singleShot(800, self.progress_bar.hide)

    def _stat_card(self, label, value, color):
        card = QFrame()
        card.setObjectName("statCard")
        card.setStyleSheet(f"""
            QFrame#statCard {{
                background-color: #1e2330;
                border: 1px solid #2a3040;
                border-radius: 12px;
                border-top: 3px solid {color};
            }}
        """)
        cl = QVBoxLayout(card)
        cl.setContentsMargins(16, 12, 16, 12)
        cl.setSpacing(2)

        val_label = QLabel(value)
        val_label.setStyleSheet(f"color: {color}; font-size: 24px; font-weight: bold;")
        val_label.setObjectName(f"stat_{label}")
        cl.addWidget(val_label)

        name_label = QLabel(label)
        name_label.setStyleSheet("color: #7a8599; font-size: 12px;")
        cl.addWidget(name_label)

        return card

    def update_data(self, homework_list):
        self.homework_list = homework_list
        pending = [h for h in homework_list if not h.submitted and not h.is_overdue]
        urgent = [h for h in pending if h.urgency == "urgent"]
        done = [h for h in homework_list if h.submitted]

        self.stat_total.findChild(QLabel, f"stat_总作业").setText(str(len(homework_list)))
        self.stat_unsub.findChild(QLabel, f"stat_未提交").setText(str(len(pending)))
        self.stat_urgent.findChild(QLabel, f"stat_紧急").setText(str(len(urgent)))
        self.stat_done.findChild(QLabel, f"stat_已完成").setText(str(len(done)))

        while self.cards_layout.count() > 1:
            item = self.cards_layout.takeAt(0)
            if item.widget():
                item.widget().deleteLater()

        if not pending:
            empty = QLabel("没有待完成的作业！")
            empty.setAlignment(Qt.AlignmentFlag.AlignCenter)
            empty.setStyleSheet("color: #66bb6a; font-size: 16px; padding: 40px;")
            self.cards_layout.insertWidget(0, empty)
            return

        sorted_list = sorted(pending, key=lambda h: h.deadline or datetime.max)
        for h in sorted_list:
            card = HomeworkCard(h)
            card.clicked.connect(card._open_url)
            self.cards_layout.insertWidget(self.cards_layout.count() - 1, card)


class ConfigPage(QWidget):
    config_saved = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.config_data = load_config()
        self.platform_widgets = {}
        self._build()

    def _build(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 24, 28, 20)
        layout.setSpacing(16)

        header_row = QHBoxLayout()
        title = QLabel("平台配置")
        title.setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;")
        header_row.addWidget(title)
        header_row.addStretch()

        save_btn = QPushButton("保存")
        save_btn.setObjectName("accentBtn")
        save_btn.setFixedSize(80, 36)
        save_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        save_btn.clicked.connect(self._on_save)
        header_row.addWidget(save_btn)

        reset_btn = QPushButton("重置")
        reset_btn.setObjectName("ghostBtn")
        reset_btn.setFixedSize(80, 36)
        reset_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        reset_btn.clicked.connect(self._on_reset)
        header_row.addWidget(reset_btn)

        layout.addLayout(header_row)

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)

        container = QWidget()
        container_layout = QVBoxLayout(container)
        container_layout.setSpacing(16)
        container_layout.setContentsMargins(0, 0, 0, 0)

        for platform in ["chaoxing", "ketangpai", "yuketang"]:
            card = self._build_platform_card(platform)
            container_layout.addWidget(card)

        container_layout.addStretch()
        scroll.setWidget(container)
        layout.addWidget(scroll)

    def _build_platform_card(self, platform):
        card = QFrame()
        card.setObjectName("configCard")
        card.setStyleSheet("""
            QFrame#configCard {
                background-color: #1e2330;
                border: 1px solid #2a3040;
                border-radius: 12px;
            }
        """)

        cl = QVBoxLayout(card)
        cl.setContentsMargins(20, 16, 20, 16)
        cl.setSpacing(12)

        header = QHBoxLayout()
        name = QLabel(PLATFORM_LABELS[platform])
        name.setStyleSheet("color: #ffffff; font-size: 15px; font-weight: bold;")
        header.addWidget(name)
        header.addStretch()

        enabled_cb = QCheckBox("启用")
        enabled_cb.setChecked(self.config_data.get(platform, {}).get("enabled", False))
        enabled_cb.setObjectName("platformEnabled")
        enabled_cb.setStyleSheet("""
            QCheckBox::indicator {
                width: 18px; height: 18px;
                border-radius: 4px;
                border: 2px solid #4a5568;
            }
            QCheckBox::indicator:checked {
                background-color: #5b8af5;
                border-color: #5b8af5;
            }
            QCheckBox {
                color: #a0aec0;
                font-size: 13px;
            }
        """)
        header.addWidget(enabled_cb)

        cl.addLayout(header)

        sep = QFrame()
        sep.setFrameShape(QFrame.Shape.HLine)
        sep.setStyleSheet("background-color: #2a3040; border: none; max-height: 1px;")
        cl.addWidget(sep)

        fields_dict = {}
        if platform == "yuketang":
            qr_row = QHBoxLayout()
            qr_row.setSpacing(16)

            self.ykt_qr_label = QLabel()
            self.ykt_qr_label.setFixedSize(200, 200)
            self.ykt_qr_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self.ykt_qr_label.setStyleSheet("background-color: #0d1117; border-radius: 8px;")
            qr_row.addWidget(self.ykt_qr_label)

            qr_info = QVBoxLayout()
            qr_info.setSpacing(8)

            qr_btn = QPushButton("扫码登录")
            qr_btn.setObjectName("accentBtn")
            qr_btn.setFixedSize(120, 38)
            qr_btn.setCursor(Qt.CursorShape.PointingHandCursor)
            qr_btn.clicked.connect(self._on_ykt_qr_login)
            qr_info.addWidget(qr_btn)

            self.ykt_status_label = QLabel("未登录")
            self.ykt_status_label.setStyleSheet("color: #7a8599; font-size: 12px;")
            csrftoken = self.config_data.get("yuketang", {}).get("csrftoken", "")
            sessionid = self.config_data.get("yuketang", {}).get("sessionid", "")
            if csrftoken and sessionid:
                self.ykt_status_label.setText("已登录（凭证已保存）")
                self.ykt_status_label.setStyleSheet("color: #66bb6a; font-size: 12px;")
            qr_info.addWidget(self.ykt_status_label)

            qr_info.addStretch()
            qr_row.addLayout(qr_info)

            cl.addLayout(qr_row)
        else:
            for field in PLATFORM_FIELDS[platform]:
                row = QHBoxLayout()
                row.setSpacing(12)

                label = QLabel(field["label"])
                label.setFixedWidth(90)
                label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                label.setStyleSheet("color: #8892a4; font-size: 13px;")
                row.addWidget(label)

                line_edit = QLineEdit()
                line_edit.setPlaceholderText(f"请输入{field['label']}")
                line_edit.setText(self.config_data.get(platform, {}).get(field["key"], ""))
                line_edit.setObjectName("configInput")
                line_edit.setFixedHeight(38)
                if field["secret"]:
                    line_edit.setEchoMode(QLineEdit.EchoMode.Password)
                row.addWidget(line_edit)

                fields_dict[field["key"]] = line_edit

                if field["secret"]:
                    toggle = QCheckBox("显示")
                    toggle.setFixedWidth(56)
                    toggle.setStyleSheet("color: #6b7a8d; font-size: 11px;")
                    toggle.toggled.connect(
                        lambda checked, le=line_edit: le.setEchoMode(
                            QLineEdit.EchoMode.Normal if checked else QLineEdit.EchoMode.Password
                        )
                    )
                    row.addWidget(toggle)

                cl.addLayout(row)

        hint = QLabel(PLATFORM_HINTS.get(platform, ""))
        hint.setWordWrap(True)
        hint.setStyleSheet("color: #5a6577; font-size: 11px; padding-left: 102px;")
        cl.addWidget(hint)

        self.platform_widgets[platform] = {
            "enabled": enabled_cb,
            "fields": fields_dict,
        }

        return card

    def _collect_config(self):
        cfg = {}
        for platform, widgets in self.platform_widgets.items():
            section = {"enabled": widgets["enabled"].isChecked()}
            for key, le in widgets["fields"].items():
                section[key] = le.text()
            if platform == "yuketang":
                section["csrftoken"] = self.config_data.get("yuketang", {}).get("csrftoken", "")
                section["sessionid"] = self.config_data.get("yuketang", {}).get("sessionid", "")
                section["university_id"] = self.config_data.get("yuketang", {}).get("university_id", "3078")
            cfg[platform] = section
        return cfg

    def _on_save(self):
        cfg = self._collect_config()
        save_config(cfg)
        self.config_data = cfg
        self.config_saved.emit()

    def _on_reset(self):
        self.config_data = json.loads(json.dumps(DEFAULT_CONFIG))
        for platform, widgets in self.platform_widgets.items():
            cfg = self.config_data.get(platform, {})
            widgets["enabled"].setChecked(cfg.get("enabled", False))
            for key, le in widgets["fields"].items():
                le.setText(cfg.get(key, ""))
        if hasattr(self, "ykt_status_label"):
            self.ykt_status_label.setText("未登录")
            self.ykt_status_label.setStyleSheet("color: #7a8599; font-size: 12px;")
            self.ykt_qr_label.setPixmap(QPixmap())

    def _on_ykt_qr_login(self):
        from homework_reminder import YuKeTangClient

        self.ykt_status_label.setText("等待扫码...")
        self.ykt_status_label.setStyleSheet("color: #ffa726; font-size: 12px;")

        ykt = YuKeTangClient()

        def on_qrcode(url):
            try:
                import qrcode as qr_mod
                from io import BytesIO
                qr = qr_mod.QRCode(box_size=6, border=2)
                qr.add_data(url)
                qr.make(fit=True)
                img = qr.make_image(fill_color="white", back_color="#0d1117")
                buf = BytesIO()
                img.save(buf, format="PNG")
                pixmap = QPixmap()
                pixmap.loadFromData(buf.getvalue())
                self.ykt_qr_label.setPixmap(pixmap.scaled(
                    200, 200, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation
                ))
            except Exception as e:
                self.ykt_status_label.setText(f"二维码生成失败: {e}")

        def do_login():
            success = ykt.login_qrcode(qrcode_callback=on_qrcode)
            if success:
                self.config_data.setdefault("yuketang", {})
                self.config_data["yuketang"]["csrftoken"] = ykt.csrftoken
                self.config_data["yuketang"]["sessionid"] = ykt.sessionid
                self.config_data["yuketang"]["university_id"] = ykt.university_id
                self.config_data["yuketang"]["enabled"] = True
                save_config(self.config_data)
                self.ykt_status_label.setText("登录成功！凭证已保存")
                self.ykt_status_label.setStyleSheet("color: #66bb6a; font-size: 12px;")
            else:
                self.ykt_status_label.setText("登录失败或超时")
                self.ykt_status_label.setStyleSheet("color: #ef5350; font-size: 12px;")

        import threading
        t = threading.Thread(target=do_login, daemon=True)
        t.start()


class LogPage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.log_text = ""
        self._build()

    def _build(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 24, 28, 20)
        layout.setSpacing(16)

        title = QLabel("运行日志")
        title.setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;")
        layout.addWidget(title)

        self.log_edit = QTextEdit()
        self.log_edit.setReadOnly(True)
        self.log_edit.setObjectName("logEdit")
        self.log_edit.setFont(QFont("Cascadia Code", 9))
        layout.addWidget(self.log_edit)

    def update_log(self, text):
        self.log_text = text
        self.log_edit.setPlainText(text)


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("HomeworkSync")
        self.setMinimumSize(820, 560)
        self.resize(900, 620)
        self.config_data = load_config()
        self.worker_signals = WorkerSignals()
        self.worker_signals.finished.connect(self._on_fetch_done)
        self.worker_signals.progress.connect(self._on_fetch_progress)
        self._build()

    def _build(self):
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, False)

        central = QWidget()
        self.setCentralWidget(central)
        root = QHBoxLayout(central)
        root.setSpacing(0)
        root.setContentsMargins(0, 0, 0, 0)

        self.sidebar = Sidebar()
        self.sidebar.page_changed.connect(self._switch_page)
        root.addWidget(self.sidebar)

        right = QFrame()
        right.setObjectName("mainContent")
        right_layout = QVBoxLayout(right)
        right_layout.setContentsMargins(0, 0, 0, 0)
        right_layout.setSpacing(0)

        titlebar = QFrame()
        titlebar.setObjectName("titleBar")
        titlebar.setFixedHeight(42)
        tb_layout = QHBoxLayout(titlebar)
        tb_layout.setContentsMargins(16, 0, 8, 0)

        self.title_label = QLabel("HomeworkSync")
        self.title_label.setStyleSheet("color: #a0aec0; font-size: 12px; font-weight: bold;")
        tb_layout.addWidget(self.title_label)

        tb_layout.addStretch()

        for text, color, slot in [
            ("—", "#6b7a8d", self.showMinimized),
            ("□", "#6b7a8d", self._toggle_maximize),
            ("×", "#ef5350", self.close),
        ]:
            btn = QPushButton(text)
            btn.setFixedSize(36, 28)
            btn.setObjectName("titleBtn")
            hover_color = "#3a4050" if text != "×" else "#c62828"
            btn.setStyleSheet(f"""
                QPushButton#titleBtn {{
                    color: {color};
                    border: none;
                    border-radius: 4px;
                    font-size: 14px;
                    font-weight: bold;
                    background: transparent;
                }}
                QPushButton#titleBtn:hover {{
                    background-color: {hover_color};
                    color: #ffffff;
                }}
            """)
            btn.clicked.connect(slot)
            tb_layout.addWidget(btn)

        right_layout.addWidget(titlebar)

        self.stack = QStackedWidget()
        self.stack.setObjectName("pageStack")

        self.home_page = HomePage()
        self.home_page.refresh_requested.connect(self._on_refresh)
        self.stack.addWidget(self.home_page)

        self.config_page = ConfigPage()
        self.config_page.config_saved.connect(self._on_config_saved)
        self.stack.addWidget(self.config_page)

        self.log_page = LogPage()
        self.stack.addWidget(self.log_page)

        right_layout.addWidget(self.stack)

        status_bar = QFrame()
        status_bar.setObjectName("statusBar")
        status_bar.setFixedHeight(28)
        sb_layout = QHBoxLayout(status_bar)
        sb_layout.setContentsMargins(16, 0, 16, 0)

        self.status_label = QLabel("就绪")
        self.status_label.setStyleSheet("color: #5a6577; font-size: 11px;")
        sb_layout.addWidget(self.status_label)

        sb_layout.addStretch()

        time_label = QLabel(datetime.now().strftime("%H:%M"))
        time_label.setStyleSheet("color: #5a6577; font-size: 11px;")
        sb_layout.addWidget(time_label)

        right_layout.addWidget(status_bar)

        root.addWidget(right)

        self._drag_pos = None

    def _switch_page(self, page_id):
        idx = {"home": 0, "config": 1, "log": 2}.get(page_id, 0)
        self.stack.setCurrentIndex(idx)

    def _toggle_maximize(self):
        if self.isMaximized():
            self.showNormal()
        else:
            self.showMaximized()

    def _on_refresh(self):
        self.status_label.setText("正在获取作业...")
        self.home_page.set_progress(0, 3, "准备中...")
        self.home_page.refresh_btn.setEnabled(False)
        worker = HomeworkWorker(self.worker_signals)
        worker.start()

    def _on_fetch_progress(self, step, total, msg):
        self.status_label.setText(msg)
        self.home_page.set_progress(step, total, msg)

    def _on_config_saved(self):
        self.status_label.setText("配置已保存")

    def _on_fetch_done(self, log_text, homework_list):
        self.status_label.setText(f"获取完成 - 共 {len(homework_list)} 项作业")
        self.home_page.refresh_btn.setEnabled(True)
        self.home_page.update_data(homework_list)
        self.log_page.update_log(log_text)
        self.stack.setCurrentIndex(0)

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            titlebar = self.findChild(QFrame, "titleBar")
            if titlebar and titlebar.geometry().contains(event.position().toPoint()):
                self._drag_pos = event.globalPosition().toPoint() - self.frameGeometry().topLeft()

    def mouseMoveEvent(self, event):
        if self._drag_pos and event.buttons() & Qt.MouseButton.LeftButton:
            self.move(event.globalPosition().toPoint() - self._drag_pos)

    def mouseReleaseEvent(self, event):
        self._drag_pos = None

    def mouseDoubleClickEvent(self, event):
        titlebar = self.findChild(QFrame, "titleBar")
        if titlebar and titlebar.geometry().contains(event.position().toPoint()):
            self._toggle_maximize()


GLOBAL_STYLE = """
QMainWindow {
    background-color: #141820;
}

QFrame#sidebar {
    background-color: #0d1117;
    border-right: 1px solid #1e2530;
}

QLabel#sidebarLogo {
    color: #5b8af5;
    padding-left: 16px;
}

QFrame#sidebarSep {
    background-color: #1e2530;
    border: none;
}

QLabel#sidebarVersion {
    color: #3a4555;
    font-size: 11px;
}

QPushButton#navBtn {
    background: transparent;
    border: none;
    border-radius: 8px;
    margin: 2px 8px;
    padding: 0 12px;
    text-align: left;
    color: #7a8599;
    font-size: 13px;
}

QPushButton#navBtn:hover {
    background-color: #1a2030;
    color: #c0cad8;
}

QPushButton#navBtn:checked {
    background-color: #1a2540;
    color: #5b8af5;
    border-left: 3px solid #5b8af5;
}

QFrame#mainContent {
    background-color: #141820;
    border: none;
}

QFrame#titleBar {
    background-color: #0f1318;
    border-bottom: 1px solid #1e2530;
}

QFrame#statusBar {
    background-color: #0f1318;
    border-top: 1px solid #1e2530;
}

QScrollArea#homeScroll, QScrollArea {
    background-color: transparent;
    border: none;
}

QScrollBar:vertical {
    background: transparent;
    width: 6px;
    margin: 0;
}

QScrollBar::handle:vertical {
    background: #2a3545;
    border-radius: 3px;
    min-height: 30px;
}

QScrollBar::handle:vertical:hover {
    background: #3a4555;
}

QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0;
}

QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {
    background: transparent;
}

QLineEdit#configInput {
    background-color: #1a2030;
    border: 1px solid #2a3545;
    border-radius: 8px;
    padding: 0 14px;
    color: #e0e0e0;
    font-size: 13px;
    selection-background-color: #5b8af5;
}

QLineEdit#configInput:focus {
    border-color: #5b8af5;
    background-color: #1e2538;
}

QLineEdit#configInput::placeholder {
    color: #4a5568;
}

QPushButton#accentBtn {
    background-color: #5b8af5;
    color: #ffffff;
    border: none;
    border-radius: 8px;
    font-size: 13px;
    font-weight: bold;
}

QPushButton#accentBtn:hover {
    background-color: #4a7ae0;
}

QPushButton#accentBtn:pressed {
    background-color: #3a6ad0;
}

QPushButton#ghostBtn {
    background-color: transparent;
    color: #7a8599;
    border: 1px solid #2a3545;
    border-radius: 8px;
    font-size: 13px;
}

QPushButton#ghostBtn:hover {
    background-color: #1a2030;
    color: #c0cad8;
    border-color: #3a4555;
}

QTextEdit#logEdit {
    background-color: #0d1117;
    border: 1px solid #1e2530;
    border-radius: 12px;
    color: #a0b0c0;
    padding: 12px;
    selection-background-color: #5b8af5;
}

QFrame#statsFrame {
    background: transparent;
    border: none;
}

QProgressBar#fetchProgress {
    background-color: #1e2530;
    border: none;
    border-radius: 2px;
}

QProgressBar#fetchProgress::chunk {
    background-color: #5b8af5;
    border-radius: 2px;
}
"""


ICON_PATH = _resource_dir() / "icon.png"
ICO_PATH = _resource_dir() / "icon.ico"


def main():
    QApplication.setHighDpiScaleFactorRoundingPolicy(Qt.HighDpiScaleFactorRoundingPolicy.PassThrough)
    app = QApplication(sys.argv)
    app.setStyle("Fusion")
    app.setStyleSheet(GLOBAL_STYLE)

    font = QFont("Segoe UI", 9)
    app.setFont(font)

    icon = QIcon()
    if ICO_PATH.exists():
        icon = QIcon(str(ICO_PATH))
    elif ICON_PATH.exists():
        pixmap = QPixmap(str(ICON_PATH))
        for s in [16, 24, 32, 48, 64, 128, 256]:
            icon.addPixmap(pixmap.scaled(s, s, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation))
    if not icon.isNull():
        app.setWindowIcon(icon)

    win = MainWindow()
    win.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
