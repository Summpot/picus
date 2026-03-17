#[cfg(target_os = "macos")]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    time::Duration,
};

#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, sel};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSWorkspace;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSBundle, NSError, NSObject, NSObjectProtocol, NSString, NSURL};
#[cfg(target_os = "macos")]
use plist::{Dictionary, Value};

#[cfg(target_os = "macos")]
use crate::{
    ActivationError, MacosAppBundle, MacosBundleConfig, MacosInfoPlist,
    ResolvedProtocolRegistration, Result,
};

#[cfg(target_os = "macos")]
const DEFAULT_HANDLER_COMPLETION_WAIT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
const K_INTERNET_EVENT_CLASS: u32 = four_cc(*b"GURL");
#[cfg(target_os = "macos")]
const K_AE_GET_URL: u32 = four_cc(*b"GURL");
#[cfg(target_os = "macos")]
const KEY_DIRECT_OBJECT: u32 = four_cc(*b"----");

#[cfg(target_os = "macos")]
const fn four_cc(code: [u8; 4]) -> u32 {
    u32::from_be_bytes(code)
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct ActivationUrlEventHandlerIvars {
    sender: std::sync::mpsc::Sender<String>,
}

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    #[ivars = ActivationUrlEventHandlerIvars]
    struct ActivationUrlEventHandler;

    unsafe impl NSObjectProtocol for ActivationUrlEventHandler {}

    impl ActivationUrlEventHandler {
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_get_url_event(&self, event: &AnyObject, _reply_event: &AnyObject) {
            if let Some(uri) = extract_url_string_from_apple_event(event) {
                let _ = self.ivars().sender.send(uri);
            }
        }
    }
);

