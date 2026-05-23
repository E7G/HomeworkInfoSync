#include "ui_bridge.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QEvent>
#include <QtCore/QFile>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTime>
#include <QtCore/QTimer>
#include <QtCore/QStringList>
#include <QtCore/QUrl>
#include <QtGui/QDesktopServices>
#include <QtGui/QIcon>
#include <QtGui/QFont>
#include <QtGui/QMouseEvent>
#include <QtGui/QPixmap>
#include <QtWidgets/QApplication>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QFrame>
#include <QtWidgets/QHBoxLayout>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QProgressBar>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QScrollArea>
#include <QtWidgets/QStackedWidget>
#include <QtWidgets/QTextEdit>
#include <QtWidgets/QVBoxLayout>

#include <vector>

static const char *kStyle = R"(
QMainWindow { background-color: #141820; }
QFrame#sidebar { background-color: #0d1117; border-right: 1px solid #1e2530; }
QLabel#sidebarLogo { color: #5b8af5; padding-left: 16px; font-weight: bold; }
QFrame#sidebarSep { background-color: #1e2530; border: none; max-height: 1px; }
QLabel#sidebarVersion { color: #3a4555; font-size: 11px; }
QPushButton#navBtn {
    background: transparent; border: none; border-radius: 8px;
    margin: 2px 8px; padding: 0 12px; text-align: left;
    color: #7a8599; font-size: 13px;
}
QPushButton#navBtn:hover { background-color: #1a2030; color: #c0cad8; }
QPushButton#navBtn:checked {
    background-color: #1a2540; color: #5b8af5; border-left: 3px solid #5b8af5;
}
QFrame#mainContent { background-color: #141820; border: none; }
QFrame#titleBar { background-color: #0f1318; border-bottom: 1px solid #1e2530; }
QFrame#statusBar { background-color: #0f1318; border-top: 1px solid #1e2530; }
QScrollArea#homeScroll, QScrollArea { background: transparent; border: none; }
QScrollBar:vertical { background: transparent; width: 6px; margin: 0; }
QScrollBar::handle:vertical { background: #2a3545; border-radius: 3px; min-height: 30px; }
QScrollBar::handle:vertical:hover { background: #3a4555; }
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }
QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical { background: transparent; }
QLineEdit#configInput {
    background-color: #1a2030; border: 1px solid #2a3545; border-radius: 8px;
    padding: 0 14px; color: #e0e0e0; font-size: 13px;
    selection-background-color: #5b8af5;
}
QLineEdit#configInput:focus { border-color: #5b8af5; background-color: #1e2538; }
QLineEdit#configInput::placeholder { color: #4a5568; }
QPushButton#accentBtn {
    background-color: #5b8af5; color: #ffffff; border: none;
    border-radius: 8px; font-size: 13px; font-weight: bold;
}
QPushButton#accentBtn:hover { background-color: #4a7ae0; }
QPushButton#accentBtn:pressed { background-color: #3a6ad0; }
QPushButton#accentBtn:disabled { background-color: #3a4555; color: #8892a4; }
QPushButton#ghostBtn {
    background-color: transparent; color: #7a8599;
    border: 1px solid #2a3545; border-radius: 8px; font-size: 13px;
}
QPushButton#ghostBtn:hover {
    background-color: #1a2030; color: #c0cad8; border-color: #3a4555;
}
QCheckBox#platformEnabled { color: #a0aec0; font-size: 13px; }
QCheckBox#platformEnabled::indicator {
    width: 18px; height: 18px; border-radius: 4px; border: 2px solid #4a5568;
}
QCheckBox#platformEnabled::indicator:checked {
    background-color: #5b8af5; border-color: #5b8af5;
}
QTextEdit#logEdit {
    background-color: #0d1117; border: 1px solid #1e2530; border-radius: 12px;
    color: #a0b0c0; padding: 12px; selection-background-color: #5b8af5;
}
QFrame#configCard {
    background-color: #1e2330; border: 1px solid #2a3040; border-radius: 12px;
}
QProgressBar#fetchProgress { background-color: #1e2530; border: none; border-radius: 2px; }
QProgressBar#fetchProgress::chunk { background-color: #5b8af5; border-radius: 2px; }
)";

