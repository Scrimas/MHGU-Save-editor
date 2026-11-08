// Android 14's predictive back: the gesture's progress goes to the interface, which moves
// the sheet or shrinks the screen Back would close (app/gui/ui/mobile/app.slint); when it
// goes through, Slint gets its Back key. Its own class, loaded only on Android 14 and newer.
package io.github.scrimas.mhgusaveeditor;

import android.util.Log;
import android.window.BackEvent;
import android.window.OnBackAnimationCallback;

final class Back34 {
    static Object callback(final MainActivity activity) {
        return new OnBackAnimationCallback() {
            @Override
            public void onBackStarted(BackEvent e) {
                tenth = -1;
                Log.i("mhgu-editor", "back started");
                MainActivity.onBack(0, e.getProgress(), e.getSwipeEdge() == BackEvent.EDGE_RIGHT);
            }

            // the progress in steps of 0.02, logged as it changes, with the finger's x
            private int tenth = -1;

            @Override
            public void onBackProgressed(BackEvent e) {
                int t = (int) (e.getProgress() * 50);
                if (t != tenth) {
                    tenth = t;
                    Log.i("mhgu-editor", "back progress " + e.getProgress() + " x=" + e.getTouchX());
                }
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
                activity.backInvoked();
            }
        };
    }

    private Back34() {}
}
