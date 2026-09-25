// Self-update from GitHub releases.
//
// `check` asks GitHub for the latest release and returns it if it's newer
// than this build and is signed for this platform. It uses github.com's web
// URLs, not the REST API, which only allows 60 unauthenticated requests an
// hour per IP address - a whole office can share one. `install` downloads
// the asset, verifies its minisign signature against the
// public key in keys/release-signing.pub - including the signed trusted
// comment, which must name this asset and the release's version, so an older
// signed build can't be passed off as a newer one - and then swaps it in:
// - Windows (and Linux, which is only used for testing): the running
//   executable is replaced in place (self_replace).
// - macOS: the whole .app bundle is replaced, since the macOS release asset
//   is the zipped Test Your Might.app.
// `relaunch` then starts the new version. The in-game prompt lives in
// update_prompt.rs; `run_cli` is the `--update` flag.
//
// Test hooks (environment variables, see docs/dev-environment.md):
//   TYM_UPDATE_URL=<url>       a stand-in for the GitHub repo URL, serving
//                              <url>/releases/latest (a redirect to
//                              .../releases/tag/<tag>) and
//                              <url>/releases/download/<tag>/<file>, like
//                              tools/fake-release-server.py (also turns on
//                              the startup check in debug builds)
//   TYM_UPDATE_PUBKEY=<base64> trust this minisign public key instead
//   TYM_UPDATE_ASSET=<name>    asset to look for (e.g. to try a Linux build,
//                              which isn't released)

use std::env;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const PUBLIC_KEY_FILE: &str = include_str!("../keys/release-signing.pub");
// Far above any real release (~5 MB), so a bad server can't fill the disk.
const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;
const USER_AGENT: &str = concat!("test-your-might/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Version(u64, u64, u64);

impl Version {
    // "1.2.3" or "v1.2.3".
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.strip_prefix('v').unwrap_or(s).split('.').map(|p| p.parse::<u64>().ok());
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }

    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("Cargo.toml version is x.y.z")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    asset: String,
    asset_url: String,
    // The asset's minisign signature, fetched by `check`.
    signature: String,
}

// Where to restart from once an update is installed.
#[derive(Clone, Debug)]
pub enum Installed {
    Executable(PathBuf),
    MacBundle(PathBuf),
}

// Whether to check for updates at startup: release builds only (a debug
// build is someone working on the game), unless a test server is set.
pub fn startup_check_enabled() -> bool {
    !cfg!(debug_assertions) || env::var_os("TYM_UPDATE_URL").is_some()
}

// The release asset this build updates from, if there is one.
fn asset_name() -> Option<String> {
    if let Ok(name) = env::var("TYM_UPDATE_ASSET") {
        return Some(name);
    }
    if cfg!(windows) {
        Some("test-your-might.exe".into())
    } else if cfg!(target_os = "macos") {
        Some("test-your-might-macos.zip".into())
    } else {
        None
    }
}

// The trusted comment the release workflow signs each asset with.
pub fn trusted_comment(asset: &str, version: Version) -> String {
    format!("test-your-might {asset} {version}")
}

fn agent(redirects: u32) -> Result<ureq::Agent, String> {
    let builder = ureq::AgentBuilder::new()
        .redirects(redirects)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(USER_AGENT);
    // The OS's TLS on Windows and macOS (see Cargo.toml).
    #[cfg(any(windows, target_os = "macos"))]
    let builder = builder.tls_connector(std::sync::Arc::new(
        ureq::native_tls::TlsConnector::new().map_err(|e| format!("Couldn't set up HTTPS ({e})."))?,
    ));
    Ok(builder.build())
}

// The latest release, if it's newer than this build and signed for this
// platform. Fetching the (small) signature here means only releases that can
// actually be installed are offered.
pub fn check() -> Result<Option<Release>, String> {
    let Some(asset) = asset_name() else {
        return Ok(None);
    };
    let repo = env::var("TYM_UPDATE_URL").unwrap_or_else(|_| crate::REPO_URL.to_owned());
    let repo = repo.trim_end_matches('/');
    // <repo>/releases/latest redirects to <repo>/releases/tag/<latest tag>.
    let latest = agent(0)?
        .get(&format!("{repo}/releases/latest"))
        .call()
        .map_err(|e| format!("Couldn't check for updates ({e})."))?;
    let tag = latest
        .header("Location")
        .and_then(|location| location.rsplit_once("/releases/tag/"))
        .map(|(_, tag)| tag.to_owned())
        .ok_or_else(|| format!("Couldn't find the latest release (HTTP {}).", latest.status()))?;
    let version =
        Version::parse(&tag).ok_or_else(|| format!("The latest release has an unexpected tag {tag:?}."))?;
    if version <= Version::current() {
        return Ok(None);
    }
    let asset_url = format!("{repo}/releases/download/{tag}/{asset}");
    let signature = match agent(5)?.get(&format!("{asset_url}.minisig")).call() {
        Ok(response) => response
            .into_string()
            .map_err(|e| format!("Couldn't download the update's signature ({e})."))?,
        Err(ureq::Error::Status(404, _)) => {
            return Err(format!("Release {version} has no signature for {asset}."));
        }
        Err(e) => return Err(format!("Couldn't download the update's signature ({e}).")),
    };
    Ok(Some(Release { version, asset, asset_url, signature }))
}