static void pushIconCandidates(QStringList *paths, const QString &start) {
    QDir dir(start);
    if (!dir.exists()) {
        return;
    }
    paths->append(dir.filePath("icon.ico"));
    paths->append(dir.filePath("icon.png"));
    for (int i = 0; i < 8; ++i) {
        if (!dir.cdUp()) {
            break;
        }
        paths->append(dir.filePath("icon.ico"));
        paths->append(dir.filePath("icon.png"));
    }
}

static QIcon loadAppIcon() {
    QStringList paths;
    pushIconCandidates(&paths, QCoreApplication::applicationDirPath());
    pushIconCandidates(&paths, QDir::currentPath());
    paths.removeDuplicates();
    for (const QString &path : paths) {
        if (QFile::exists(path)) {
            QIcon icon(path);
            if (!icon.isNull()) {
                return icon;
            }
        }
    }
    return QIcon();
}

class MainWindow : public QMainWindow {
public:
    explicit MainWindow(UiCallbacks callbacks, QWidget *parent = nullptr)
        : QMainWindow(parent), cb_(callbacks) {
        setWindowTitle("HomeworkSync");
        setMinimumSize(820, 560);
        resize(900, 620);
        setWindowFlags(Qt::FramelessWindowHint);
        buildUi();
        poll_timer_ = new QTimer(this);
        QObject::connect(poll_timer_, &QTimer::timeout, [this]() {
            if (cb_.poll) {
                cb_.poll(cb_.ctx);
            }
        });
        poll_timer_->start(80);
    }

    void startup() {
        if (cb_.poll) {
            cb_.poll(cb_.ctx);
        }
    }

    void setProgress(int step, int total, const QString &msg) {
        progress_->setMaximum(qMax(1, total));
        progress_->setValue(step);
        progress_->setVisible(true);
        status_label_->setText(msg);
        if (total > 0 && step >= total) {
            QTimer::singleShot(800, this, [this]() { progress_->hide(); });
        }
    }

    void hideProgress() {
        progress_->setVisible(false);
    }

    void setStatus(const QString &msg) { status_label_->setText(msg); }

    void setLog(const QString &msg) { log_edit_->setPlainText(msg); }

    void setRefreshEnabled(bool on) { refresh_btn_->setEnabled(on); }

    void showHomePage() {
        if (stack_) {
            stack_->setCurrentIndex(0);
        }
        for (int i = 0; i < nav_buttons_.size(); ++i) {
            nav_buttons_[i]->setChecked(i == 0);
        }
    }

    void updateHomework(const HwItemC *items, int count, const HwStatsC &stats) {
        stat_total_val_->setText(QString::number(stats.total));
        stat_pending_val_->setText(QString::number(stats.pending));
        stat_urgent_val_->setText(QString::number(stats.urgent));
        stat_done_val_->setText(QString::number(stats.done));

        QLayoutItem *child;
        while ((child = cards_layout_->takeAt(0)) != nullptr) {
            if (child->widget()) {
                child->widget()->deleteLater();
            }
            delete child;
        }

        if (count == 0) {
            auto *empty = new QLabel("没有待完成的作业！");
            empty->setAlignment(Qt::AlignCenter);
            empty->setStyleSheet("color: #66bb6a; font-size: 16px; padding: 40px;");
            cards_layout_->addWidget(empty);
            return;
        }

        for (int i = 0; i < count; ++i) {
            cards_layout_->addWidget(makeCard(items[i]));
        }
        cards_layout_->addStretch();
    }

