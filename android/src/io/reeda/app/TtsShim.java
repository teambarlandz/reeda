package io.reeda.app;

import android.content.Context;
import android.media.AudioAttributes;
import android.speech.tts.TextToSpeech;
import android.speech.tts.UtteranceProgressListener;
import android.util.Log;

/**
 * Read-aloud TTS shim (docs/TTS_SPEC.md §2).
 *
 * Singleton wrapping {@link TextToSpeech}; Rust calls speak/stop/rate/pitch
 * through JNI, and the UtteranceProgressListener feeds events back to the
 * native bridge via {@link #onEvent}.
 *
 * minSdk 26 guarantees {@code onRangeStart} word-level callbacks.
 */
public class TtsShim implements TextToSpeech.OnInitListener {
    private static final String TAG = "ReedaTts";

    private static TtsShim instance;

    private TextToSpeech tts;
    private boolean ready;
    private Context appContext;

    private TtsShim(Context context) {
        appContext = context.getApplicationContext();
        tts = new TextToSpeech(appContext, this);
        tts.setAudioAttributes(new AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build());
        tts.setOnUtteranceProgressListener(new UtteranceProgressListener() {
            @Override
            public void onStart(String utteranceId) {
                onEvent(0, Long.parseLong(utteranceId), 0, 0);
            }

            @Override
            public void onRangeStart(String utteranceId, int start, int end, int frame) {
                onEvent(1, Long.parseLong(utteranceId), start, end);
            }

            @Override
            public void onDone(String utteranceId) {
                onEvent(2, Long.parseLong(utteranceId), 0, 0);
            }

            @Override
            public void onError(String utteranceId) {
                onEvent(3, Long.parseLong(utteranceId), 0, 0);
            }
        });
    }

    /** Initialize the singleton; no-op if already initialized. */
    public static synchronized void init(Context context) {
        if (instance == null) {
            instance = new TtsShim(context);
        }
    }

    /** Returns the initialized singleton (null before init). */
    public static synchronized TtsShim get() {
        return instance;
    }

    /** Enqueue text as the given utterance id (QUEUE_ADD). */
    public int speak(String text, long utteranceId) {
        if (!ready) {
            return TextToSpeech.ERROR;
        }
        return tts.speak(text, TextToSpeech.QUEUE_ADD, null, String.valueOf(utteranceId));
    }

    /** Stop all speech. */
    public int stop() {
        return tts.stop();
    }

    /** Set the speech rate (0.5–2.5). */
    public int setRate(float rate) {
        return tts.setSpeechRate(rate);
    }

    /** Set the pitch (0.5–1.5). */
    public int setPitch(float pitch) {
        return tts.setPitch(pitch);
    }

    @Override
    public void onInit(int status) {
        ready = status == TextToSpeech.SUCCESS;
        if (!ready) {
            Log.e(TAG, "TextToSpeech init failed: " + status);
        }
    }

    /**
     * Native callback (binder thread). Implemented in Rust
     * (Java_io_reeda_app_TtsShim_onEvent).
     */
    private static native void onEvent(int type, long utteranceId, int start, int end);
}