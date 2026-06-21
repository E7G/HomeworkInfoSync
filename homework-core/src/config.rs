use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChaoxingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KetangpaiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YuketangConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub csrftoken: String,
    #[serde(default)]
    pub sessionid: String,
    #[serde(default = "default_university_id")]
    pub university_id: String,
}

fn default_university_id() -> String {
    "3078".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub chaoxing: ChaoxingConfig,
    #[serde(default)]
    pub ketangpai: KetangpaiConfig,
    #[serde(default)]
    pub yuketang: YuketangConfig,
}

impl Default for YuketangConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            csrftoken: String::new(),
            sessionid: String::new(),
            university_id: default_university_id(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            chaoxing: ChaoxingConfig::default(),
            ketangpai: KetangpaiConfig::default(),
            yuketang: YuketangConfig::default(),
        }
    }
}

pub enum PlatformConfig {
    Chaoxing(ChaoxingConfig),
    Ketangpai(KetangpaiConfig),
    Yuketang(YuketangConfig),
}

pub fn app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn push_config_candidates(paths: &mut Vec<PathBuf>, start: &Path) {
    let mut dir = start.to_path_buf();
    paths.push(dir.join("config.json"));
    for _ in 0..8 {
        if !dir.pop() {
            break;
        }
        paths.push(dir.join("config.json"));
    }
}

fn config_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(env) = std::env::var_os("HOMEWORK_CONFIG") {
        paths.push(PathBuf::from(env));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_config_candidates(&mut paths, dir);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_config_candidates(&mut paths, &cwd);
    }
    paths.push(app_dir().join("config.json"));

    let mut seen = HashSet::new();
    paths.into_iter().filter(|p| seen.insert(p.clone())).collect()
}

/// First existing `config.json` on the search path, if any.
pub fn resolve_config_path() -> Option<PathBuf> {
    config_candidates()
        .into_iter()
        .find(|p| p.is_file())
}

/// Path used for load/save: existing file, or `config.json` next to the executable.
pub fn config_path() -> PathBuf {
    resolve_config_path().unwrap_or_else(|| app_dir().join("config.json"))
}

pub fn load_config() -> AppConfig {
    let path = match resolve_config_path() {
        Some(p) => p,
        None => return AppConfig::default(),
    };
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(cfg: &AppConfig) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    let path = config_path();
    clear_readonly(&path);
    fs::write(path, text)
}

/// Clear a read-only attribute before writing.
///
/// A `config.json` shipped in a release archive can land on disk read-only
/// (Windows preserves the zip entry's attribute). `fs::write` then fails with
/// "access denied" and a freshly scanned QR session is silently lost, so the
/// next launch reloads the stale credentials and reports them as expired.
/// Best-effort: ignore errors and let the subsequent write surface any real
/// permission problem.
fn clear_readonly(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }
}

impl AppConfig {
    pub fn should_auto_refresh(&self) -> bool {
        if self.chaoxing.enabled
            && !self.chaoxing.user.is_empty()
            && !self.chaoxing.password.is_empty()
        {
            return true;
        }
        if self.ketangpai.enabled
            && !self.ketangpai.email.is_empty()
            && !self.ketangpai.password.is_empty()
        {
            return true;
        }
        if self.yuketang.enabled
            && !self.yuketang.csrftoken.is_empty()
            && !self.yuketang.sessionid.is_empty()
        {
            return true;
        }
        false
    }
}