    void setQrPng(const uint8_t *pngData, int len) {
        QPixmap px;
        if (px.loadFromData(pngData, len, "PNG")) {
            ykt_qr_->setPixmap(px.scaled(184, 184, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        }
    }

    void setYktStatus(const QString &text) {
        ykt_status_->setText(text);
        if (text.contains("成功") || text.contains("已保存")) {
            ykt_status_->setStyleSheet("color: #66bb6a; font-size: 12px;");
        } else if (text.contains("失败") || text.contains("超时")) {
            ykt_status_->setStyleSheet("color: #ef5350; font-size: 12px;");
        } else if (text.contains("等待")) {
            ykt_status_->setStyleSheet("color: #ffa726; font-size: 12px;");
        } else {
            ykt_status_->setStyleSheet("color: #7a8599; font-size: 12px;");
        }
    }

    void loadConfigFields() {
        if (!cb_.get_config_json) {
            return;
        }
        std::vector<char> buf(65536);
        int n = cb_.get_config_json(cb_.ctx, buf.data(), static_cast<int32_t>(buf.size()));
        if (n <= 0) {
            return;
        }
        QJsonDocument doc = QJsonDocument::fromJson(QByteArray(buf.data(), n));
        QJsonObject root = doc.object();
        auto cx = root.value("chaoxing").toObject();
        cx_enable_->setChecked(cx.value("enabled").toBool());
        cx_user_->setText(cx.value("user").toString());
        cx_pass_->setText(cx.value("password").toString());
        auto ktp = root.value("ketangpai").toObject();
        ktp_enable_->setChecked(ktp.value("enabled").toBool());
        ktp_email_->setText(ktp.value("email").toString());
        ktp_pass_->setText(ktp.value("password").toString());
        auto ykt = root.value("yuketang").toObject();
        ykt_enable_->setChecked(ykt.value("enabled").toBool());
        if (!ykt.value("csrftoken").toString().isEmpty() && !ykt.value("sessionid").toString().isEmpty()) {
            ykt_status_->setText("已登录（凭证已保存）");
            ykt_status_->setStyleSheet("color: #66bb6a; font-size: 12px;");
        }
    }

protected:
    void mousePressEvent(QMouseEvent *event) override {
        if (event->button() == Qt::LeftButton && titlebar_->geometry().contains(event->pos())) {
            drag_pos_ = event->globalPosition().toPoint() - frameGeometry().topLeft();
        }
        QMainWindow::mousePressEvent(event);
    }

    void mouseMoveEvent(QMouseEvent *event) override {
        if (!drag_pos_.isNull() && (event->buttons() & Qt::LeftButton)) {
            move(event->globalPosition().toPoint() - drag_pos_);
        }
        QMainWindow::mouseMoveEvent(event);
    }

    void mouseReleaseEvent(QMouseEvent *event) override {
        drag_pos_ = QPoint();
        QMainWindow::mouseReleaseEvent(event);
    }

    void mouseDoubleClickEvent(QMouseEvent *event) override {
        if (titlebar_->geometry().contains(event->pos())) {
            if (isMaximized()) {
                showNormal();
            } else {
                showMaximized();
            }
        }
        QMainWindow::mouseDoubleClickEvent(event);
    }

    void onResetConfig() {
        cx_enable_->setChecked(false);
        cx_user_->clear();
        cx_pass_->clear();
        ktp_enable_->setChecked(false);
        ktp_email_->clear();
        ktp_pass_->clear();
        ykt_enable_->setChecked(false);
        ykt_qr_->clear();
        setYktStatus("未登录");
    }

    void onSaveConfig() {
        QJsonObject root;
        QJsonObject cx;
        cx.insert("enabled", cx_enable_->isChecked());
        cx.insert("user", cx_user_->text());
        cx.insert("password", cx_pass_->text());
        root.insert("chaoxing", cx);
        QJsonObject ktp;
        ktp.insert("enabled", ktp_enable_->isChecked());
        ktp.insert("email", ktp_email_->text());
        ktp.insert("password", ktp_pass_->text());
        root.insert("ketangpai", ktp);
        QJsonObject ykt;
        ykt.insert("enabled", ykt_enable_->isChecked());
        QByteArray prev = QByteArray();
        if (cb_.get_config_json) {
            std::vector<char> buf(65536);
            int n = cb_.get_config_json(cb_.ctx, buf.data(), static_cast<int32_t>(buf.size()));
            if (n > 0) {
                prev = QByteArray(buf.data(), n);
            }
        }
        QJsonObject prevYkt = QJsonDocument::fromJson(prev).object().value("yuketang").toObject();
        ykt.insert("csrftoken", prevYkt.value("csrftoken").toString());
        ykt.insert("sessionid", prevYkt.value("sessionid").toString());
        ykt.insert("university_id", prevYkt.value("university_id").toString("3078"));
        root.insert("yuketang", ykt);
        QByteArray json = QJsonDocument(root).toJson(QJsonDocument::Compact);
        if (cb_.save_config) {
            cb_.save_config(cb_.ctx, json.constData());
        }
    }

private:
    UiCallbacks cb_;
    QTimer *poll_timer_ = nullptr;
    QPoint drag_pos_;
    QFrame *titlebar_ = nullptr;
    QStackedWidget *stack_ = nullptr;
    QVector<QPushButton *> nav_buttons_;
    QLabel *status_label_ = nullptr;
    QPushButton *refresh_btn_ = nullptr;
    QProgressBar *progress_ = nullptr;
    QVBoxLayout *cards_layout_ = nullptr;
    QTextEdit *log_edit_ = nullptr;
    QLabel *stat_total_val_ = nullptr;
    QLabel *stat_pending_val_ = nullptr;
    QLabel *stat_urgent_val_ = nullptr;
    QLabel *stat_done_val_ = nullptr;
    QCheckBox *cx_enable_ = nullptr;
    QLineEdit *cx_user_ = nullptr;
    QLineEdit *cx_pass_ = nullptr;
    QCheckBox *ktp_enable_ = nullptr;
    QLineEdit *ktp_email_ = nullptr;
    QLineEdit *ktp_pass_ = nullptr;
    QCheckBox *ykt_enable_ = nullptr;
    QLabel *ykt_qr_ = nullptr;
    QLabel *ykt_status_ = nullptr;

    QWidget *makeCard(const HwItemC &h) {
        const QString color = QString::fromUtf8(h.color ? h.color : "#90a4ae");
        const QString bg = QString::fromUtf8(h.bg_color ? h.bg_color : "rgba(144,164,174,0.08)");

        auto *card = new QFrame();
        card->setObjectName("hwCard");
        card->setCursor(h.url && h.url[0] ? Qt::PointingHandCursor : Qt::ArrowCursor);
        card->setStyleSheet(QString(R"(
            QFrame#hwCard {
                background-color: #1e2330;
                border: 1px solid #2a3040;
                border-radius: 12px;
                border-left: 4px solid %1;
                padding: 2px;
            }
            QFrame#hwCard:hover {
                border: 1px solid #3a4050;
                border-left: 4px solid %1;
                background-color: #222838;
            }
        )")
                                .arg(color));

        auto *layout = new QVBoxLayout(card);
        layout->setSpacing(6);
        layout->setContentsMargins(18, 14, 18, 14);

        auto *top = new QHBoxLayout();
        top->setSpacing(10);
        auto *badge = new QLabel(QString("  %1  ").arg(QString::fromUtf8(h.platform)));
        badge->setStyleSheet(QString(R"(
            background-color: %1; color: %2;
            border-radius: 10px; padding: 2px 10px;
            font-size: 11px; font-weight: bold;
        )")
                                .arg(bg, color));
        top->addWidget(badge);
        auto *course = new QLabel(QString::fromUtf8(h.course));
        course->setStyleSheet("color: #e0e0e0; font-size: 13px; font-weight: 600;");
        top->addWidget(course);
        top->addStretch();
        auto *urg = new QLabel(QString::fromUtf8(h.urgency_label));
        urg->setStyleSheet(QString(R"(
            color: %1; font-size: 12px; font-weight: bold;
            background-color: %2; border-radius: 10px; padding: 2px 10px;
        )")
                               .arg(color, bg));
        top->addWidget(urg);
        if (h.url && h.url[0]) {
            auto *link = new QLabel("↗");
            link->setStyleSheet("color: #5b8af5; font-size: 16px; font-weight: bold;");
            link->setToolTip("点击打开作业页面");
            top->addWidget(link);
        }
        layout->addLayout(top);

        auto *title = new QLabel(QString::fromUtf8(h.title));
        title->setStyleSheet("color: #ffffff; font-size: 15px; font-weight: 500;");
        title->setWordWrap(true);
        layout->addWidget(title);

        auto *bottom = new QHBoxLayout();
        auto *deadline = new QLabel(QString("截止: %1").arg(QString::fromUtf8(h.deadline)));
        deadline->setStyleSheet("color: #7a8599; font-size: 12px;");
        bottom->addWidget(deadline);
        bottom->addStretch();
        if (h.remain && h.remain[0]) {
            auto *rem = new QLabel(QString::fromUtf8(h.remain));
            rem->setStyleSheet(
                QString("color: %1; font-size: 12px; font-weight: bold;").arg(color));
            bottom->addWidget(rem);
        }
        layout->addLayout(bottom);

        if (h.url && h.url[0]) {
            card->setProperty("hwUrl", QString::fromUtf8(h.url));
            card->installEventFilter(this);
        }
        return card;
    }

