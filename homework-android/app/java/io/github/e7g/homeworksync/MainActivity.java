package io.github.e7g.homeworksync;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.res.ColorStateList;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.Insets;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.InputType;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import java.text.DateFormat;
import java.util.Date;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class MainActivity extends Activity {
    private static final int MATCH = -1, WRAP = -2;
    private static final String[] KEYS = {"chaoxing", "ketangpai", "yuketang"};
    private static final String[] NAMES = {"超星", "课堂派", "长江雨课堂"};
    private static final String[] FULL_NAMES = {"超星 / 学习通", "课堂派", "长江雨课堂"};
    private static final String[] STATUS = {"pending", "done", "overdue", "all"};
    private static final String[] TABS = {"待提交", "已提交", "已过期", "全部"};

    private final Handler main = new Handler(Looper.getMainLooper());
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Runnable tick = new Runnable() {
        @Override public void run() {
            if (page == 0 && !busy) render(true);
            main.postDelayed(this, 60_000);
        }
    };
    private LocalStore store;
    private JSONObject config = new JSONObject(), cache = new JSONObject();
    private final String[] syncNotes = new String[3];
    private LinearLayout screen, header, navigation, content;
    private ScrollView scroll;
    private View navigationDivider;
    private TextView syncInfo;
    private Appearance palette;
    private int appearanceMode;
    private String status = "", readError = "";
    private int page, filter, source = -1;
    private boolean busy, destroyed;
    private QrFlow qrFlow;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        appearanceMode = Appearance.normalize(getSharedPreferences(Appearance.PREFS, MODE_PRIVATE)
                .getInt(Appearance.MODE, Appearance.SYSTEM));
        palette = Appearance.resolve(appearanceMode, getResources().getConfiguration().uiMode);
        Window window = getWindow();
        window.setStatusBarColor(Color.TRANSPARENT);
        window.setNavigationBarColor(Color.TRANSPARENT);
        if (Build.VERSION.SDK_INT >= 29) window.setNavigationBarContrastEnforced(false);
        if (Build.VERSION.SDK_INT >= 30) window.setDecorFitsSystemWindows(false);

        store = new LocalStore(this);
        try { config = store.readConfig(); }
        catch (Exception e) { readError = "无法读取本机登录信息，请重新配置平台。"; }
        cache = store.readCache();
        if (state != null) {
            page = state.getInt("page");
            filter = state.getInt("filter");
            source = state.getInt("source", -1);
        }

        screen = column();
        screen.setOnApplyWindowInsetsListener((v, insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                Insets gaps = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                screen.setPadding(gaps.left, gaps.top, gaps.right, gaps.bottom);
            } else {
                screen.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                        insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            }
            return insets;
        });
        header = row();
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.setPadding(dp(20), dp(8), dp(20), dp(8));
        screen.addView(header, new LinearLayout.LayoutParams(MATCH, WRAP));
        navigation = row();
        navigation.setPadding(dp(16), 0, dp(16), dp(8));
        screen.addView(navigation, new LinearLayout.LayoutParams(MATCH, WRAP));
        navigationDivider = rule();
        screen.addView(navigationDivider);
        scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        scroll.setClipToPadding(false);
        screen.addView(scroll, new LinearLayout.LayoutParams(MATCH, 0, 1));
        FrameLayout widthFrame = new FrameLayout(this);
        scroll.addView(widthFrame, new ScrollView.LayoutParams(MATCH, WRAP));
        content = column();
        content.setPadding(dp(20), dp(20), dp(20), dp(32));
        widthFrame.addView(content, new FrameLayout.LayoutParams(contentWidth(), WRAP, Gravity.TOP | Gravity.CENTER_HORIZONTAL));
        setContentView(screen);
        updateAppearance();
        screen.requestApplyInsets();
        render(false);
        if (configuredCount() > 0) refresh();
    }

    @Override protected void onSaveInstanceState(Bundle out) {
        super.onSaveInstanceState(out);
        out.putInt("page", page);
        out.putInt("filter", filter);
        out.putInt("source", source);
    }

    @Override public void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        content.getLayoutParams().width = contentWidth();
        content.requestLayout();
        if (updateAppearance()) render(true);
    }

    @Override protected void onResume() {
        super.onResume();
        main.removeCallbacks(tick);
        if (content != null && page == 0 && !busy) render(true);
        main.postDelayed(tick, 60_000);
    }

    @Override protected void onPause() {
        main.removeCallbacks(tick);
        super.onPause();
    }

    @Override protected void onDestroy() {
        destroyed = true;
        if (qrFlow != null) qrFlow.cancelled = true;
        main.removeCallbacksAndMessages(null);
        worker.shutdownNow();
        super.onDestroy();
    }

    @Override public void onBackPressed() {
        if (page != 0) navigate(0);
        else super.onBackPressed();
    }

    private int contentWidth() {
        return getResources().getConfiguration().screenWidthDp > 760 ? dp(720) : MATCH;
    }

    private void navigate(int next) {
        page = next;
        render(false);
    }

    private boolean updateAppearance() {
        Appearance next = Appearance.resolve(appearanceMode, getResources().getConfiguration().uiMode);
        boolean changed = palette != next;
        palette = next;
        screen.setBackgroundColor(palette.page);
        navigationDivider.setBackgroundColor(palette.border);
        Window window = getWindow();
        window.setBackgroundDrawable(new ColorDrawable(palette.page));
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = window.getInsetsController();
            int flags = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS |
                    WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
            if (controller != null) controller.setSystemBarsAppearance(palette.dark ? 0 : flags, flags);
        } else {
            int layoutFlags = View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN |
                    View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION;
            int iconFlags = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            window.getDecorView().setSystemUiVisibility(layoutFlags | (palette.dark ? 0 : iconFlags));
        }
        return changed;
    }

    private ContextThemeWrapper themedContext() {
        String name = palette.dark ? "HomeworkThemeDark" : "HomeworkThemeLight";
        int style = getResources().getIdentifier(name, "style", getPackageName());
        return new ContextThemeWrapper(this, style);
    }

    private AlertDialog.Builder dialogBuilder() {
        return new AlertDialog.Builder(themedContext());
    }

    private void pickAppearance() {
        String[] choices = {"跟随系统", "浅色", "深色"};
        dialogBuilder().setTitle("外观").setSingleChoiceItems(choices, appearanceMode, (dialog, mode) -> {
            dialog.dismiss();
            if (mode == appearanceMode) return;
            appearanceMode = mode;
            getSharedPreferences(Appearance.PREFS, MODE_PRIVATE).edit().putInt(Appearance.MODE, mode).apply();
            updateAppearance();
            render(true);
        }).setNegativeButton("取消", null).show();
    }

    private void render(boolean preserveScroll) {
        int y = preserveScroll ? scroll.getScrollY() : 0;
        header.removeAllViews();
        navigation.removeAllViews();
        content.removeAllViews();
        syncInfo = null;
        LinearLayout id = row();
        id.setGravity(Gravity.CENTER_VERTICAL);
        ImageView mark = new ImageView(this);
        mark.setImageResource(getResources().getIdentifier("ic_launcher", "drawable", getPackageName()));
        id.addView(mark, new LinearLayout.LayoutParams(dp(33), dp(33)));
        TextView brand = text("作业同步", 19, palette.ink, true);
        LinearLayout.LayoutParams brandParams = new LinearLayout.LayoutParams(WRAP, WRAP);
        brandParams.leftMargin = dp(10);
        id.addView(brand, brandParams);
        header.addView(id, new LinearLayout.LayoutParams(0, WRAP, 1));
        TextView appearance = action("外观", palette.page, palette.accent, this::pickAppearance);
        LinearLayout.LayoutParams appearanceParams = new LinearLayout.LayoutParams(WRAP, WRAP);
        appearanceParams.rightMargin = dp(6);
        header.addView(appearance, appearanceParams);
        TextView syncButton = action(busy ? "同步中…" : "同步", palette.soft, palette.accent, this::refresh);
        syncButton.setEnabled(!busy);
        syncButton.setAlpha(busy ? .65f : 1f);
        header.addView(syncButton, new LinearLayout.LayoutParams(WRAP, WRAP));
        String[] sections = {"作业", "平台", "同步记录"};
        for (int i = 0; i < sections.length; i++) {
            final int tab = i;
            TextView label = action(sections[i], i == page ? palette.soft : palette.page,
                    i == page ? palette.accent : palette.muted,
                    () -> navigate(tab));
            label.setSelected(i == page);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, WRAP, 1f);
            params.setMargins(dp(4), 0, dp(4), 0);
            navigation.addView(label, params);
        }
        if (page == 0) renderHome();
        else if (page == 1) renderPlatforms();
        else renderHistory();
        scroll.scrollTo(0, y);
        scroll.post(() -> scroll.scrollTo(0, y));
    }

    private void renderHome() {
        if (configuredCount() == 0) {
            LinearLayout hero = box(palette.hero, false, 22);
            hero.addView(kicker("01  /  作业总览", palette.highlight));
            add(hero, text("三个平台，\n一张作业清单。", 33, palette.heroInk, true), 12);
            add(hero, text("连接学习平台，把待交作业、截止时间和提交状态放在一起看。", 15, palette.heroMuted, false), 12);
            add(hero, action("添加学习平台", palette.highlight, palette.onHighlight, () -> navigate(1)), 20);
            content.addView(hero, fill());
            add(content, text("支持的平台", 18, palette.ink, true), 26);
            LinearLayout list = box(palette.card, true, 16);
            for (int i = 0; i < KEYS.length; i++) {
                if (i > 0) list.addView(rule());
                final int idx = i;
                TextView item = action(FULL_NAMES[i] + "  ›", palette.card, palette.accent, () -> editPlatform(idx));
                item.setGravity(Gravity.CENTER_VERTICAL);
                list.addView(item, fill());
            }
            add(content, list, 10);
            if (!readError.isEmpty()) add(content, text(readError, 13, palette.warning, false), 12);
            return;
        }

        JSONObject snapshot = snapshot();
        LinearLayout hero = box(palette.hero, false, 20);
        hero.addView(kicker("01  /  作业总览", palette.highlight));
        LinearLayout metrics = row();
        metrics.setGravity(Gravity.BOTTOM);
        metric(metrics, "待提交", snapshot.optInt("pending"), palette.heroInk, true);
        metric(metrics, "6 小时内", snapshot.optInt("urgent"), palette.highlight, false);
        metric(metrics, "已提交", snapshot.optInt("done"), palette.heroInk, false);
        add(hero, metrics, 12);
        content.addView(hero, fill());
        syncInfo = text(statusLine(), 13, palette.muted, false);
        syncInfo.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        add(content, syncInfo, 12);

        LinearLayout section = row();
        section.setGravity(Gravity.CENTER_VERTICAL);
        section.addView(text("作业清单", 22, palette.ink, true), new LinearLayout.LayoutParams(0, WRAP, 1));
        if (configuredCount() > 1) {
            section.addView(action(source < 0 ? "所有平台 ▾" : NAMES[source] + " ▾", palette.soft, palette.accent, this::pickSource));
        }
        add(content, section, 18);

        LinearLayout filters = row();
        filters.setBaselineAligned(false);
        for (int i = 0; i < STATUS.length; i++) {
            final int selected = i;
            TextView chip = action(TABS[i], i == filter ? palette.selected : palette.soft,
                    i == filter ? palette.onSelected : palette.accent,
                    () -> { filter = selected; render(true); });
            chip.setSelected(i == filter);
            chip.setPadding(dp(2), dp(8), dp(2), dp(8));
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, MATCH, 1f);
            if (i > 0) params.leftMargin = dp(5);
            filters.addView(chip, params);
        }
        add(content, filters, 10);
        JSONArray rows = snapshot.optJSONArray("rows");
        int shown = 0;
        if (rows != null) {
            for (int i = 0; i < rows.length(); i++) {
                JSONObject item = rows.optJSONObject(i);
                if (item == null) continue;
                if (source >= 0 && !NAMES[source].equals(item.optString("platform"))) continue;
                if (filter != 3 && !STATUS[filter].equals(item.optString("state"))) continue;
                add(content, homeworkCard(item), 10);
                shown++;
            }
        }
        if (shown == 0) {
            LinearLayout empty = box(palette.card, true, 18);
            String heading = busy ? "正在同步作业" : lastUpdated() == 0 ? "还没有同步结果" :
                    filter == 0 ? "没有待提交的作业" : "没有符合条件的作业";
            if (snapshot.has("error")) heading = "暂时无法显示作业";
            empty.addView(text(heading, 17, palette.ink, true));
            String hint = busy ? "已有缓存会继续显示。" :
                    lastUpdated() == 0 ? "同步结果和失败原因可在「同步记录」查看。" :
                    "切换上方状态或平台，查看其他作业。";
            add(empty, text(hint, 14, palette.muted, false), 5);
            add(content, empty, 12);
        }
    }

    private View homeworkCard(JSONObject item) {
        LinearLayout card = box(palette.card, true, 16);
        LinearLayout top = row();
        top.setGravity(Gravity.CENTER_VERTICAL);
        top.addView(kicker(item.optString("platform"), palette.accent), new LinearLayout.LayoutParams(0, WRAP, 1));
        String state = item.optString("state");
        boolean attention = item.optBoolean("urgent") && "pending".equals(state) || "overdue".equals(state);
        String tag = "done".equals(state) ? "已提交" : item.optString("label");
        TextView badge = text(tag, 12, attention ? palette.warning : palette.accent, true);
        badge.setPadding(dp(9), dp(4), dp(9), dp(4));
        badge.setBackground(shape(attention ? palette.warningSurface : palette.soft, 9, false));
        top.addView(badge);
        card.addView(top, fill());
        add(card, text(item.optString("title"), 18, palette.ink, true), 7);
        add(card, text(item.optString("course"), 14, palette.muted, false), 3);
        String date = item.optString("deadline");
        String remain = item.isNull("remain") || "done".equals(state) ? "" : item.optString("remain");
        add(card, text(("无截止时间".equals(date) ? date : "截止  " + date) +
                (remain.isEmpty() ? "" : "  ·  " + remain), 13, palette.muted, false), 10);
        String url = item.optString("url");
        if (safeUrl(url)) {
            add(card, action("打开原平台  ↗", palette.soft, palette.accent, () -> openLink(url)), 12);
        }
        return card;
    }

    private void renderPlatforms() {
        sectionIntro("02  /  学习平台", "管理平台", "登录信息只保存在本机。同步时，应用直接连接对应平台。");
        for (int i = 0; i < KEYS.length; i++) {
            final int index = i;
            LinearLayout cell = box(palette.card, true, 16);
            LinearLayout line = row();
            line.setGravity(Gravity.CENTER_VERTICAL);
            line.addView(text(FULL_NAMES[i], 18, palette.ink, true), new LinearLayout.LayoutParams(0, WRAP, 1));
            TextView tag = text(connected(i) ? "已配置" : "未连接", 12, palette.muted, false);
            line.addView(tag);
            cell.addView(line, fill());
            String hint = i == 2 ? "微信扫码登录，也可手动填写已有凭证。" :
                    i == 0 ? "使用学习通账号和密码。" : "使用课堂派注册邮箱和密码。";
            add(cell, text(hint, 14, palette.muted, false), 7);
            add(cell, action(connected(i) ? "管理登录" : "连接平台", palette.soft, palette.accent, () -> editPlatform(index)), 12);
            add(content, cell, 10);
        }
        if (!readError.isEmpty()) add(content, text(readError, 13, palette.warning, false), 12);
    }

    private void renderHistory() {
        sectionIntro("03  /  同步记录", "同步记录", "按平台查看本次同步结果。若请求失败，上次成功的缓存会保留。");
        syncInfo = text(statusLine(), 14, palette.muted, false);
        syncInfo.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        add(content, syncInfo, 14);
        for (int i = 0; i < KEYS.length; i++) {
            LinearLayout cell = box(palette.card, true, 16);
            JSONObject result = cache.optJSONObject(KEYS[i]);
            cell.addView(text(FULL_NAMES[i], 16, palette.ink, true));
            String note = syncNotes[i];
            if (note == null) note = !connected(i) ? "未连接" : result == null ? "尚未同步" : "已缓存";
            add(cell, text(note, 14, palette.muted, false), 5);
            if (connected(i) && result != null) {
                long ts = result.optLong("updated");
                JSONArray items = result.optJSONArray("items");
                if (ts > 0 && items != null) {
                    add(cell, text("上次成功：" + displayTime(ts) + "  ·  " + items.length() + " 项",
                            13, palette.muted, false), 5);
                }
            }
            add(content, cell, 10);
        }
        add(content, action("查看开源项目  ↗", palette.soft, palette.accent,
                () -> openLink("https://github.com/E7G/HomeworkInfoSync")), 18);
    }

    private void refresh() {
        if (busy || qrFlow != null) return;
        if (configuredCount() == 0) { navigate(1); return; }
        final String settings = config.toString();
        final boolean[] enabled = {connected(0), connected(1), connected(2)};
        busy = true;
        status = "正在同步…";
        render(true);
        worker.execute(() -> {
            int ok = 0, errors = 0;
            for (int i = 0; i < KEYS.length && !Thread.currentThread().isInterrupted(); i++) {
                if (!enabled[i]) continue;
                final int idx = i;
                main.post(() -> {
                    if (!destroyed) updateStatus("正在同步 " + NAMES[idx] + "…");
                });
                JSONObject response;
                try { response = object(NativeBridge.fetch(KEYS[idx], settings)); }
                catch (RuntimeException | LinkageError e) { response = new JSONObject(); }
                final JSONArray items = response.optJSONArray("items");
                final String message = response.optString("error", "暂时无法同步，请稍后重试。");
                final long timestamp = System.currentTimeMillis();
                if (items == null) errors++;
                else ok++;
                main.post(() -> {
                    if (destroyed) return;
                    if (items == null) {
                        syncNotes[idx] = message + (cache.has(KEYS[idx]) ? " 已保留缓存。" : "");
                    } else {
                        put(cache, KEYS[idx], put(put(new JSONObject(), "updated", timestamp), "items", items));
                        syncNotes[idx] = "同步成功，获得 " + items.length() + " 项。";
                        try { store.writeCache(cache); }
                        catch (Exception e) { syncNotes[idx] += " 本次结果未能保存到本机。"; }
                    }
                });
            }
            final int succeeded = ok, failed = errors;
            main.post(() -> {
                if (destroyed) return;
                busy = false;
                status = failed == 0 ? "" : succeeded == 0 ? "同步未成功 · 查看同步记录" :
                        "部分平台同步失败 · 查看同步记录";
                render(true);
            });
        });
    }

    private JSONObject snapshot() {
        JSONArray items = new JSONArray();
        for (int i = 0; i < KEYS.length; i++) {
            JSONObject bucket = cache.optJSONObject(KEYS[i]);
            JSONArray array = connected(i) && bucket != null ? bucket.optJSONArray("items") : null;
            if (array == null) continue;
            for (int j = 0; j < array.length(); j++) {
                if (array.optJSONObject(j) != null) items.put(array.optJSONObject(j));
            }
        }
        if (items.length() == 0) return new JSONObject();
        try { return object(NativeBridge.snapshot(items.toString())); }
        catch (RuntimeException | LinkageError e) { return put(new JSONObject(), "error", true); }
    }

    private void editPlatform(int index) {
        if (busy || qrFlow != null) { toast("同步或扫码进行中，请稍候。"); return; }
        if (index == 2) {
            String[] choices = connected(index) ?
                new String[]{"重新扫码登录", "手动填写凭证", "断开平台"} :
                new String[]{"微信扫码登录", "手动填写凭证"};
            dialogBuilder().setTitle(FULL_NAMES[index]).setItems(choices, (dialog, which) -> {
                if (which == 0) startQr();
                else if (which == 1) showCredentialForm(index);
                else disconnect(index);
            }).setNegativeButton("取消", null).show();
        } else {
            showCredentialForm(index);
        }
    }

    private void showCredentialForm(int index) {
        JSONObject current = platform(index);
        LinearLayout fields = column();
        fields.setPadding(dp(20), dp(10), dp(20), dp(4));
        boolean rain = index == 2;
        String loginKey = rain ? "csrftoken" : index == 0 ? "user" : "email";
        String secretKey = rain ? "sessionid" : "password";
        int loginType = rain ? InputType.TYPE_TEXT_VARIATION_PASSWORD : index == 1 ?
                InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS : InputType.TYPE_CLASS_TEXT;
        EditText login = input(fields, rain ? "csrftoken" : index == 0 ? "学习通账号" : "课堂派邮箱",
                current.optString(loginKey), loginType);
        EditText secret = input(fields, rain ? "sessionid" : "密码",
                current.optString(secretKey), InputType.TYPE_TEXT_VARIATION_PASSWORD);
        EditText university = rain ? input(fields, "university_id", current.optString("university_id", "3078"),
                InputType.TYPE_CLASS_NUMBER) : null;
        ScrollView form = new ScrollView(this);
        form.addView(fields);
        AlertDialog.Builder builder = dialogBuilder().setTitle(FULL_NAMES[index])
                .setView(form).setNegativeButton("取消", null).setPositiveButton("保存并同步", null);
        if (connected(index)) builder.setNeutralButton("断开", (d, w) -> disconnect(index));
        AlertDialog dialog = builder.create();
        dialog.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        dialog.show();
        dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v -> {
            String account = login.getText().toString().trim();
            String password = secret.getText().toString();
            if (account.isEmpty()) { login.setError("不能为空"); return; }
            if (password.isEmpty() || (rain && password.trim().isEmpty())) { secret.setError("不能为空"); return; }
            JSONObject changed = put(put(put(new JSONObject(), "enabled", true), loginKey, account),
                    secretKey, rain ? password.trim() : password);
            if (rain) {
                String id = university.getText().toString().trim();
                if (!id.matches("[1-9][0-9]{0,15}")) { university.setError("请输入有效的平台编号"); return; }
                put(changed, "university_id", id);
            }
            if (savePlatform(index, changed)) {
                dialog.dismiss();
                navigate(0);
                refresh();
            }
        });
    }

    private void disconnect(int index) {
        dialogBuilder().setTitle("断开" + NAMES[index] + "？")
                .setMessage("会清除该平台在本机保存的登录信息和作业缓存。")
                .setNegativeButton("取消", null).setPositiveButton("断开", (d, w) -> {
                    if (savePlatform(index, new JSONObject())) render(false);
                }).show();
    }

    private boolean savePlatform(int index, JSONObject value) {
        if (busy) return false;
        String key = KEYS[index];
        JSONObject next = object(config.toString());
        if (value.length() == 0) next.remove(key);
        else put(next, key, value);
        try {
            if (!platform(index).toString().equals(value.toString()) && cache.has(key)) {
                // Invalidate the old account before writing credentials for a new one.
                JSONObject valid = object(cache.toString());
                valid.remove(key);
                store.writeCache(valid);
                cache = valid;
            }
            store.writeConfig(next);
        } catch (Exception e) {
            toast("无法保存到本机，请重试。");
            return false;
        }
        config = next;
        syncNotes[index] = null;
        status = "";
        readError = "";
        if (source == index || configuredCount() < 2) source = -1;
        return true;
    }

    private void startQr() {
        LinearLayout panel = column();
        panel.setPadding(dp(20), dp(10), dp(20), dp(4));
        TextView hint = text("正在生成二维码…", 15, palette.muted, false);
        panel.addView(hint, fill());
        ImageView image = new ImageView(this);
        image.setContentDescription("长江雨课堂登录二维码");
        image.setVisibility(View.GONE);
        image.setScaleType(ImageView.ScaleType.FIT_CENTER);
        LinearLayout.LayoutParams size = new LinearLayout.LayoutParams(MATCH, dp(210));
        size.topMargin = dp(8);
        panel.addView(image, size);
        ProgressBar progress = new ProgressBar(themedContext());
        LinearLayout.LayoutParams spin = new LinearLayout.LayoutParams(dp(32), dp(32));
        spin.gravity = Gravity.CENTER_HORIZONTAL;
        spin.topMargin = dp(16);
        panel.addView(progress, spin);
        AlertDialog dialog = dialogBuilder().setTitle("微信扫码登录")
                .setView(panel).setNegativeButton("取消", null).create();
        QrFlow flow = new QrFlow(main, (bytes) -> {
            Bitmap bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
            if (bitmap == null) return;
            progress.setVisibility(View.GONE);
            image.setImageBitmap(bitmap);
            image.setVisibility(View.VISIBLE);
            hint.setText("用另一台设备的微信扫码。也可截图后切到微信「扫一扫」，从相册选取。");
        });
        qrFlow = flow;
        dialog.setOnDismissListener(d -> {
            flow.cancelled = true;
            if (qrFlow == flow) qrFlow = null;
        });
        dialog.show();
        String university = platform(2).optString("university_id", "3078");
        worker.execute(() -> {
            JSONObject result;
            try { result = object(NativeBridge.qrLogin(university, flow)); }
            catch (RuntimeException | LinkageError e) { result = new JSONObject(); }
            final JSONObject response = result;
            main.post(() -> {
                if (destroyed || flow.cancelled || qrFlow != flow) return;
                if (response.optBoolean("enabled") && savePlatform(2, response)) {
                    qrFlow = null;
                    dialog.dismiss();
                    navigate(0);
                    refresh();
                } else {
                    image.setVisibility(View.GONE);
                    progress.setVisibility(View.GONE);
                    hint.setText(response.optBoolean("enabled") ? "已扫码，但无法保存登录信息。" :
                            response.optString("error", "无法连接雨课堂，请稍后重试。"));
                    dialog.getButton(AlertDialog.BUTTON_NEGATIVE).setText("关闭");
                }
            });
        });
    }

    private interface QrImage { void show(byte[] image); }

    private static final class QrFlow implements NativeBridge.QrCallback {
        private final Handler handler;
        private final QrImage image;
        private volatile boolean cancelled;
        QrFlow(Handler handler, QrImage image) { this.handler = handler; this.image = image; }
        @Override public boolean isCancelled() { return cancelled; }
        @Override public void onQr(byte[] png) {
            handler.post(() -> { if (!cancelled) image.show(png); });
        }
    }

    private void pickSource() {
        String[] labels = new String[configuredCount() + 1];
        int[] indices = new int[labels.length];
        labels[0] = "所有平台";
        indices[0] = -1;
        int pos = 1, selected = 0;
        for (int i = 0; i < KEYS.length; i++) {
            if (!connected(i)) continue;
            if (source == i) selected = pos;
            labels[pos] = FULL_NAMES[i];
            indices[pos++] = i;
        }
        dialogBuilder().setTitle("筛选平台").setSingleChoiceItems(labels, selected, (d, i) -> {
            source = indices[i];
            d.dismiss();
            render(true);
        }).setNegativeButton("取消", null).show();
    }

    private JSONObject platform(int index) {
        JSONObject value = config.optJSONObject(KEYS[index]);
        return value == null ? new JSONObject() : value;
    }

    private boolean connected(int index) { return platform(index).optBoolean("enabled"); }

    private int configuredCount() {
        int total = 0;
        for (int i = 0; i < KEYS.length; i++) if (connected(i)) total++;
        return total;
    }

    private long lastUpdated() {
        long max = 0;
        for (int i = 0; i < KEYS.length; i++) {
            JSONObject bucket = cache.optJSONObject(KEYS[i]);
            if (connected(i) && bucket != null) max = Math.max(max, bucket.optLong("updated"));
        }
        return max;
    }

    private String statusLine() {
        if (!status.isEmpty()) return status;
        long updated = lastUpdated();
        return updated == 0 ? "暂无同步结果" : "最近成功同步  " + displayTime(updated);
    }

    private void updateStatus(String value) {
        status = value;
        if (syncInfo != null) syncInfo.setText(statusLine());
    }

    private static String displayTime(long time) {
        return DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(new Date(time));
    }

    private static boolean safeUrl(String url) {
        Uri uri = Uri.parse(url);
        return "https".equalsIgnoreCase(uri.getScheme()) && uri.getHost() != null;
    }

    private void openLink(String url) {
        if (!safeUrl(url)) return;
        try { startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE)); }
        catch (ActivityNotFoundException e) { toast("没有可打开链接的应用。"); }
    }

    private void sectionIntro(String number, String title, String caption) {
        content.addView(kicker(number, palette.accent));
        add(content, text(title, 30, palette.ink, true), 6);
        add(content, text(caption, 15, palette.muted, false), 7);
        View spacer = new View(this);
        content.addView(spacer, new LinearLayout.LayoutParams(MATCH, dp(9)));
    }

    private void metric(LinearLayout row, String label, int value, int color, boolean large) {
        LinearLayout cell = column();
        TextView number = text(Integer.toString(value), large ? 45 : 29, color, true);
        number.setTypeface(Typeface.create("sans-serif-condensed", Typeface.BOLD));
        number.setIncludeFontPadding(false);
        cell.addView(number);
        add(cell, text(label, 13, palette.heroMuted, false), 4);
        row.addView(cell, new LinearLayout.LayoutParams(0, WRAP, large ? 1.3f : 1f));
    }

    private EditText input(LinearLayout parent, String label, String value, int type) {
        add(parent, text(label, 13, palette.accent, true), 8);
        EditText field = new EditText(themedContext());
        field.setSingleLine(true);
        field.setSaveEnabled(false);
        field.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);
        field.setImeOptions(EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING);
        field.setInputType(InputType.TYPE_CLASS_TEXT | type);
        if (type == InputType.TYPE_CLASS_NUMBER) field.setInputType(type);
        field.setHint(label);
        field.setContentDescription(label);
        field.setText(value);
        field.setTextSize(16);
        add(parent, field, 2);
        return field;
    }

    private TextView text(String value, int sp, int color, boolean bold) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextSize(sp);
        view.setTextColor(color);
        view.setTypeface(Typeface.create("sans-serif", bold ? Typeface.BOLD : Typeface.NORMAL));
        view.setLineSpacing(dp(2), 1f);
        return view;
    }

    private TextView kicker(String value, int color) {
        TextView view = text(value, 12, color, true);
        view.setLetterSpacing(.10f);
        return view;
    }

    private TextView action(String label, int bg, int fg, Runnable click) {
        TextView view = text(label, 14, fg, true);
        view.setMinHeight(dp(48));
        view.setGravity(Gravity.CENTER);
        view.setPadding(dp(13), dp(8), dp(13), dp(8));
        view.setBackground(new RippleDrawable(ColorStateList.valueOf(palette.ripple),
                shape(bg, 12, false), null));
        view.setFocusable(true);
        view.setOnClickListener(v -> click.run());
        view.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName("android.widget.Button");
            }
        });
        return view;
    }

    private LinearLayout box(int color, boolean border, int padding) {
        LinearLayout box = column();
        box.setBackground(shape(color, 18, border));
        box.setPadding(dp(padding), dp(padding), dp(padding), dp(padding));
        return box;
    }

    private GradientDrawable shape(int color, int radius, boolean border) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(color);
        d.setCornerRadius(dp(radius));
        if (border) d.setStroke(dp(1), palette.border);
        return d;
    }

    private View rule() {
        View line = new View(this);
        line.setBackgroundColor(palette.border);
        line.setLayoutParams(new LinearLayout.LayoutParams(MATCH, dp(1)));
        return line;
    }

    private LinearLayout row() { return new LinearLayout(this); }
    private LinearLayout column() {
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        return col;
    }
    private LinearLayout.LayoutParams fill() { return new LinearLayout.LayoutParams(MATCH, WRAP); }
    private void add(LinearLayout parent, View child, int gap) {
        LinearLayout.LayoutParams p = fill();
        p.topMargin = dp(gap);
        parent.addView(child, p);
    }
    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    private void toast(String msg) { Toast.makeText(this, msg, Toast.LENGTH_SHORT).show(); }

    private static JSONObject object(String json) {
        if (json == null) return new JSONObject();
        try { return new JSONObject(json); }
        catch (JSONException e) { return new JSONObject(); }
    }

    private static JSONObject put(JSONObject object, String key, Object value) {
        try { return object.put(key, value); }
        catch (JSONException e) { throw new IllegalArgumentException(e); }
    }
}
