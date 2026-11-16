use log::info;
use volta_core::error::{ExitCode, Fallible};
use volta_core::layout::volta_home;
use volta_core::session::{ActivityKind, Session};
use volta_core::shim::regenerate_shims_for_dir;
use volta_core::style::success_prefix;

use crate::command::Command;

#[derive(clap::Args)]
pub(crate) struct Setup {}

impl Command for Setup {
    fn run(self, session: &mut Session) -> Fallible<ExitCode> {
        session.add_event_start(ActivityKind::Setup);

        os::setup_environment()?;
        regenerate_shims_for_dir(volta_home()?.shim_dir())?;

        info!(
            "{} Setup complete. Open a new terminal to start using Volta!",
            success_prefix()
        );

        session.add_event_end(ActivityKind::Setup, ExitCode::Success);
        Ok(ExitCode::Success)
    }
}

#[cfg(unix)]
mod os {
    use std::env;
    use std::fs::File;
    use std::io::{self, BufRead, BufReader, Write};
    use std::path::{Path, PathBuf};

    use log::{debug, warn};
    use volta_core::constant;
    use volta_core::error::{ErrorKind, Fallible};
    use volta_core::layout::volta_home;
    use volta_core::tool::node::HostPlatform;

    pub fn setup_environment() -> Fallible<()> {
        let home = volta_home()?;
        let formatted_home = format_home(home.root());

        // Don't update the user's shell config files if VOLTA_HOME and PATH already contain what we need.
        let home_in_path = match env::var_os("PATH") {
            Some(paths) => env::split_paths(&paths).find(|p| p == home.shim_dir()),
            None => None,
        };

        if env::var_os(constant::ENVNAME_VOLTA_HOME).is_some() && home_in_path.is_some() {
            debug!(
                "Skipping dot-file modification as VOLTA_HOME is set, and included in the PATH."
            );
            return Ok(());
        }

        debug!("Searching for profiles to update");
        let profiles = determine_profiles()?;

        let found_profile = profiles.into_iter().fold(false, |prev, profile| {
            let contents = read_profile_without_volta(&profile).unwrap_or_default();

            let write_profile = match profile.extension() {
                Some(ext) if ext == "fish" => write_profile_fish,
                _ => write_profile_sh,
            };

            match write_profile(&profile, contents, &formatted_home) {
                Ok(()) => true,
                Err(err) => {
                    warn!(
                        "Found profile script, but could not modify it: {}",
                        profile.display()
                    );
                    debug!("Profile modification error: {}", err);
                    prev
                }
            }
        });

        if found_profile {
            Ok(())
        } else {
            Err(ErrorKind::NoShellProfile {
                env_profile: String::new(),
                bin_dir: home.shim_dir().to_owned(),
            }
            .into())
        }
    }

    /// Returns a list of profile files to modify / create.
    ///
    /// Any file in the list should be created if it doesn't already exist
    fn determine_profiles() -> Fallible<Vec<PathBuf>> {
        let home_dir = dirs::home_dir().ok_or(ErrorKind::NoHomeEnvironmentVar)?;
        let shell = env::var("SHELL").unwrap_or_else(|_| String::new());
        // Always include `~/.profile`
        let mut profiles = vec![home_dir.join(".profile")];

        // PROFILE environment variable, if set
        if let Ok(profile_env) = env::var("PROFILE")
            && !profile_env.is_empty()
        {
            profiles.push(profile_env.into());
        }

        add_zsh_profile(&home_dir, &shell, &mut profiles);
        add_bash_profiles(&home_dir, &shell, &mut profiles);
        add_fish_profile(&home_dir, &shell, &mut profiles);

        Ok(profiles)
    }

    /// Add zsh profile script, if necessary
    fn add_zsh_profile(home_dir: &Path, shell: &str, profiles: &mut Vec<PathBuf>) {
        let zdotdir_env = env::var("ZDOTDIR").unwrap_or_else(|_| String::new());
        let zdotdir = if zdotdir_env.is_empty() {
            home_dir
        } else {
            Path::new(&zdotdir_env)
        };

        let zshenv = zdotdir.join(".zshenv");

        let zshrc = zdotdir.join(".zshrc");

        if shell.contains("zsh") || zshenv.exists() {
            profiles.push(zshenv);
        } else if zshrc.exists() {
            profiles.push(zshrc);
        }
    }

    /// Add bash profile scripts, if necessary
    ///
    /// Note: We only add the bash scripts if they already exist, as creating new files can impact
    /// the processing of existing files in bash (e.g. preventing ~/.profile from being loaded)
    fn add_bash_profiles(home_dir: &Path, shell: &str, profiles: &mut Vec<PathBuf>) {
        let mut bash_added = false;

        let bashrc = home_dir.join(".bashrc");
        if bashrc.exists() {
            bash_added = true;
            profiles.push(bashrc);
        }

        let bash_profile = home_dir.join(".bash_profile");
        if bash_profile.exists() {
            bash_added = true;
            profiles.push(bash_profile);
        }

        if shell.contains("bash") && !bash_added {
            warn!(
                "We detected that you are using bash, however we couldn't find any bash profile scripts.
If you run into problems running Volta, create {} and run `volta setup` again.",
                suggested_bash_profile(HostPlatform::current())
            );
        }
    }