    QFrame *makeStat(const QString &label, const QString &color, QLabel **valOut) {
        auto *frame = new QFrame();
        frame->setObjectName("statCard");
        frame->setStyleSheet(QString(R"(
            QFrame#statCard {
                background-color: #1e2330;
                border: 1px solid #2a3040;
                border-radius: 12px;
                border-top: 3px solid %1;
            }
        )")
                                 .arg(color));
        auto *lay = new QVBoxLayout(frame);
        lay->setContentsMargins(16, 12, 16, 12);
        lay->setSpacing(2);
        auto *val = new QLabel("0");
        val->setStyleSheet(QString("color: %1; font-size: 24px; font-weight: bold;").arg(color));
        *valOut = val;
        lay->addWidget(val);
        auto *name = new QLabel(label);
        name->setStyleSheet("color: #7a8599; font-size: 12px;");
        lay->addWidget(name);
        return frame;
    }

    bool eventFilter(QObject *obj, QEvent *event) override {
        if (event->type() == QEvent::MouseButtonPress) {
            auto *me = static_cast<QMouseEvent *>(event);
            if (me->button() == Qt::LeftButton) {
                auto *w = qobject_cast<QWidget *>(obj);
                if (w && w->property("hwUrl").isValid()) {
                    QString url = w->property("hwUrl").toString();
                    if (!url.isEmpty()) {
                        QDesktopServices::openUrl(QUrl(url));
                    }
                }
            }
        }
        return QMainWindow::eventFilter(obj, event);
    }

    void buildUi() {
        auto *central = new QWidget(this);
        setCentralWidget(central);
        auto *root = new QHBoxLayout(central);
        root->setContentsMargins(0, 0, 0, 0);
        root->setSpacing(0);

        auto *sidebar = new QFrame();
        sidebar->setObjectName("sidebar");
        sidebar->setFixedWidth(180);
        auto *sbLay = new QVBoxLayout(sidebar);
        sbLay->setContentsMargins(0, 0, 0, 0);
        auto *logo = new QLabel("  HomeworkSync");
        logo->setObjectName("sidebarLogo");
        logo->setFixedHeight(56);
        sbLay->addWidget(logo);
        auto *sep = new QFrame();
        sep->setObjectName("sidebarSep");
        sep->setFixedHeight(1);
        sbLay->addWidget(sep);
        sbLay->addSpacing(8);
        const char *nav[][2] = {{"⌂", "作业"}, {"⚙", "配置"}, {"≡", "日志"}};
        for (int i = 0; i < 3; ++i) {
            auto *btn = new QPushButton(QString("%1  %2").arg(nav[i][0], nav[i][1]));
            btn->setObjectName("navBtn");
            btn->setCheckable(true);
            btn->setFixedHeight(48);
            btn->setCursor(Qt::PointingHandCursor);
            btn->setProperty("pageIndex", i);
            QObject::connect(btn, &QPushButton::clicked, [this, btn]() {
                for (auto *b : nav_buttons_) {
                    b->setChecked(b == btn);
                }
                stack_->setCurrentIndex(btn->property("pageIndex").toInt());
                if (btn->property("pageIndex").toInt() == 1) {
                    loadConfigFields();
                }
            });
            nav_buttons_.append(btn);
            sbLay->addWidget(btn);
        }
        if (!nav_buttons_.isEmpty()) {
            nav_buttons_[0]->setChecked(true);
        }
        sbLay->addStretch();
        auto *ver = new QLabel("  v0.2.0");
        ver->setObjectName("sidebarVersion");
        ver->setFixedHeight(32);
        sbLay->addWidget(ver);
        root->addWidget(sidebar);

        auto *right = new QFrame();
        right->setObjectName("mainContent");
        auto *rightLay = new QVBoxLayout(right);
        rightLay->setContentsMargins(0, 0, 0, 0);

        titlebar_ = new QFrame();
        titlebar_->setObjectName("titleBar");
        titlebar_->setFixedHeight(42);
        auto *tbLay = new QHBoxLayout(titlebar_);
        tbLay->setContentsMargins(16, 0, 8, 0);
        auto *titleLbl = new QLabel("HomeworkSync");
        titleLbl->setStyleSheet("color: #a0aec0; font-size: 12px; font-weight: bold;");
        tbLay->addWidget(titleLbl);
        tbLay->addStretch();
        auto addTitleBtn = [&](const char *text, const char *color, const char *hoverBg, auto slot) {
            auto *b = new QPushButton(text);
            b->setObjectName("titleBtn");
            b->setFixedSize(36, 28);
            b->setStyleSheet(QString(R"(
                QPushButton#titleBtn {
                    color: %1; border: none; border-radius: 4px;
                    font-size: 14px; font-weight: bold; background: transparent;
                }
                QPushButton#titleBtn:hover {
                    background-color: %2; color: #ffffff;
                }
            )")
                                   .arg(color, hoverBg));
            QObject::connect(b, &QPushButton::clicked, slot);
            tbLay->addWidget(b);
        };
        addTitleBtn("—", "#6b7a8d", "#3a4050", [this]() { showMinimized(); });
        addTitleBtn("□", "#6b7a8d", "#3a4050", [this]() {
            if (isMaximized()) {
                showNormal();
            } else {
                showMaximized();
            }
        });
        addTitleBtn("×", "#ef5350", "#c62828", [this]() { close(); });
        rightLay->addWidget(titlebar_);

        stack_ = new QStackedWidget();
        stack_->addWidget(buildHomePage());
        stack_->addWidget(buildConfigPage());
        stack_->addWidget(buildLogPage());
        rightLay->addWidget(stack_, 1);

        auto *statusBar = new QFrame();
        statusBar->setObjectName("statusBar");
        statusBar->setFixedHeight(28);
        auto *sbRow = new QHBoxLayout(statusBar);
        sbRow->setContentsMargins(16, 0, 16, 0);
        status_label_ = new QLabel("就绪");
        status_label_->setStyleSheet("color: #5a6577; font-size: 11px;");
        sbRow->addWidget(status_label_);
        sbRow->addStretch();
        auto *time = new QLabel(QTime::currentTime().toString("hh:mm"));
        time->setStyleSheet("color: #5a6577; font-size: 11px;");
        sbRow->addWidget(time);
        rightLay->addWidget(statusBar);

        root->addWidget(right, 1);
    }

    QWidget *buildHomePage() {
        auto *page = new QWidget();
        auto *lay = new QVBoxLayout(page);
        lay->setContentsMargins(28, 24, 28, 20);
        lay->setSpacing(16);
        auto *header = new QHBoxLayout();
        auto *title = new QLabel("作业提醒");
        title->setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;");
        header->addWidget(title);
        header->addStretch();
        refresh_btn_ = new QPushButton("刷新");
        refresh_btn_->setObjectName("accentBtn");
        refresh_btn_->setFixedSize(80, 36);
        refresh_btn_->setCursor(Qt::PointingHandCursor);
        QObject::connect(refresh_btn_, &QPushButton::clicked, [this]() {
            if (cb_.refresh) {
                cb_.refresh(cb_.ctx, 0);
            }
        });
        header->addWidget(refresh_btn_);
        lay->addLayout(header);

        auto *statsFrame = new QFrame();
        statsFrame->setObjectName("statsFrame");
        auto *statsRow = new QHBoxLayout(statsFrame);
        statsRow->setContentsMargins(0, 0, 0, 0);
        statsRow->setSpacing(12);
        statsRow->addWidget(makeStat("总作业", "#42a5f5", &stat_total_val_));
        statsRow->addWidget(makeStat("未提交", "#ffa726", &stat_pending_val_));
        statsRow->addWidget(makeStat("紧急", "#ef5350", &stat_urgent_val_));
        statsRow->addWidget(makeStat("已完成", "#66bb6a", &stat_done_val_));
        lay->addWidget(statsFrame);

        auto *scroll = new QScrollArea();
        scroll->setObjectName("homeScroll");
        scroll->setWidgetResizable(true);
        scroll->setFrameShape(QFrame::NoFrame);
        auto *cardsHost = new QWidget();
        cards_layout_ = new QVBoxLayout(cardsHost);
        cards_layout_->setSpacing(10);
        cards_layout_->addStretch();
        scroll->setWidget(cardsHost);
        lay->addWidget(scroll, 1);

        progress_ = new QProgressBar();
        progress_->setObjectName("fetchProgress");
        progress_->setTextVisible(false);
        progress_->setFixedHeight(4);
        progress_->hide();
        lay->addWidget(progress_);

        return page;
    }

    QWidget *buildConfigPage() {
        auto *page = new QWidget();
        auto *lay = new QVBoxLayout(page);
        lay->setContentsMargins(28, 24, 28, 20);
        lay->setSpacing(16);
        auto *header = new QHBoxLayout();
        auto *cfgTitle = new QLabel("平台配置");
        cfgTitle->setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;");
        header->addWidget(cfgTitle);
        header->addStretch();
        auto *save = new QPushButton("保存");
        save->setObjectName("accentBtn");
        save->setFixedSize(80, 36);
        save->setCursor(Qt::PointingHandCursor);
        QObject::connect(save, &QPushButton::clicked, [this]() { onSaveConfig(); });
        header->addWidget(save);
        auto *reset = new QPushButton("重置");
        reset->setObjectName("ghostBtn");
        reset->setFixedSize(80, 36);
        reset->setCursor(Qt::PointingHandCursor);
        QObject::connect(reset, &QPushButton::clicked, [this]() { onResetConfig(); });
        header->addWidget(reset);
        lay->addLayout(header);

        auto *scroll = new QScrollArea();
        scroll->setWidgetResizable(true);
        scroll->setFrameShape(QFrame::NoFrame);
        auto *host = new QWidget();
        auto *col = new QVBoxLayout(host);
        col->setSpacing(16);

        auto addPlatform = [&](const QString &title, const QString &hintText, auto fillHeader,
                               auto fillBody) {
            auto *card = new QFrame();
            card->setObjectName("configCard");
            auto *cl = new QVBoxLayout(card);
            cl->setContentsMargins(20, 16, 20, 16);
            cl->setSpacing(12);
            auto *hdr = new QHBoxLayout();
            auto *name = new QLabel(title);
            name->setStyleSheet("color: #ffffff; font-size: 15px; font-weight: bold;");
            hdr->addWidget(name);
            hdr->addStretch();
            fillHeader(hdr);
            cl->addLayout(hdr);
            auto *sep = new QFrame();
            sep->setFrameShape(QFrame::HLine);
            sep->setStyleSheet("background-color: #2a3040; border: none; max-height: 1px;");
            cl->addWidget(sep);
            fillBody(cl);
            if (!hintText.isEmpty()) {
                auto *hint = new QLabel(hintText);
                hint->setWordWrap(true);
                hint->setStyleSheet("color: #5a6577; font-size: 11px; padding-left: 102px;");
                cl->addWidget(hint);
            }
            col->addWidget(card);
        };

        addPlatform("超星/学习通", "使用超星/学习通的手机号或学号登录",
                    [this](QHBoxLayout *hdr) {
                        cx_enable_ = new QCheckBox("启用");
                        cx_enable_->setObjectName("platformEnabled");
                        hdr->addWidget(cx_enable_);
                    },
                    [this](QVBoxLayout *cl) {
                        addField(cl, "账号", &cx_user_, false, "请输入账号");
                        addField(cl, "密码", &cx_pass_, true, "请输入密码");
                    });

        addPlatform("课堂派", "使用课堂派注册邮箱登录",
                    [this](QHBoxLayout *hdr) {
                        ktp_enable_ = new QCheckBox("启用");
                        ktp_enable_->setObjectName("platformEnabled");
                        hdr->addWidget(ktp_enable_);
                    },
                    [this](QVBoxLayout *cl) {
                        addField(cl, "邮箱", &ktp_email_, false, "请输入邮箱");
                        addField(cl, "密码", &ktp_pass_, true, "请输入密码");
                    });

        addPlatform("长江雨课堂",
                    "点击扫码登录按钮，使用微信长江雨课堂小程序扫描二维码\n"
                    "登录成功后凭证会自动保存",
                    [this](QHBoxLayout *hdr) {
                        ykt_enable_ = new QCheckBox("启用");
                        ykt_enable_->setObjectName("platformEnabled");
                        hdr->addWidget(ykt_enable_);
                    },
                    [this](QVBoxLayout *cl) {
                        auto *row = new QHBoxLayout();
                        row->setSpacing(16);
                        ykt_qr_ = new QLabel();
                        ykt_qr_->setFixedSize(200, 200);
                        ykt_qr_->setAlignment(Qt::AlignCenter);
                        ykt_qr_->setStyleSheet("background-color: #0d1117; border-radius: 8px;");
                        row->addWidget(ykt_qr_);
                        auto *info = new QVBoxLayout();
                        info->setSpacing(8);
                        auto *qrBtn = new QPushButton("扫码登录");
                        qrBtn->setObjectName("accentBtn");
                        qrBtn->setFixedSize(120, 38);
                        qrBtn->setCursor(Qt::PointingHandCursor);
                        QObject::connect(qrBtn, &QPushButton::clicked, [this]() {
                            setYktStatus("等待扫码...");
                            ykt_qr_->clear();
                            if (cb_.ykt_qr_login) {
                                cb_.ykt_qr_login(cb_.ctx);
                            }
                        });
                        info->addWidget(qrBtn);
                        ykt_status_ = new QLabel("未登录");
                        ykt_status_->setStyleSheet("color: #7a8599; font-size: 12px;");
                        info->addWidget(ykt_status_);
                        info->addStretch();
                        row->addLayout(info);
                        cl->addLayout(row);
                    });

        col->addStretch();
        scroll->setWidget(host);
        lay->addWidget(scroll, 1);
        loadConfigFields();
        return page;
    }

    QWidget *buildLogPage() {
        auto *page = new QWidget();
        auto *lay = new QVBoxLayout(page);
        lay->setContentsMargins(28, 24, 28, 20);
        lay->setSpacing(16);
        auto *title = new QLabel("运行日志");
        title->setStyleSheet("color: #ffffff; font-size: 22px; font-weight: bold;");
        lay->addWidget(title);
        log_edit_ = new QTextEdit();
        log_edit_->setObjectName("logEdit");
        log_edit_->setReadOnly(true);
        log_edit_->setFont(QFont("Cascadia Code", 9));
        lay->addWidget(log_edit_, 1);
        return page;
    }

    void addField(QVBoxLayout *parent, const QString &label, QLineEdit **out, bool secret,
                  const QString &placeholder) {
        auto *row = new QHBoxLayout();
        row->setSpacing(12);
        auto *lbl = new QLabel(label);
        lbl->setFixedWidth(90);
        lbl->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
        lbl->setStyleSheet("color: #8892a4; font-size: 13px;");
        row->addWidget(lbl);
        auto *edit = new QLineEdit();
        edit->setObjectName("configInput");
        edit->setPlaceholderText(placeholder);
        edit->setFixedHeight(38);
        if (secret) {
            edit->setEchoMode(QLineEdit::Password);
            auto *toggle = new QCheckBox("显示");
            toggle->setFixedWidth(56);
            toggle->setStyleSheet("color: #6b7a8d; font-size: 11px;");
            QObject::connect(toggle, &QCheckBox::toggled, [edit](bool checked) {
                edit->setEchoMode(checked ? QLineEdit::Normal : QLineEdit::Password);
            });
            *out = edit;
            row->addWidget(edit, 1);
            row->addWidget(toggle);
        } else {
            *out = edit;
            row->addWidget(edit, 1);
        }
        parent->addLayout(row);
    }
};

