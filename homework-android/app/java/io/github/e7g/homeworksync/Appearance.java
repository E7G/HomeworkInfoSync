package io.github.e7g.homeworksync;

import android.content.res.Configuration;

/** Colors by role, with explicit contrast in both themes. */
final class Appearance {
    static final int SYSTEM = 0, LIGHT = 1, DARK = 2;
    static final String PREFS = "appearance", MODE = "mode";
    private static final Appearance DAY = new Appearance(false);
    private static final Appearance NIGHT = new Appearance(true);

    final boolean dark;
    final int page, card, ink, muted, border, accent, soft;
    final int hero, heroInk, heroMuted, highlight, onHighlight, selected, onSelected;
    final int warning, warningSurface, ripple;

    private Appearance(boolean dark) {
        this.dark = dark;
        page = dark ? 0xff111916 : 0xfff5f4ed;
        card = dark ? 0xff1b2721 : 0xfffffef8;
        ink = dark ? 0xffecf3ed : 0xff1c3026;
        muted = dark ? 0xffb4c2b8 : 0xff52665a;
        border = dark ? 0xff36493e : 0xffdce2d5;
        accent = dark ? 0xffb8dbbd : 0xff254d3a;
        soft = dark ? 0xff2a4033 : 0xffe3ebdd;
        hero = dark ? 0xff223a2e : 0xff254d3a;
        heroInk = dark ? 0xfff2f9f2 : 0xffffffff;
        heroMuted = dark ? 0xffcadccd : 0xffe0ebdd;
        highlight = 0xffd6e8b8;
        onHighlight = 0xff183126;
        selected = dark ? 0xffb8dbbd : 0xff254d3a;
        onSelected = dark ? 0xff183126 : 0xffffffff;
        warning = dark ? 0xffffc0a5 : 0xff8a3b1f;
        warningSurface = dark ? 0xff482c21 : 0xfffae8d6;
        ripple = dark ? 0x33d6e8b8 : 0x33407759;
    }

    static int normalize(int mode) {
        return mode == LIGHT || mode == DARK ? mode : SYSTEM;
    }

    static Appearance resolve(int mode, int uiMode) {
        boolean systemDark = (uiMode & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
        return mode == DARK || (normalize(mode) == SYSTEM && systemDark) ? NIGHT : DAY;
    }
}