    fn suggested_bash_profile(platform: HostPlatform) -> &'static str {
        if platform.is_macos() {
            "~/.bash_profile"
        } else {
            "~/.bashrc"
        }
    }

    /// Add fish profile scripts, if necessary
    fn add_fish_profile(home_dir: &Path, shell: &str, profiles: &mut Vec<PathBuf>) {
        let fish_config = home_dir.join(".config/fish/config.fish");

        if shell.contains("fish") || fish_config.exists() {
            profiles.push(fish_config);
        }
    }

    fn read_profile_without_volta(path: &Path) -> Option<String> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);

        reader
            .lines()
            .filter(|line_result| match line_result {
                Ok(line) if !line.contains("VOLTA") => true,
                Ok(_) => false,
                Err(_) => true,
            })
            .collect::<io::Result<Vec<String>>>()
            .map(|lines| lines.join("\n"))
            .ok()
    }

    fn format_home(volta_home: &Path) -> String {
        if let Some(home_dir) = env::var_os("HOME")
            && let Ok(suffix) = volta_home.strip_prefix(home_dir)
        {
            // If the HOME environment variable is set _and_ the proposed VOLTA_HOME starts
            // with that value, use $HOME when writing the profile scripts
            return format!("$HOME/{}", suffix.display());
        }

        volta_home.display().to_string()
    }

    fn write_profile_sh(path: &Path, contents: String, volta_home: &str) -> io::Result<()> {
        let mut file = File::create(path)?;
        write!(
            file,
            "{}\nexport VOLTA_HOME=\"{}\"\nexport PATH=\"$VOLTA_HOME/bin:$PATH\"\n",
            contents, volta_home,
        )
    }

    fn write_profile_fish(path: &Path, contents: String, volta_home: &str) -> io::Result<()> {
        let mut file = File::create(path)?;
        write!(
            file,
            "{}\nset -gx VOLTA_HOME \"{}\"\nset -gx PATH \"$VOLTA_HOME/bin\" $PATH\n",
            contents, volta_home,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn suggests_bash_profile_per_platform() {
            for platform in HostPlatform::ALL {
                assert_eq!(
                    suggested_bash_profile(platform),
                    if platform.is_macos() {
                        "~/.bash_profile"
                    } else {
                        "~/.bashrc"
                    }
                );
            }
        }
    }
}