fn download(url: &str, progress: &dyn Fn(f32)) -> Result<Vec<u8>, String> {
    let response = agent(5)?.get(url).call().map_err(|e| format!("Couldn't download the update ({e})."))?;
    let total = response.header("Content-Length").and_then(|s| s.parse::<usize>().ok());
    if total.is_some_and(|t| t > MAX_DOWNLOAD_BYTES) {
        return Err("The download is unexpectedly large.".into());
    }
    let mut reader = response.into_reader().take(MAX_DOWNLOAD_BYTES as u64 + 1);
    let mut data = Vec::with_capacity(total.unwrap_or(0));
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Couldn't download the update ({e})."))?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if data.len() > MAX_DOWNLOAD_BYTES {
            return Err("The download is unexpectedly large.".into());
        }
        if let Some(total) = total {
            progress((data.len() as f32 / total as f32).min(1.0));
        }
    }
    if total.is_some_and(|t| t != data.len()) {
        return Err("The download was cut short.".into());
    }
    Ok(data)
}

fn public_key() -> Result<minisign_verify::PublicKey, String> {
    let base64 = match env::var("TYM_UPDATE_PUBKEY") {
        Ok(key) => key,
        // Second line of the minisign .pub file.
        Err(_) => PUBLIC_KEY_FILE.lines().nth(1).unwrap_or_default().trim().to_owned(),
    };
    minisign_verify::PublicKey::from_base64(&base64).map_err(|e| format!("Bad update signing key ({e})."))
}

// Checks `data` is exactly the asset the release workflow signed for this
// release's version.
fn verify(data: &[u8], signature: &str, release: &Release) -> Result<(), String> {
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|e| format!("The update's signature is unreadable ({e})."))?;
    public_key()?
        .verify(data, &signature, false)
        .map_err(|e| format!("The download failed its signature check, so nothing was changed. ({e})"))?;
    let expected = trusted_comment(&release.asset, release.version);
    if signature.trusted_comment() != expected {
        return Err(format!(
            "The download is signed for a different version, so nothing was changed. ({:?}, expected {expected:?})",
            signature.trusted_comment()
        ));
    }
    Ok(())
}

// Downloads, verifies, and installs `release` over this copy of the game.
// `progress` gets the download's progress, 0 to 1.
pub fn install(release: &Release, progress: &dyn Fn(f32)) -> Result<Installed, String> {
    // Before anything is replaced: afterwards the running executable's path
    // may point at the moved-away old file.
    let exe = env::current_exe().map_err(|e| format!("Can't find the running game ({e})."))?;
    let data = download(&release.asset_url, progress)?;
    verify(&data, &release.signature, release)?;
    match mac_bundle(&exe) {
        Some(bundle) => replace_bundle(&bundle, &data).map(|()| Installed::MacBundle(bundle)),
        None => replace_executable(&exe, &data).map(|()| Installed::Executable(exe)),
    }
}

fn replace_executable(exe: &Path, data: &[u8]) -> Result<(), String> {
    let dir = exe.parent().ok_or("Can't find the game's folder.")?;
    let name = exe.file_name().ok_or("Can't find the game's file name.")?.to_string_lossy();
    // Same folder, so it's on the same drive as the executable.
    let new = dir.join(format!(".{name}.update-{}", std::process::id()));
    let result = (|| {
        fs::write(&new, data).map_err(|e| format!("Couldn't save the update next to the game ({e})."))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&new, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Couldn't make the update executable ({e})."))?;
        }
        self_replace::self_replace(&new).map_err(|e| format!("Couldn't replace {} ({e}).", exe.display()))
    })();
    let _ = fs::remove_file(&new);
    result
}

// The .app bundle `exe` runs from (<bundle>.app/Contents/MacOS/<exe>).
fn mac_bundle(exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_path_buf())
}

