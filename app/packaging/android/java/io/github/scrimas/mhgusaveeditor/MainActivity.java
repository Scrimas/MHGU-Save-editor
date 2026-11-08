// The editor's activity: Slint draws in the NativeActivity's window, the Rust side
// (app/gui/src/android) runs the app. This adds what native code cannot do itself: the
// system's folder and file pickers (their answer comes back as an activity result), reading
// and writing the documents they hand out, the wallpaper's colours, opening links, and
// Android 14's predictive back (Back34.java).
//
// Methods called from Rust never throw: a failure is null (then lastError()) or a string
// starting with "!". No lambdas: the SDK's android.jar has no LambdaMetafactory for javac.
package io.github.scrimas.mhgusaveeditor;

import android.app.NativeActivity;
import android.content.ClipData;
import android.content.Intent;
import android.content.UriPermission;
import android.content.res.Resources;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import android.provider.OpenableColumns;
import android.view.View;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;

import java.io.ByteArrayOutputStream;
import java.io.FileOutputStream;
import java.io.InputStream;

public class MainActivity extends NativeActivity {
    // the picker's answer (app/gui/src/android/mod.rs): the URIs one per line, "" when
    // cancelled, "!why" when there is no picker
    static native void onPicked(int request, String uris);

    // the predictive back gesture (Rust: android/back.rs): 0 started, 1 progressed, 2
    // cancelled, 3 done; how far (0 to 1); from the right edge
    static native void onBack(int phase, float progress, boolean fromRight);

    // activity results of the pickers: 0x4000 | the request (its low 14 bits)
    private static final int PICKED = 0x4000;

    private volatile String lastError = "";

