package io.reeda.app;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;
import android.util.Log;

/**
 * Read-aloud foreground service (docs/TTS_SPEC.md §2).
 *
 * Runs while narration is active: media notification with
 * play/pause/stop/skip-back/skip-forward/speed actions (PendingIntents →
 * JNI → Rust engine), audio focus handling (GAIN/LOSS/TRANSIENT/DUCK), and
 * a partial wake-lock so the device can sleep while narrating.
 *
 * Started/stopped from Rust via {@link #start}/{@link #stop}; action taps
 * arrive in {@link #onStartCommand} and are forwarded to the native
 * {@link #onAction} (Java_io_reeda_app_NarrationService_onAction).
 */
public class NarrationService extends Service {
    private static final String TAG = "ReedaNarration";

    private static final String CHANNEL_ID = "reeda_narration";
    private static final String EXTRA_ACTION = "io.reeda.app.action";
    private static final int NOTIF_ID = 1;

    /** Action ids — must match `android_bridge.rs` ACT_* constants. */
    static final int ACT_PLAY = 0;
    static final int ACT_PAUSE = 1;
    static final int ACT_STOP = 2;
    static final int ACT_SKIP_BACK = 3;
    static final int ACT_SKIP_FORWARD = 4;
    static final int ACT_SPEED_UP = 5;
    static final int ACT_SPEED_DOWN = 6;

    private AudioManager audioManager;
    private AudioFocusRequest focusRequest;
    private PowerManager.WakeLock wakeLock;
    private boolean ducked;
    private int volumeBeforeDuck;

    /** Forward an action to the Rust engine (binder-safe; pushes to the
     *  native event queue). */
    private static native void onAction(int action);

    /** Start the foreground service (called from Rust on first speak). */
    public static void start(Context context) {
        context.startForegroundService(new Intent(context, NarrationService.class));
    }

    /** Stop the foreground service (called from Rust on narration end). */
    public static void stop(Context context) {
        context.stopService(new Intent(context, NarrationService.class));
    }

    @Override
    public void onCreate() {
        super.onCreate();
        audioManager = (AudioManager) getSystemService(Context.AUDIO_SERVICE);
        focusRequest = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(new AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build())
                .setOnAudioFocusChangeListener(this::onAudioFocusChange)
                .build();
        PowerManager pm = (PowerManager) getSystemService(Context.POWER_SERVICE);
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "reeda:narration");
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && intent.hasExtra(EXTRA_ACTION)) {
            int action = intent.getIntExtra(EXTRA_ACTION, -1);
            onAction(action);
            if (action == ACT_STOP) {
                stopForeground(STOP_FOREGROUND_REMOVE);
                stopSelf();
            }
            return START_NOT_STICKY;
        }
        startForegroundCompat();
        acquireWakeLock();
        if (audioManager.requestAudioFocus(focusRequest) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
            Log.w(TAG, "audio focus not granted; narrating without focus");
        }
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        releaseWakeLock();
        if (focusRequest != null) {
            audioManager.abandonAudioFocusRequest(focusRequest);
        }
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void startForegroundCompat() {
        Notification notification = buildNotification();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        } else {
            startForeground(NOTIF_ID, notification);
        }
    }

    private Notification buildNotification() {
        NotificationManager nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        nm.createNotificationChannel(new NotificationChannel(
                CHANNEL_ID, "Narration", NotificationManager.IMPORTANCE_LOW));

        Notification.Builder b = new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("Reeda")
                .setContentText("Reading aloud")
                .setContentIntent(openAppIntent())
                .setSmallIcon(android.R.drawable.ic_media_play)
                .setOngoing(true)
                .setShowWhen(false)
                .setCategory(Notification.CATEGORY_TRANSPORT)
                .setVisibility(Notification.VISIBILITY_PUBLIC);

        Notification.MediaStyle style = new Notification.MediaStyle();
        style.setShowActionsInCompactView(0, 1, 2);
        b.setStyle(style);

        b.addAction(mediaAction(ACT_SKIP_BACK, "Previous", android.R.drawable.ic_media_previous));
        b.addAction(mediaAction(ACT_PLAY, "Play", android.R.drawable.ic_media_play));
        b.addAction(mediaAction(ACT_SKIP_FORWARD, "Next", android.R.drawable.ic_media_next));
        b.addAction(mediaAction(ACT_PAUSE, "Pause", android.R.drawable.ic_media_pause));
        b.addAction(mediaAction(ACT_STOP, "Stop", android.R.drawable.ic_menu_close_clear_cancel));
        b.addAction(mediaAction(ACT_SPEED_UP, "Faster", android.R.drawable.arrow_up_float));
        b.addAction(mediaAction(ACT_SPEED_DOWN, "Slower", android.R.drawable.arrow_down_float));
        return b.build();
    }

    private PendingIntent openAppIntent() {
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        return PendingIntent.getActivity(this, 0, launch,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
    }

    private Notification.Action mediaAction(int action, String title, int icon) {
        Intent intent = new Intent(this, NarrationService.class)
                .putExtra(EXTRA_ACTION, action);
        PendingIntent pi = PendingIntent.getService(this, action, intent,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Action.Builder(icon, title, pi).build();
    }

    private void onAudioFocusChange(int change) {
        switch (change) {
            case AudioManager.AUDIOFOCUS_GAIN:
                if (ducked) {
                    audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, volumeBeforeDuck, 0);
                    ducked = false;
                }
                onAction(ACT_PLAY);
                break;
            case AudioManager.AUDIOFOCUS_LOSS:
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
                onAction(ACT_PAUSE);
                break;
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK:
                volumeBeforeDuck = audioManager.getStreamVolume(AudioManager.STREAM_MUSIC);
                int ducked = (int) (audioManager.getStreamMaxVolume(AudioManager.STREAM_MUSIC) * 0.3f);
                audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, ducked, 0);
                this.ducked = true;
                break;
            default:
                break;
        }
    }

    private void acquireWakeLock() {
        if (wakeLock != null && !wakeLock.isHeld()) {
            wakeLock.acquire();
        }
    }

    private void releaseWakeLock() {
        if (wakeLock != null && wakeLock.isHeld()) {
            wakeLock.release();
        }
    }
}