// Replaces the whole bundle with the one in `zip`. The old bundle is moved
// aside first and put back if the new one can't be moved in. A running app's
// bundle can be moved and deleted on macOS; the process keeps running.
fn replace_bundle(bundle: &Path, zip: &[u8]) -> Result<(), String> {
    let parent = bundle.parent().ok_or("Can't find the app's folder.")?;
    // Next to the bundle, so the final moves stay on one volume.
    let work = parent.join(format!(".test-your-might-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir(&work).map_err(|e| format!("Couldn't save the update next to the app ({e})."))?;
    let result = (|| {
        let zip_path = work.join("update.zip");
        fs::write(&zip_path, zip).map_err(|e| format!("Couldn't save the update ({e})."))?;
        let unpacked = work.join("new");
        let status = Command::new("ditto")
            .args(["-x", "-k"])
            .arg(&zip_path)
            .arg(&unpacked)
            .status()
            .map_err(|e| format!("Couldn't unpack the update ({e})."))?;
        if !status.success() {
            return Err(format!("Couldn't unpack the update (ditto {status})."));
        }
        let new_bundle = fs::read_dir(&unpacked)
            .map_err(|e| format!("Couldn't read the update ({e})."))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "app"))
            .ok_or("The update has no app in it.")?;
        let has_program = fs::read_dir(new_bundle.join("Contents/MacOS"))
            .ok()
            .and_then(|mut d| d.next())
            .is_some();
        if !has_program {
            return Err("The update's app has no program in it.".into());
        }
        let old = work.join("old.app");
        fs::rename(bundle, &old).map_err(|e| format!("Couldn't move the old app aside ({e})."))?;
        if let Err(e) = fs::rename(&new_bundle, bundle) {
            let _ = fs::rename(&old, bundle);
            return Err(format!("Couldn't move the new app into place ({e})."));
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&work);
    result
}

// Starts the newly installed game. The caller should then quit. Only
// settings flags carry over: one-shot actions like --reset must not run
// again.
pub fn relaunch(installed: &Installed) -> Result<(), String> {
    let args = relaunch_args(env::args().skip(1));
    let spawned = match installed {
        Installed::MacBundle(bundle) => Command::new("open").arg("-n").arg(bundle).arg("--args").args(&args).spawn(),
        Installed::Executable(exe) => Command::new(exe).args(&args).spawn(),
    };
    spawned.map(|_| ()).map_err(|e| format!("The game couldn't restart itself ({e})."))
}

fn relaunch_args(args: impl Iterator<Item = String>) -> Vec<String> {
    let mut keep = Vec::new();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        if arg == "--height" {
            keep.push(arg);
            keep.extend(args.next());
        }
    }
    keep
}

// `--update`: check and install from the command line, without the game.
// Returns the process exit code.
pub fn run_cli() -> i32 {
    let current = Version::current();
    let release = match check() {
        Ok(Some(release)) => release,
        Ok(None) if asset_name().is_none() => {
            println!("Updates aren't available for this platform.");
            return 0;
        }
        Ok(None) => {
            println!("Test Your Might {current} is up to date.");
            return 0;
        }
        Err(e) => {
            eprintln!("Update failed. {e}");
            return 1;
        }
    };
    println!("Updating Test Your Might {current} to {}...", release.version);
    match install(&release, &|_| {}) {
        Ok(_) => {
            println!("Updated to {}. Start the game again to use it.", release.version);
            0
        }
        Err(e) => {
            eprintln!("Update failed. {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(Version::parse("v0.0.18"), Some(Version(0, 0, 18)));
        assert_eq!(Version::parse("1.2.3"), Some(Version(1, 2, 3)));
        assert_eq!(Version::parse("v1.2"), None);
        assert_eq!(Version::parse("v1.2.3.4"), None);
        assert_eq!(Version::parse("v1.2.x"), None);
        assert_eq!(Version::parse("v1.2.3-beta"), None);
    }

    #[test]
    fn orders_versions_numerically() {
        assert!(Version::parse("v0.0.10") > Version::parse("v0.0.9"));
        assert!(Version::parse("v0.1.0") > Version::parse("v0.0.99"));
        assert!(Version::parse("v1.0.0") > Version::parse("v0.99.99"));
    }

    #[test]
    fn relaunch_keeps_only_settings() {
        let args = ["--reset", "--height", "0.6", "--update", "--version"].map(String::from);
        assert_eq!(relaunch_args(args.into_iter()), ["--height", "0.6"]);
    }

    #[test]
    fn built_in_public_key_parses() {
        let base64 = PUBLIC_KEY_FILE.lines().nth(1).unwrap().trim();
        assert!(minisign_verify::PublicKey::from_base64(base64).is_ok());
    }
}
