// Android 14's predictive back: the gesture's progress goes to the interface, which moves
// the sheet or shrinks the screen Back would close (app/gui/ui/mobile/app.slint); when it
// goes through, the Rust side runs the interface's Back (android/back.rs). Its own class,
// loaded only on Android 14 and newer.
package io.github.scrimas.mhgusaveeditor;

import android.util.Log;
import android.window.BackEvent;
import android.window.OnBackAnimationCallback;

final class Back34 {
    static Object callback() {
        return new OnBackAnimationCallback() {
            @Override
            public void onBackStarted(BackEvent e) {
                Log.i("mhgu-editor", "back started");
                MainActivity.onBack(0, e.getProgress(), e.getSwipeEdge() == BackEvent.EDGE_RIGHT);
            }

            // (since Android 15 this comes once, at 0: the window's touches, which Android
            // reads the progress from, go to Slint; app.slint measures it from them)
            @Override
            public void onBackProgressed(BackEvent e) {
                MainActivity.onBack(1, e.getProgress(), e.getSwipeEdge() == BackEvent.EDGE_RIGHT);
            }

            @Override
            public void onBackCancelled() {
                Log.i("mhgu-editor", "back cancelled");
                MainActivity.onBack(2, 0f, false);
            }

            @Override
            public void onBackInvoked() {
                Log.i("mhgu-editor", "back invoked");
                MainActivity.onBack(3, 1f, false);
            }
        };
    }

    private Back34() {}
}
