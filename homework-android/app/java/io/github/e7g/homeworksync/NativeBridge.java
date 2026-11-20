package io.github.e7g.homeworksync;

final class NativeBridge {
    static { System.loadLibrary("homework_android"); }

    interface QrCallback {
        void onQr(byte[] png);
        boolean isCancelled();
    }

    static native String fetch(String platform, String config);
    static native String snapshot(String rawItems);
    static native String qrLogin(String universityId, QrCallback callback);

    private NativeBridge() {}
}
