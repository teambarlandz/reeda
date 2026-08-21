package io.reeda.app;

import android.content.ContentValues;
import android.content.Context;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;

import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStream;

/**
 * Mirrors the Rust event log to a user-visible location.
 *
 * On API 29+ the file is written to the public Downloads folder through
 * MediaStore (no permission needed for files this app creates); on older
 * API levels it falls back to the app-external files dir, which is still
 * reachable via USB/adb without permissions.
 */
public class LogExporter {
    public static void export(Context context, String fileName, String content) {
        if (Build.VERSION.SDK_INT >= 29) {
            try {
                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, fileName);
                values.put(MediaStore.MediaColumns.MIME_TYPE, "text/plain");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);
                Uri uri = context.getContentResolver()
                        .insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                if (uri != null) {
                    try (OutputStream os = context.getContentResolver().openOutputStream(uri)) {
                        if (os != null) {
                            os.write(content.getBytes("UTF-8"));
                            os.flush();
                        }
                    }
                    return;
                }
            } catch (Exception ignored) {
                // fall through to the app-external copy
            }
        }
        try {
            File dir = context.getExternalFilesDir(null);
            if (dir != null) {
                File target = new File(dir, fileName);
                try (FileOutputStream fos = new FileOutputStream(target)) {
                    fos.write(content.getBytes("UTF-8"));
                }
            }
        } catch (Exception ignored) {
        }
    }
}
