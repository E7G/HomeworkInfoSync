#pragma once

#include <cstdint>

#ifdef _WIN32
#define HW_EXPORT __declspec(dllexport)
#else
#define HW_EXPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct HwItemC {
    const char *title;
    const char *course;
    const char *platform;
    const char *deadline;
    const char *remain;
    const char *color;
    const char *bg_color;
    const char *urgency_label;
    const char *url;
} HwItemC;

typedef struct HwStatsC {
    int32_t total;
    int32_t pending;
    int32_t urgent;
    int32_t done;
} HwStatsC;

typedef struct UiCallbacks {
    void *ctx;
    void (*on_window)(void *ctx, void *window);
    void (*poll)(void *ctx);
    void (*refresh)(void *ctx, int32_t silent);
    void (*save_config)(void *ctx, const char *json_utf8);
    void (*ykt_qr_login)(void *ctx);
    int32_t (*get_config_json)(void *ctx, char *buf, int32_t cap);
} UiCallbacks;
HW_EXPORT void ui_on_progress(void *window, int32_t step, int32_t total, const char *msg);
HW_EXPORT void ui_on_fetch_done(void *window, const HwItemC *items, int32_t count, HwStatsC stats);
HW_EXPORT void ui_on_status(void *window, const char *msg);
HW_EXPORT void ui_on_log(void *window, const char *msg);
HW_EXPORT void ui_on_qr_png(void *window, const uint8_t *data, int32_t len);
HW_EXPORT void ui_on_ykt_status(void *window, const char *msg);
HW_EXPORT void ui_set_refresh_enabled(void *window, int32_t enabled);

HW_EXPORT int32_t ui_run(UiCallbacks cb, int32_t argc, char **argv);

#ifdef __cplusplus
}
#endif
