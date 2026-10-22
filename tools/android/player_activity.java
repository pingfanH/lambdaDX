//% MAIN_ACTIVITY_BODY

private native void playerBackgrounded();
private native void playerForegrounded();

// Called by the Rust entry point before rendering or audio threads start.
public String preparePlayerAssets() throws java.io.IOException {
    java.io.File root = new java.io.File(getFilesDir(), "player-assets");
    String stamp;
    try (java.io.InputStream input = getAssets().open("android-assets.version")) {
        byte[] bytes = new byte[128];
        int count = input.read(bytes);
        stamp = new String(bytes, 0, count, java.nio.charset.StandardCharsets.UTF_8);
    }
    java.io.File marker = new java.io.File(root, "android-assets.version");
    String installed = marker.isFile()
        ? new String(java.nio.file.Files.readAllBytes(marker.toPath()), java.nio.charset.StandardCharsets.UTF_8)
        : "";
    if (!stamp.equals(installed)) {
        copyPlayerAssets("", root);
        java.nio.file.Files.write(marker.toPath(), stamp.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }
    return root.getAbsolutePath();
}

private void copyPlayerAssets(String path, java.io.File destination) throws java.io.IOException {
    String[] entries = getAssets().list(path);
    if (entries != null && entries.length > 0) {
        if (!destination.isDirectory() && !destination.mkdirs()) {
            throw new java.io.IOException("Cannot create " + destination);
        }
        for (String name : entries) {
            if (path.isEmpty() && name.equals("android-assets.version")) continue;
            copyPlayerAssets(path.isEmpty() ? name : path + "/" + name, new java.io.File(destination, name));
        }
    } else {
        try (java.io.InputStream input = getAssets().open(path);
             java.io.OutputStream output = new java.io.BufferedOutputStream(new java.io.FileOutputStream(destination))) {
            byte[] buffer = new byte[65536];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }
}

private final Object linkLock = new Object();
private String linkText;
private byte[] linkFrame;
private android.app.Dialog linkScanner;
private android.hardware.Camera linkCamera;
private long linkLastFrame;

public String pollLinkText() {
    synchronized (linkLock) { String value = linkText; linkText = null; return value; }
}
public byte[] pollLinkFrame() {
    synchronized (linkLock) { byte[] value = linkFrame; linkFrame = null; return value; }
}
public void openLinkManual() {
    runOnUiThread(new Runnable() { public void run() {
        closeLinkScannerUi();
        final android.widget.EditText input = new android.widget.EditText(MainActivity.this);
        input.setSingleLine(true);
        input.setHint("ldx-preview://…");
        input.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        new android.app.AlertDialog.Builder(MainActivity.this)
            .setTitle("粘贴 Editor 连接码").setView(input)
            .setPositiveButton("连接", new android.content.DialogInterface.OnClickListener() {
                public void onClick(android.content.DialogInterface dialog, int which) {
                    synchronized (linkLock) { linkText = input.getText().toString().trim(); }
                }
            }).setNegativeButton("取消", null).show();
    }});
}
public void openLinkScanner() {
    runOnUiThread(new Runnable() { public void run() {
        if (checkSelfPermission(android.Manifest.permission.CAMERA) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] { android.Manifest.permission.CAMERA }, 4765);
            return;
        }
        showLinkScanner();
    }});
}
@Override public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
    super.onRequestPermissionsResult(requestCode, permissions, results);
    if (requestCode == 4765) {
        if (results.length > 0 && results[0] == android.content.pm.PackageManager.PERMISSION_GRANTED) showLinkScanner();
        else android.widget.Toast.makeText(this, "未授予相机权限，可改用手动连接码", android.widget.Toast.LENGTH_LONG).show();
    }
}
private void showLinkScanner() {
    closeLinkScannerUi();
    synchronized (linkLock) { linkFrame = null; linkText = null; }
    final android.view.SurfaceView surface = new android.view.SurfaceView(this);
    final android.app.AlertDialog dialog = new android.app.AlertDialog.Builder(this)
        .setTitle("扫描 Editor 的连接二维码").setView(surface)
        .setNegativeButton("取消", null).create();
    linkScanner = dialog;
    dialog.setOnDismissListener(new android.content.DialogInterface.OnDismissListener() {
        public void onDismiss(android.content.DialogInterface ignored) { releaseLinkCamera(); linkScanner = null; }
    });
    surface.getHolder().addCallback(new android.view.SurfaceHolder.Callback() {
        public void surfaceCreated(android.view.SurfaceHolder holder) { startLinkCamera(holder); }
        public void surfaceChanged(android.view.SurfaceHolder holder, int format, int width, int height) {}
        public void surfaceDestroyed(android.view.SurfaceHolder holder) { releaseLinkCamera(); }
    });
    dialog.show();
    dialog.getWindow().setLayout((int)(getResources().getDisplayMetrics().widthPixels * 0.85),
        (int)(getResources().getDisplayMetrics().heightPixels * 0.9));
}
@SuppressWarnings("deprecation")
private void startLinkCamera(android.view.SurfaceHolder holder) {
    try {
        linkCamera = android.hardware.Camera.open();
        android.hardware.Camera.Parameters params = linkCamera.getParameters();
        android.hardware.Camera.Size chosen = null;
        for (android.hardware.Camera.Size size : params.getSupportedPreviewSizes()) {
            if (size.width <= 1280 && size.height <= 960 &&
                (chosen == null || Math.abs(size.width * size.height - 640 * 480) < Math.abs(chosen.width * chosen.height - 640 * 480))) chosen = size;
        }
        if (chosen == null) throw new java.io.IOException("没有可用的扫码预览尺寸");
        params.setPreviewSize(chosen.width, chosen.height);
        params.setPreviewFormat(android.graphics.ImageFormat.NV21);
        java.util.List<String> focus = params.getSupportedFocusModes();
        if (focus != null && focus.contains(android.hardware.Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE))
            params.setFocusMode(android.hardware.Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE);
        linkCamera.setParameters(params);
        final int width = chosen.width, height = chosen.height;
        linkCamera.setPreviewDisplay(holder);
        linkLastFrame = 0;
        linkCamera.setPreviewCallbackWithBuffer(new android.hardware.Camera.PreviewCallback() {
            public void onPreviewFrame(byte[] data, android.hardware.Camera camera) {
                long now = android.os.SystemClock.elapsedRealtime();
                if (data != null && data.length >= width * height && now - linkLastFrame >= 300) {
                    linkLastFrame = now;
                    synchronized (linkLock) {
                        if (linkFrame == null) {
                            linkFrame = new byte[8 + width * height];
                            java.nio.ByteBuffer.wrap(linkFrame).putInt(width).putInt(height).put(data, 0, width * height);
                        }
                    }
                }
                if (data != null && linkCamera == camera) camera.addCallbackBuffer(data);
            }
        });
        linkCamera.addCallbackBuffer(new byte[width * height * 3 / 2]);
        linkCamera.startPreview();
    } catch (Exception error) {
        closeLinkScannerUi();
        android.widget.Toast.makeText(this, "相机不可用: " + error.getMessage(), android.widget.Toast.LENGTH_LONG).show();
    }
}
@SuppressWarnings("deprecation")
private void releaseLinkCamera() {
    if (linkCamera != null) {
        try { linkCamera.setPreviewCallbackWithBuffer(null); linkCamera.stopPreview(); } catch (Exception ignored) {}
        linkCamera.release(); linkCamera = null;
    }
    synchronized (linkLock) { linkFrame = null; }
}
private void closeLinkScannerUi() {
    if (linkScanner != null) { android.app.Dialog dialog = linkScanner; linkScanner = null; dialog.dismiss(); }
    releaseLinkCamera();
}
public void closeLinkScanner() {
    runOnUiThread(new Runnable() { public void run() { closeLinkScannerUi(); }});
}

//% END

//% MAIN_ACTIVITY_ON_CREATE
getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
//% END

//% MAIN_ACTIVITY_ON_PAUSE
playerBackgrounded();
closeLinkScannerUi();
//% END

//% MAIN_ACTIVITY_ON_RESUME
playerForegrounded();
//% END