#[cfg(target_os = "macos")]
impl ActivationUrlEventHandler {
    fn new(sender: std::sync::mpsc::Sender<String>, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ActivationUrlEventHandlerIvars { sender });
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub(crate) struct MacosActivationListener {
    _handler: Retained<ActivationUrlEventHandler>,
}

#[cfg(target_os = "macos")]
impl Drop for MacosActivationListener {
    fn drop(&mut self) {
        unsafe {
            let manager: *mut AnyObject =
                msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
            if !manager.is_null() {
                let (): () = msg_send![
                    manager,
                    removeEventHandlerForEventClass: K_INTERNET_EVENT_CLASS,
                    andEventID: K_AE_GET_URL
                ];
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn install_url_event_listener(
    sender: std::sync::mpsc::Sender<String>,
) -> Result<MacosActivationListener> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        ActivationError::Platform(
            "macOS activation Apple Event listener must be installed on the main thread"
                .to_string(),
        )
    })?;
    let handler = ActivationUrlEventHandler::new(sender, mtm);

    unsafe {
        let manager: *mut AnyObject =
            msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        if manager.is_null() {
            return Err(ActivationError::Platform(
                "NSAppleEventManager sharedAppleEventManager returned nil".to_string(),
            ));
        }

        let (): () = msg_send![
            manager,
            setEventHandler: &*handler,
            andSelector: sel!(handleGetURLEvent:withReplyEvent:),
            forEventClass: K_INTERNET_EVENT_CLASS,
            andEventID: K_AE_GET_URL
        ];
    }

    Ok(MacosActivationListener { _handler: handler })
}

#[cfg(target_os = "macos")]
fn extract_url_string_from_apple_event(event: &AnyObject) -> Option<String> {
    unsafe {
        let descriptor: *mut AnyObject =
            msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT];
        if descriptor.is_null() {
            return None;
        }

        let string: *mut NSString = msg_send![descriptor, stringValue];
        if string.is_null() {
            return None;
        }

        Some((&*string).to_string())
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn register(protocol: &ResolvedProtocolRegistration) -> Result<()> {
    let bundle = match find_main_app_bundle()?
        .or(find_current_app_bundle(protocol.executable.as_path())?)
    {
        Some(bundle) => bundle,
        None => {
            let Some(config) = protocol.macos_bundle.as_ref() else {
                return Err(ActivationError::InvalidConfig(
                    "macOS custom URL activation requires either running from an .app bundle or supplying ProtocolRegistration::with_macos_bundle(...)"
                        .to_string(),
                ));
            };
            create_app_bundle_from_plist(protocol.executable.as_path(), config)?
        }
    };

    if !bundle
        .info_plist
        .url_schemes
        .iter()
        .any(|scheme| scheme.eq_ignore_ascii_case(protocol.scheme.as_str()))
    {
        return Err(ActivationError::InvalidConfig(format!(
            "Info.plist for bundle `{}` does not register scheme `{}`",
            bundle.info_plist.bundle_identifier, protocol.scheme
        )));
    }

    register_bundle_with_launch_services(bundle.bundle_path.as_path())?;
    set_default_handler(protocol.scheme.as_str(), bundle.bundle_path.as_path())?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn read_info_plist(info_plist_path: impl AsRef<Path>) -> Result<MacosInfoPlist> {
    let info_plist_path = info_plist_path.as_ref();
    let value = Value::from_file(info_plist_path)?;
    let dictionary = value.as_dictionary().ok_or_else(|| {
        ActivationError::InvalidConfig(format!(
            "Info.plist at {:?} must contain a top-level dictionary",
            info_plist_path
        ))
    })?;

    let bundle_identifier = required_string(dictionary, "CFBundleIdentifier", info_plist_path)?;
    let executable_name = required_string(dictionary, "CFBundleExecutable", info_plist_path)?;
    let bundle_name = optional_string(dictionary, "CFBundleName")
        .or_else(|| optional_string(dictionary, "CFBundleDisplayName"))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| bundle_identifier.clone());
    let url_schemes = dedupe_preserve_order(parse_url_schemes(dictionary));

    Ok(MacosInfoPlist {
        bundle_identifier,
        bundle_name,
        executable_name,
        url_schemes,
    })
}

#[cfg(target_os = "macos")]
pub fn create_app_bundle_from_plist(
    executable: impl AsRef<Path>,
    config: &MacosBundleConfig,
) -> Result<MacosAppBundle> {
    let executable = executable.as_ref();
    if !executable.exists() {
        return Err(ActivationError::InvalidConfig(format!(
            "executable does not exist: {:?}",
            executable
        )));
    }

    let info_plist = read_info_plist(&config.info_plist)?;
    let bundle_name = config
        .bundle_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| info_plist.bundle_name.clone());

    if bundle_name.trim().is_empty() {
        return Err(ActivationError::InvalidConfig(
            "macOS bundle name cannot be empty".to_string(),
        ));
    }

    let applications_dir = match config.applications_dir.clone() {
        Some(path) => path,
        None => default_applications_dir()?,
    };
    fs::create_dir_all(&applications_dir).map_err(|error| {
        ActivationError::Platform(format!(
            "failed to create Applications directory {:?}: {error}",
            applications_dir
        ))
    })?;

    let bundle_path = applications_dir.join(format!("{bundle_name}.app"));
    if bundle_path.exists() && !bundle_path.is_dir() {
        remove_path_if_exists(&bundle_path)?;
    }
    let contents_dir = bundle_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    fs::create_dir_all(&macos_dir).map_err(|error| {
        ActivationError::Platform(format!(
            "failed to create app bundle directories {:?}: {error}",
            macos_dir
        ))
    })?;

    let info_plist_path = contents_dir.join("Info.plist");
    fs::copy(&config.info_plist, &info_plist_path).map_err(|error| {
        ActivationError::Platform(format!(
            "failed to copy Info.plist from {:?} to {:?}: {error}",
            config.info_plist, info_plist_path
        ))
    })?;
    write_pkg_info(&contents_dir)?;

    let executable_path = macos_dir.join(info_plist.executable_name.as_str());
    remove_path_if_exists(&executable_path)?;
    create_symlink_or_copy(executable, &executable_path)?;

    Ok(MacosAppBundle {
        bundle_path,
        info_plist_path,
        executable_path,
        info_plist,
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn find_current_app_bundle(executable: &Path) -> Result<Option<MacosAppBundle>> {
    let Some(macos_dir) = executable.parent() else {
        return Ok(None);
    };
    if macos_dir.file_name().and_then(|value| value.to_str()) != Some("MacOS") {
        return Ok(None);
    }

    let Some(contents_dir) = macos_dir.parent() else {
        return Ok(None);
    };
    if contents_dir.file_name().and_then(|value| value.to_str()) != Some("Contents") {
        return Ok(None);
    }

    let Some(bundle_path) = contents_dir.parent() else {
        return Ok(None);
    };
    if bundle_path.extension().and_then(|value| value.to_str()) != Some("app") {
        return Ok(None);
    }

    app_bundle_from_bundle_path(bundle_path, Some(executable.to_path_buf()))
}

#[cfg(target_os = "macos")]
fn find_main_app_bundle() -> Result<Option<MacosAppBundle>> {
    let bundle = NSBundle::mainBundle();
    let bundle_path = PathBuf::from(bundle.bundlePath().to_string());
    app_bundle_from_bundle_path(bundle_path.as_path(), None)
}

#[cfg(target_os = "macos")]
fn app_bundle_from_bundle_path(
    bundle_path: &Path,
    executable_override: Option<PathBuf>,
) -> Result<Option<MacosAppBundle>> {
    if bundle_path.extension().and_then(|value| value.to_str()) != Some("app") {
        return Ok(None);
    }

    let contents_dir = bundle_path.join("Contents");
    let info_plist_path = contents_dir.join("Info.plist");
    if !info_plist_path.exists() {
        return Ok(None);
    }

    let info_plist = read_info_plist(&info_plist_path)?;
    let executable_path = executable_override.unwrap_or_else(|| {
        contents_dir
            .join("MacOS")
            .join(info_plist.executable_name.as_str())
    });

    if !executable_path.exists() {
        return Ok(None);
    }

    Ok(Some(MacosAppBundle {
        bundle_path: bundle_path.to_path_buf(),
        info_plist_path,
        executable_path,
        info_plist,
    }))
}

#[cfg(target_os = "macos")]
fn required_string(dict: &Dictionary, key: &str, path: &Path) -> Result<String> {
    dict.get(key)
        .and_then(Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            ActivationError::InvalidConfig(format!(
                "Info.plist {:?} is missing a non-empty `{key}` string",
                path
            ))
        })
}

#[cfg(target_os = "macos")]
fn optional_string(dict: &Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(target_os = "macos")]
fn parse_url_schemes(dict: &Dictionary) -> Vec<String> {
    dict.get("CFBundleURLTypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_dictionary)
        .flat_map(|entry| {
            entry
                .get("CFBundleURLSchemes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_string)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::with_capacity(values.len());
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(target_os = "macos")]
fn default_applications_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|error| {
        ActivationError::Platform(format!(
            "failed to get HOME for app bundle creation: {error}"
        ))
    })?;
    Ok(PathBuf::from(home).join("Applications"))
}

#[cfg(target_os = "macos")]
fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ActivationError::Platform(format!("failed to stat existing path {:?}: {error}", path))
    })?;

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path).map_err(|error| {
            ActivationError::Platform(format!("failed to remove directory {:?}: {error}", path))
        })?;
    } else {
        fs::remove_file(path).map_err(|error| {
            ActivationError::Platform(format!("failed to remove file {:?}: {error}", path))
        })?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn create_symlink_or_copy(source: &Path, dest: &Path) -> Result<()> {
    // macOS TCC (Transparency, Consent, and Control) protects folders like
    // ~/Documents, ~/Desktop, and ~/Downloads.
    // If we use a symlink, Launch Services resolves the symlink and tries to read
    // the executable from the protected folder, causing a permission prompt.
    // To avoid this, we hard-link or copy the executable instead.

    if fs::hard_link(source, dest).is_ok() {
        return Ok(());
    }

    fs::copy(source, dest).map_err(|error| {
        ActivationError::Platform(format!(
            "failed to copy executable from {:?} to {:?}: {error}",
            source, dest
        ))
    })?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn write_pkg_info(contents_dir: &Path) -> Result<()> {
    let pkg_info_path = contents_dir.join("PkgInfo");
    fs::write(&pkg_info_path, b"APPL????").map_err(|error| {
        ActivationError::Platform(format!(
            "failed to write PkgInfo at {:?}: {error}",
            pkg_info_path
        ))
    })
}

#[cfg(target_os = "macos")]
fn register_bundle_with_launch_services(bundle_path: &Path) -> Result<()> {
    let output = Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
        .arg("-f")
        .arg(bundle_path)
        .output()
        .map_err(|error| {
            ActivationError::Platform(format!("failed to run lsregister: {error}"))
        })?;

    if !output.status.success() {
        return Err(ActivationError::Platform(format!(
            "lsregister failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn bundle_url(bundle_path: &Path) -> Result<Retained<NSURL>> {
    NSURL::from_directory_path(bundle_path).ok_or_else(|| {
        ActivationError::InvalidConfig(format!(
            "bundle path cannot be converted to NSURL: {:?}",
            bundle_path
        ))
    })
}

#[cfg(target_os = "macos")]
fn format_ns_error(error_ptr: *mut NSError) -> String {
    if error_ptr.is_null() {
        return "unknown NSWorkspace error".to_string();
    }

    let error = unsafe { &*error_ptr };
    let domain = error.domain().to_string();
    let code = error.code();
    let description = error.localizedDescription().to_string();
    format!("{description} (domain: {domain}, code: {code})")
}

#[cfg(target_os = "macos")]
fn finish_default_handler_request(
    receiver: &mpsc::Receiver<std::result::Result<(), String>>,
    timeout: Duration,
) -> Result<()> {
    match receiver.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(ActivationError::Platform(error)),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(ActivationError::Platform(
            "NSWorkspace default-handler request completion channel disconnected unexpectedly"
                .to_string(),
        )),
    }
}

#[cfg(target_os = "macos")]
fn set_default_handler(scheme: &str, bundle_path: &Path) -> Result<()> {
    let application_url = bundle_url(bundle_path)?;
    let url_scheme = NSString::from_str(scheme);
    let workspace = NSWorkspace::sharedWorkspace();
    let (completion_sender, completion_receiver) =
        mpsc::channel::<std::result::Result<(), String>>();
    let completion_handler = RcBlock::new(move |error: *mut NSError| {
        let result = if error.is_null() {
            Ok(())
        } else {
            Err(format_ns_error(error))
        };
        let _ = completion_sender.send(result);
    });

    workspace.setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
        &application_url,
        &url_scheme,
        Some(&*completion_handler),
    );

    finish_default_handler_request(&completion_receiver, DEFAULT_HANDLER_COMPLETION_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "bevy-xilem-activation-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    fn write_test_info_plist(path: &Path) {
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>Pixiv Desktop</string>
    <key>CFBundleExecutable</key>
    <string>example_pixiv_client</string>
    <key>CFBundleIdentifier</key>
    <string>dev.summpot.example-pixiv-client</string>
    <key>CFBundleName</key>
    <string>Pixiv Desktop</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>pixiv</string>
                <string>pixiv</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
"#;
        fs::write(path, plist).expect("Info.plist should be written");
    }

    #[test]
    fn read_info_plist_extracts_bundle_metadata() {
        let dir = temp_test_dir("plist-read");
        let plist_path = dir.join("Info.plist");
        write_test_info_plist(&plist_path);

        let info = read_info_plist(&plist_path).expect("Info.plist should parse");
        assert_eq!(info.bundle_identifier, "dev.summpot.example-pixiv-client");
        assert_eq!(info.bundle_name, "Pixiv Desktop");
        assert_eq!(info.executable_name, "example_pixiv_client");
        assert_eq!(info.url_schemes, vec!["pixiv".to_string()]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn create_app_bundle_from_plist_copies_plist_and_links_executable() {
        let dir = temp_test_dir("bundle-create");
        let executable = dir.join("example_pixiv_client-bin");
        let plist_path = dir.join("Info.plist");
        let apps_dir = dir.join("Applications");

        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("executable should be written");
        write_test_info_plist(&plist_path);

        let bundle = create_app_bundle_from_plist(
            &executable,
            &MacosBundleConfig::new(&plist_path).with_applications_dir(&apps_dir),
        )
        .expect("bundle creation should succeed");

        assert_eq!(
            bundle.info_plist.bundle_identifier,
            "dev.summpot.example-pixiv-client"
        );
        assert!(bundle.bundle_path.exists());
        assert!(bundle.info_plist_path.exists());
        assert!(bundle.executable_path.exists());
        assert!(bundle.bundle_path.join("Contents/PkgInfo").exists());
        assert_eq!(
            bundle
                .bundle_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("Pixiv Desktop.app")
        );

        let current = find_current_app_bundle(&bundle.executable_path)
            .expect("bundle lookup should succeed")
            .expect("bundle lookup should find the created app bundle");
        assert_eq!(
            current.info_plist.bundle_identifier,
            bundle.info_plist.bundle_identifier
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_handler_request_propagates_completion_error() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Err("permission denied".to_string()))
            .expect("test completion send should succeed");

        let error = finish_default_handler_request(&receiver, Duration::from_millis(10))
            .expect_err("completion error should surface as platform error");

        assert!(
            matches!(error, ActivationError::Platform(message) if message.contains("permission denied"))
        );
    }

    #[test]
    fn default_handler_request_timeout_is_treated_as_pending_success() {
        let (_sender, receiver) = mpsc::channel();

        finish_default_handler_request(&receiver, Duration::from_millis(0))
            .expect("pending NSWorkspace request should not be treated as a hard failure");
    }
}
