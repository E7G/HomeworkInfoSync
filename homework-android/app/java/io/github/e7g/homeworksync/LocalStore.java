package io.github.e7g.homeworksync;

import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.AtomicFile;
import org.json.JSONException;
import org.json.JSONObject;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

final class LocalStore {
    private static final String KEY_NAME = "homework-config-v1";
    private final AtomicFile credentials;
    private final AtomicFile cache;

    LocalStore(Context context) {
        File dir = context.getNoBackupFilesDir();
        credentials = new AtomicFile(new File(dir, "platforms.bin"));
        cache = new AtomicFile(new File(dir, "homework.json"));
    }

    private SecretKey key(boolean create) throws GeneralSecurityException, IOException {
        KeyStore keys = KeyStore.getInstance("AndroidKeyStore");
        keys.load(null);
        SecretKey existing = (SecretKey) keys.getKey(KEY_NAME, null);
        if (existing != null) return existing;
        if (!create) throw new GeneralSecurityException("Credential key not found");
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        generator.init(new KeyGenParameterSpec.Builder(KEY_NAME,
                    KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());
        return generator.generateKey();
    }

    JSONObject readConfig() throws IOException, GeneralSecurityException, JSONException {
        byte[] data;
        try {
            data = credentials.readFully();
        } catch (FileNotFoundException e) {
            return new JSONObject();
        }
        if (data.length < 29 || data[0] != 1) throw new IOException("Unsupported credentials");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, key(false), new GCMParameterSpec(128, data, 1, 12));
        byte[] value = cipher.doFinal(data, 13, data.length - 13);
        try {
            return new JSONObject(new String(value, StandardCharsets.UTF_8));
        } finally {
            Arrays.fill(value, (byte) 0);
        }
    }

    void writeConfig(JSONObject config) throws IOException, GeneralSecurityException {
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, key(true));
        byte[] iv = cipher.getIV();
        if (iv.length != 12) throw new GeneralSecurityException("Unsupported IV length");
        byte[] value = config.toString().getBytes(StandardCharsets.UTF_8);
        try {
            byte[] encrypted = cipher.doFinal(value);
            byte[] data = new byte[13 + encrypted.length];
            data[0] = 1;
            System.arraycopy(iv, 0, data, 1, 12);
            System.arraycopy(encrypted, 0, data, 13, encrypted.length);
            write(credentials, data);
        } finally {
            Arrays.fill(value, (byte) 0);
        }
    }

    JSONObject readCache() {
        try {
            return new JSONObject(new String(cache.readFully(), StandardCharsets.UTF_8));
        } catch (IOException | JSONException e) {
            return new JSONObject();
        }
    }

    void writeCache(JSONObject value) throws IOException {
        write(cache, value.toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void write(AtomicFile file, byte[] data) throws IOException {
        FileOutputStream stream = file.startWrite();
        try {
            stream.write(data);
            file.finishWrite(stream);
        } catch (IOException e) {
            file.failWrite(stream);
            throw e;
        }
    }
}