#[cfg(windows)]
mod os {
    use log::debug;
    use volta_core::error::{Context, ErrorKind, Fallible};
    use volta_core::layout::volta_home;
    use winreg::RegKey;
    use winreg::RegValue;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, RegType};
    use winreg::types::FromRegValue;

    pub fn setup_environment() -> Fallible<()> {
        let shim_dir = volta_home()?.shim_dir().to_string_lossy().to_string();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // Read with read-only access first, so that an already-configured setup still succeeds
        // even when the Environment key is not writable.
        {
            let env = hkcu
                .open_subkey_with_flags("Environment", KEY_READ)
                .with_context(|| ErrorKind::ReadUserPathError)?;
            let (path, _) = read_path(&env)?;
            if path_entry_present(&path, &shim_dir) {
                return Ok(());
            }
        }

        let env = hkcu
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .with_context(|| ErrorKind::WriteUserPathError)?;
        ensure_shim_in_path(&env, &shim_dir)?;

        // Notify running processes (e.g. Explorer) that the environment changed. Without this,
        // terminals opened after setup can inherit a stale Path until the user logs out.
        broadcast_environment_change();

        Ok(())
    }

    /// Reads and decodes the `Path` value of `key`, returning the text and its registry type.
    fn read_path(key: &RegKey) -> Fallible<(String, RegType)> {
        let raw = key
            .get_raw_value("Path")
            .with_context(|| ErrorKind::ReadUserPathError)?;
        let path = String::from_reg_value(&raw).with_context(|| ErrorKind::ReadUserPathError)?;
        Ok((path, raw.vtype))
    }

    /// Prepends the shim directory to `key`'s `Path` value if it is not already present.
    ///
    /// Writes the registry value directly instead of spawning `setx`, which silently truncates
    /// values longer than 1024 characters and would corrupt a long user Path. The original value
    /// type (e.g. `REG_EXPAND_SZ`) is preserved so entries such as `%USERPROFILE%\...` keep
    /// expanding at runtime.
    fn ensure_shim_in_path(key: &RegKey, shim_dir: &str) -> Fallible<()> {
        let (path, vtype) = read_path(key)?;

        // Compare against whole entries rather than a substring, so that a shim dir that is
        // merely a prefix of another entry is not mistaken for an existing entry.
        if path_entry_present(&path, shim_dir) {
            return Ok(());
        }

        let new_value = prepend_shim_dir(&path, shim_dir, vtype);
        debug!(
            "Modifying User Path to: {}",
            String::from_reg_value(&new_value).unwrap_or_default()
        );
        key.set_raw_value("Path", &new_value)
            .with_context(|| ErrorKind::WriteUserPathError)?;

        Ok(())
    }

    /// Returns whether `path` already contains `shim_dir` as a whole entry, ignoring case and
    /// trailing directory separators.
    fn path_entry_present(path: &str, shim_dir: &str) -> bool {
        let shim_dir = shim_dir.trim_end_matches(['\\', '/']);
        path.split(';').any(|entry| {
            entry
                .trim()
                .trim_end_matches(['\\', '/'])
                .eq_ignore_ascii_case(shim_dir)
        })
    }

    /// Prepends `shim_dir` to `path`, producing a null-terminated UTF-16 registry value that
    /// keeps the original `REG_EXPAND_SZ` type when present.
    fn prepend_shim_dir(path: &str, shim_dir: &str, vtype: RegType) -> RegValue<'static> {
        let vtype = match vtype {
            REG_EXPAND_SZ => REG_EXPAND_SZ,
            _ => RegType::REG_SZ,
        };
        let new_path = format!("{};{}", shim_dir, path);
        let bytes: Vec<u8> = new_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();

        RegValue {
            bytes: bytes.into(),
            vtype,
        }
    }

    /// Broadcasts `WM_SETTINGCHANGE` with `lParam = "Environment"` so that Explorer reloads the
    /// user environment. Best-effort: a failure here should not fail setup.
    fn broadcast_environment_change() {
        use windows_sys::Win32::Foundation::LPARAM;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };

        let setting: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut result: usize = 0;

        let sent = unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                setting.as_ptr() as LPARAM,
                SMTO_ABORTIFHUNG,
                5000,
                &mut result,
            )
        };

        if sent == 0 {
            debug!("Failed to broadcast WM_SETTINGCHANGE environment notification");
        }
    }

    #[cfg(test)]
    mod tests {
        use std::sync::atomic::{AtomicU32, Ordering};

        use super::*;

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        /// A disposable key under `HKCU\Software`, removed on drop so tests never touch the
        /// real `Environment` key.
        struct TempKey {
            key: RegKey,
            path: String,
        }

        impl TempKey {
            fn new() -> Self {
                let hkcu = RegKey::predef(HKEY_CURRENT_USER);
                let path = format!(
                    "Software\\VoltaSetupTest-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::SeqCst)
                );
                let (key, _) = hkcu.create_subkey(&path).expect("create temp key");
                Self { key, path }
            }
        }

        impl Drop for TempKey {
            fn drop(&mut self) {
                let hkcu = RegKey::predef(HKEY_CURRENT_USER);
                let _ = hkcu.delete_subkey_all(&self.path);
            }
        }

        fn expand_sz(text: &str) -> RegValue<'static> {
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            RegValue {
                bytes: bytes.into(),
                vtype: REG_EXPAND_SZ,
            }
        }

        #[test]
        fn preserves_path_longer_than_1024_characters() {
            let temp = TempKey::new();
            let long = format!("C:\\{}", "a".repeat(2000));
            temp.key.set_value("Path", &long).expect("seed Path");

            let shim = r"C:\Volta\bin";
            ensure_shim_in_path(&temp.key, shim).expect("update Path");

            let (path, vtype) = read_path(&temp.key).expect("read back Path");
            assert_eq!(vtype, RegType::REG_SZ);
            assert_eq!(path, format!("{};{}", shim, long));
        }

        #[test]
        fn preserves_expand_sz_type_and_references() {
            let temp = TempKey::new();
            let original = r"%USERPROFILE%\bin";
            temp.key
                .set_raw_value("Path", &expand_sz(original))
                .expect("seed Path");

            let shim = r"C:\Volta\bin";
            ensure_shim_in_path(&temp.key, shim).expect("update Path");

            let raw = temp.key.get_raw_value("Path").expect("read back Path");
            assert_eq!(raw.vtype, REG_EXPAND_SZ);
            let text = String::from_reg_value(&raw).expect("decode Path");
            assert_eq!(text, format!("{};{}", shim, original));
        }

        #[test]
        fn does_not_add_shim_twice() {
            let temp = TempKey::new();
            let shim = r"C:\Volta\bin";
            temp.key
                .set_value("Path", &format!("{};C:\\Other", shim))
                .expect("seed Path");

            ensure_shim_in_path(&temp.key, shim).expect("update Path");

            let (path, _) = read_path(&temp.key).expect("read back Path");
            assert_eq!(
                path.matches(shim).count(),
                1,
                "shim dir added twice: {path}"
            );
        }
    }
}