extern "C" void ui_on_progress(void *window, int32_t step, int32_t total, const char *msg) {
    auto *w = static_cast<MainWindow *>(window);
    w->setProgress(step, total, QString::fromUtf8(msg));
}

extern "C" void ui_on_fetch_done(void *window, const HwItemC *items, int32_t count, HwStatsC stats) {
    auto *w = static_cast<MainWindow *>(window);
    w->updateHomework(items, count, stats);
    w->hideProgress();
    w->setRefreshEnabled(true);
    w->showHomePage();
}

extern "C" void ui_on_status(void *window, const char *msg) {
    static_cast<MainWindow *>(window)->setStatus(QString::fromUtf8(msg));
}

extern "C" void ui_on_log(void *window, const char *msg) {
    static_cast<MainWindow *>(window)->setLog(QString::fromUtf8(msg));
}

extern "C" void ui_on_qr_png(void *window, const uint8_t *data, int32_t len) {
    static_cast<MainWindow *>(window)->setQrPng(data, len);
}

extern "C" void ui_on_ykt_status(void *window, const char *msg) {
    static_cast<MainWindow *>(window)->setYktStatus(QString::fromUtf8(msg));
}

extern "C" void ui_set_refresh_enabled(void *window, int32_t enabled) {
    static_cast<MainWindow *>(window)->setRefreshEnabled(enabled != 0);
}

extern "C" int32_t ui_run(UiCallbacks cb, int32_t argc, char **argv) {
    QApplication app(argc, argv);
    QApplication::setStyle("Fusion");
    QApplication::setFont(QFont("Segoe UI", 9));
    app.setStyleSheet(kStyle);
    const QIcon appIcon = loadAppIcon();
    if (!appIcon.isNull()) {
        app.setWindowIcon(appIcon);
    }
    MainWindow w(cb);
    if (!appIcon.isNull()) {
        w.setWindowIcon(appIcon);
    }
    if (cb.on_window) {
        cb.on_window(cb.ctx, &w);
    }
    w.show();
    w.startup();
    return static_cast<int32_t>(app.exec());
}
