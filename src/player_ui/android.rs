//! Initialize Android's filesystem and Rodio's Java context before any workers.

use jni::objects::{JByteArray, JObject, JString, JValue};
use macroquad::miniquad::native::android;

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_lambdadx_lambda_1dx_1player_1ui_MainActivity_playerBackgrounded(
    _env: jni::JNIEnv<'_>, _activity: JObject<'_>,
) {
    crate::app::lifecycle::global().suspend();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_lambdadx_lambda_1dx_1player_1ui_MainActivity_playerForegrounded(
    _env: jni::JNIEnv<'_>, _activity: JObject<'_>,
) {
    crate::app::lifecycle::global().restore();
}

pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    // miniquad owns a global Activity reference and has attached this thread
    // before calling quad_main. No render/audio worker exists at this point.
    let mut env = unsafe { jni::JNIEnv::from_raw(android::attach_jni_env().cast())? };
    let activity = unsafe { JObject::from_raw(android::ACTIVITY.cast()) };
    let vm = env.get_java_vm()?;
    unsafe {
        ndk_context::initialize_android_context(
            vm.get_java_vm_pointer().cast(),
            activity.as_raw().cast(),
        );
    }
    let path = env
        .call_method(
            &activity,
            "preparePlayerAssets",
            "()Ljava/lang/String;",
            &[],
        )?
        .l()?;
    let path: String = env.get_string(&JString::from(path))?.into();
    let assets = std::path::PathBuf::from(path);
    let root = assets
        .parent()
        .ok_or("Android assets have no data directory")?;
    let external = with_activity(|env, activity| {
        let file = env
            .call_method(
                activity,
                "getExternalFilesDir",
                "(Ljava/lang/String;)Ljava/io/File;",
                &[JValue::Object(&JObject::null())],
            )
            .and_then(|value| value.l())
            .map_err(|error| error.to_string())?;
        if file.is_null() {
            return Ok(None);
        }
        let path = env
            .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l())
            .map_err(|error| error.to_string())?;
        let path: String = env
            .get_string(&JString::from(path))
            .map_err(|error| error.to_string())?
            .into();
        Ok(Some(std::path::PathBuf::from(path)))
    });
    match external {
        Ok(Some(dir)) => {
            // Create an import location without changing the extracted assets.
            if let Err(error) = std::fs::create_dir_all(dir.join("chart")) {
                eprintln!("[android] cannot create external chart directory: {error}");
            }
            unsafe {
                std::env::set_var("MAI2_EXTERNAL_DATA_DIR", &dir);
            }
        }
        Ok(None) => {}
        Err(error) => eprintln!("[android] external files directory unavailable: {error}"),
    }
    // These are application-specific variables, set before starting threads.
    unsafe {
        std::env::set_var("MAI2_ASSET_DIR", &assets);
        std::env::set_var("MAI2_DATA_DIR", root);
        std::env::set_var("MAI2_CACHE_DIR", root.join("cache"));
    }
    std::env::set_current_dir(root)?;
    Ok(())
}

pub enum LinkInput {
    Text(String),
    Frame(usize, usize, Vec<u8>),
}

fn with_activity<T>(
    f: impl FnOnce(&mut jni::JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let mut env = unsafe {
        jni::JNIEnv::from_raw(android::attach_jni_env().cast()).map_err(|e| e.to_string())?
    };
    let activity = unsafe { JObject::from_raw(android::ACTIVITY.cast()) };
    // Polling can run for hours; all temporary JNI refs must be reclaimed.
    env.push_local_frame(16).map_err(|e| e.to_string())?;
    let result = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    unsafe {
        let _ = env.pop_local_frame(&JObject::null());
    }
    result
}
fn link_call(method: &str) -> Result<(), String> {
    with_activity(|env, activity| {
        env.call_method(activity, method, "()V", &[])
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}
pub fn open_link_scanner() -> Result<(), String> {
    link_call("openLinkScanner")
}
pub fn close_link_scanner() -> Result<(), String> {
    link_call("closeLinkScanner")
}
pub fn open_link_manual() -> Result<(), String> {
    link_call("openLinkManual")
}
pub fn poll_link_input() -> Result<Option<LinkInput>, String> {
    with_activity(|env, activity| {
        let text = env
            .call_method(activity, "pollLinkText", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        if !text.is_null() {
            let value: String = env
                .get_string(&JString::from(text))
                .map_err(|e| e.to_string())?
                .into();
            return Ok(Some(LinkInput::Text(value)));
        }
        let frame = env
            .call_method(activity, "pollLinkFrame", "()[B", &[])
            .and_then(|v| v.l())
            .map_err(|e| e.to_string())?;
        if frame.is_null() {
            return Ok(None);
        }
        let bytes = env
            .convert_byte_array(JByteArray::from(frame))
            .map_err(|e| e.to_string())?;
        if bytes.len() < 8 {
            return Err("扫码图像头无效".into());
        }
        let w = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        let h = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if w > 4096 || h > 4096 || bytes.len() != 8 + w * h {
            return Err("扫码图像大小无效".into());
        }
        Ok(Some(LinkInput::Frame(w, h, bytes[8..].to_vec())))
    })
}