    // Predictive back (Android 14 and newer). Slint registers a callback that turns Back
    // into its Back key; registered all the time, it would keep Android from ever playing
    // its back-to-home animation, and it gets no progress to animate with. So Slint's
    // callback is kept here instead (getOnBackInvokedDispatcher below), and this activity
    // registers its own animated one (Back34) only while the app has something for Back to
    // do (setBack, from the Slint side); otherwise Back is Android's.
    private boolean created;
    private boolean wantBack;
    private Object slintBack;  // OnBackInvokedCallback
    private Object animated;   // OnBackAnimationCallback, while registered

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        // edge to edge on Android 10 to 14 too (Android 15 does it for every app): the
        // interface keeps clear of the bars with Slint's safe-area insets
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().setDecorFitsSystemWindows(false);
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        // (Activity registered its own default Back during super.onCreate, with the real
        // dispatcher; from here on, what registers is Slint)
        created = true;
    }

    @Override
    public OnBackInvokedDispatcher getOnBackInvokedDispatcher() {
        final OnBackInvokedDispatcher real = super.getOnBackInvokedDispatcher();
        if (!created || Build.VERSION.SDK_INT < 34) return real;
        return new OnBackInvokedDispatcher() {
            @Override
            public void registerOnBackInvokedCallback(int priority, OnBackInvokedCallback callback) {
                slintBack = callback;
                updateBack();
            }

            @Override
            public void unregisterOnBackInvokedCallback(OnBackInvokedCallback callback) {
                if (callback == slintBack) {
                    slintBack = null;
                    updateBack();
                } else {
                    real.unregisterOnBackInvokedCallback(callback);
                }
            }
        };
    }

    // whether Back has something to do in the app (a sheet, a page, changes to ask about)
    public void setBack(final boolean on) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                wantBack = on;
                updateBack();
            }
        });
    }

    // on the UI thread
    private void updateBack() {
        if (Build.VERSION.SDK_INT < 34) return;
        OnBackInvokedDispatcher real = super.getOnBackInvokedDispatcher();
        boolean on = wantBack && slintBack != null;
        if (on != (animated != null)) android.util.Log.i("mhgu-editor", on ? "back: the app's" : "back: Android's");
        if (on && animated == null) {
            animated = Back34.callback(this);
            real.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, (OnBackInvokedCallback) animated);
        } else if (!on && animated != null) {
            real.unregisterOnBackInvokedCallback((OnBackInvokedCallback) animated);
            animated = null;
        }
    }

    // the gesture went through: Slint's Back key, which the interface acts on
    void backInvoked() {
        if (slintBack != null) ((OnBackInvokedCallback) slintBack).onBackInvoked();
    }

    public String lastError() {
        return lastError;
    }

    // kind: 0 a folder, 1 a file, 2 one or more files, 3 a new file named `name`
    public void pick(final int request, final int kind, final String mime, final String name) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent i;
                if (kind == 0) {
                    i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                } else if (kind == 3) {
                    i = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType(mime.isEmpty() ? "application/octet-stream" : mime);
                    i.putExtra(Intent.EXTRA_TITLE, name);
                } else {
                    i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType(mime.isEmpty() ? "*/*" : mime);
                    if (kind == 2) i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                }
                i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                try {
                    startActivityForResult(i, PICKED | (request & 0x3fff));
                } catch (Exception e) {
                    onPicked(request, "!" + e.getMessage());
                }
            }
        });
    }

    @Override
    protected void onActivityResult(int code, int result, Intent data) {
        super.onActivityResult(code, result, data);
        if ((code & 0xc000) != PICKED) return;
        StringBuilder out = new StringBuilder();
        if (result == RESULT_OK && data != null) {
            int flags = data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            ClipData clip = data.getClipData();
            if (clip != null) {
                for (int k = 0; k < clip.getItemCount(); k++) add(out, clip.getItemAt(k).getUri(), flags);
            } else if (data.getData() != null) {
                add(out, data.getData(), flags);
            }
        }
        onPicked(code & 0x3fff, out.toString());
    }

    // kept across runs, so the folder or file opens again next time without a picker
    private void add(StringBuilder out, Uri u, int flags) {
        try {
            getContentResolver().takePersistableUriPermission(u, flags);
        } catch (Exception e) {
            // (not every provider lets a permission be kept: then it lasts this run)
        }
        if (out.length() > 0) out.append('\n');
        out.append(u.toString());
    }

    // "1" while a picked folder or file is still ours (its permission kept)
    public String canAccess(String uri) {
        try {
            Uri u = Uri.parse(uri);
            for (UriPermission p : getContentResolver().getPersistedUriPermissions()) {
                if (p.getUri().equals(u) && p.isReadPermission() && p.isWritePermission()) return "1";
            }
            return "0";
        } catch (Exception e) {
            return "!" + e.getMessage();
        }
    }

    public byte[] read(String uri) {
        try (InputStream in = getContentResolver().openInputStream(Uri.parse(uri))) {
            if (in == null) throw new java.io.IOException("no data");
            ByteArrayOutputStream b = new ByteArrayOutputStream(1 << 20);
            byte[] buf = new byte[1 << 16];
            int n;
            while ((n = in.read(buf)) > 0) b.write(buf, 0, n);
            return b.toByteArray();
        } catch (Exception e) {
            lastError = e.getMessage() != null ? e.getMessage() : e.toString();
            return null;
        }
    }

    // replaces the document's bytes; null when written
    public String write(String uri, byte[] data) {
        Uri u = Uri.parse(uri);
        String first = null;
        // "rwt" truncates in place; some providers only take "wt"
        for (String mode : new String[] {"rwt", "wt"}) {
            try (ParcelFileDescriptor fd = getContentResolver().openFileDescriptor(u, mode)) {
                if (fd == null) throw new java.io.IOException("no file");
                try (FileOutputStream out = new FileOutputStream(fd.getFileDescriptor())) {
                    out.write(data);
                    out.flush();
                    try {
                        out.getFD().sync();
                    } catch (Exception e) {
                        // (a provider without fsync: the read back still checks the bytes)
                    }
                }
                return null;
            } catch (Exception e) {
                if (first == null) first = e.getMessage() != null ? e.getMessage() : e.toString();
            }
        }
        return first;
    }

    public String displayName(String uri) {
        Uri u = Uri.parse(uri);
        try (Cursor c = getContentResolver().query(u, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst() && !c.isNull(0)) return c.getString(0);
        } catch (Exception e) {
            // falls back to the URI's own name
        }
        String last = u.getLastPathSegment();
        return last != null ? last : uri;
    }

    // the save in a picked folder: "place\nsystem's URI\nsystem_backup's URI (or empty)", in
    // the folder, its save_0, or save/save_0 (the Recomp's whole files folder picked)
    public String findSave(String treeUri) {
        try {
            Uri tree = Uri.parse(treeUri);
            String root = DocumentsContract.getTreeDocumentId(tree);
            String[][] ways = {{}, {"save_0"}, {"save", "save_0"}};
            for (String[] way : ways) {
                String id = root;
                StringBuilder place = new StringBuilder(place(tree, root));
                for (String part : way) {
                    id = child(tree, id, part, true);
                    if (id == null) break;
                    place.append('/').append(part);
                }
                if (id == null) continue;
                String sys = child(tree, id, "system", false);
                if (sys == null) continue;
                String bak = child(tree, id, "system_backup", false);
                return place + "\n" + DocumentsContract.buildDocumentUriUsingTree(tree, sys) + "\n"
                    + (bak == null ? "" : DocumentsContract.buildDocumentUriUsingTree(tree, bak).toString());
            }
            return "!none";
        } catch (Exception e) {
            return "!" + e.getMessage();
        }
    }

    // the folder as the device's storage names it ("Download/MHGU"), else its own name
    private String place(Uri tree, String id) {
        int colon = id.indexOf(':');
        if ("com.android.externalstorage.documents".equals(tree.getAuthority()) && colon >= 0 && colon < id.length() - 1) {
            return id.substring(colon + 1);
        }
        return displayName(DocumentsContract.buildDocumentUriUsingTree(tree, id).toString());
    }

    private String child(Uri tree, String parent, String name, boolean folder) {
        Uri kids = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent);
        String[] cols = {Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE};
        try (Cursor c = getContentResolver().query(kids, cols, null, null, null)) {
            while (c != null && c.moveToNext()) {
                if (name.equalsIgnoreCase(c.getString(1)) && folder == Document.MIME_TYPE_DIR.equals(c.getString(2))) return c.getString(0);
            }
        }
        return null;
    }

    public String openUrl(final String url) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
                } catch (Exception e) {
                    lastError = e.toString();
                }
            }
        });
        return "";
    }

    // the Material roles Android 14 names, light and dark (the fixed ones have one value)
    private static final String[] ROLES = {
        "primary", "on_primary", "primary_container", "on_primary_container",
        "secondary", "on_secondary", "secondary_container", "on_secondary_container",
        "tertiary", "on_tertiary", "tertiary_container", "on_tertiary_container",
        "error", "on_error", "error_container", "on_error_container",
        "background", "on_background", "surface", "on_surface", "surface_variant", "on_surface_variant",
        "outline", "outline_variant", "surface_dim", "surface_bright", "surface_container_lowest",
        "surface_container_low", "surface_container", "surface_container_high", "surface_container_highest",
    };
    private static final String[] FIXED = {
        "primary_fixed", "primary_fixed_dim", "on_primary_fixed", "on_primary_fixed_variant",
        "secondary_fixed", "secondary_fixed_dim", "on_secondary_fixed", "on_secondary_fixed_variant",
        "tertiary_fixed", "tertiary_fixed_dim", "on_tertiary_fixed", "on_tertiary_fixed_variant",
    };
    // Android 12 and 13: the roles from the accent and neutral palettes (tone names: 0 is
    // white, 1000 black), light then dark, as Material's baseline scheme takes them
    private static final Object[][] TONES = {
        {"primary", "accent1", 600, 200}, {"on_primary", "accent1", 0, 800},
        {"primary_container", "accent1", 100, 700}, {"on_primary_container", "accent1", 900, 100},
        {"secondary", "accent2", 600, 200}, {"on_secondary", "accent2", 0, 800},
        {"secondary_container", "accent2", 100, 700}, {"on_secondary_container", "accent2", 900, 100},
        {"tertiary", "accent3", 600, 200}, {"on_tertiary", "accent3", 0, 800},
        {"tertiary_container", "accent3", 100, 700}, {"on_tertiary_container", "accent3", 900, 100},
        {"background", "neutral1", 10, 900}, {"on_background", "neutral1", 900, 100},
        {"surface", "neutral1", 10, 900}, {"on_surface", "neutral1", 900, 100},
        {"surface_variant", "neutral2", 100, 700}, {"on_surface_variant", "neutral2", 700, 200},
        {"outline", "neutral2", 500, 400}, {"outline_variant", "neutral2", 200, 700},
        {"surface_dim", "neutral1", 100, 900}, {"surface_bright", "neutral1", 10, 800},
        {"surface_container_lowest", "neutral1", 0, 1000}, {"surface_container_low", "neutral1", 50, 900},
        {"surface_container", "neutral1", 50, 900}, {"surface_container_high", "neutral1", 100, 800},
        {"surface_container_highest", "neutral1", 100, 800},
    };

    // "system_<role>_<light|dark>=aarrggbb" per line; empty before Android 12
    public String colors(String unused) {
        if (Build.VERSION.SDK_INT < 31) return "";
        StringBuilder s = new StringBuilder();
        Resources r = getResources();
        if (Build.VERSION.SDK_INT >= 34) {
            for (String role : ROLES) {
                put(s, r, "system_" + role + "_light", "system_" + role + "_light");
                put(s, r, "system_" + role + "_dark", "system_" + role + "_dark");
            }
            for (String role : FIXED) put(s, r, "system_" + role, "system_" + role);
        } else {
            for (Object[] t : TONES) {
                put(s, r, "system_" + t[0] + "_light", "system_" + t[1] + "_" + t[2]);
                put(s, r, "system_" + t[0] + "_dark", "system_" + t[1] + "_" + t[3]);
            }
        }
        return s.toString();
    }

    private void put(StringBuilder s, Resources r, String name, String resource) {
        int id = r.getIdentifier(resource, "color", "android");
        if (id == 0) return;
        try {
            s.append(name).append('=').append(Integer.toHexString(r.getColor(id, getTheme()))).append('\n');
        } catch (Exception e) {
            // (a colour this Android does not have)
        }
    }
}
