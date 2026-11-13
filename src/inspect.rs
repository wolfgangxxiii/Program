//! All routine inspection is read-only, with errors treated as unavailable evidence.
use crate::core::{self, Family, MenuEntry, Profile};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CONFIG: &str = "/etc/default/grub";
pub const MANIFEST: &str = "/etc/grub-studio-universal/entries.tsv";
pub const MENU_SCRIPT: &str = "/etc/grub.d/42_grub-studio";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme { pub name: String, pub path: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsRecord { pub name: String, pub source: String, pub detail: String }
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub profile: Profile,
    pub config: BTreeMap<String, String>,
    pub themes: Vec<Theme>,
    pub systems: Vec<OsRecord>,
    pub backups: Vec<String>,
    pub menu: Vec<MenuEntry>,
    pub generated: Vec<String>,
    pub deployments: Vec<String>,
}

pub fn program_path(name: &str) -> Option<PathBuf> {
    if !["pkexec", "grub-mkconfig", "grub2-mkconfig", "grub-script-check",
        "grub2-script-check", "grub2-editenv", "efibootmgr", "os-prober",
        "rpm-ostree", "timeout"].contains(&name) { return None }
    ["/usr/sbin", "/usr/bin", "/sbin", "/bin"].iter()
        .map(|d| Path::new(d).join(name))
        .find(|p| p.is_file())
}
pub fn run_capture(name: &str, args: &[&str]) -> Option<String> {
    let path = program_path(name)?;
    let output = Command::new(path).args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).chars().take(250_000).collect())
    } else { None }
}

pub fn profile() -> Profile {
    let info = core::parse_os_release(&fs::read_to_string("/etc/os-release").unwrap_or_default());
    let family = core::select_family(&info,
        Path::new("/usr/libexec/bazzite-boot-remount").is_file(),
        Path::new("/run/ostree-booted").exists());
    let efi = Path::new("/sys/firmware/efi").is_dir();
    let loader = if efi { run_capture("efibootmgr", &["-v"]).unwrap_or_default() } else { String::new() };
    let bootloader = if efi { core::detect_bootloader(&loader) } else { "unknown" };
    let target = family.target(efi);
    // symlink_metadata does not follow a protected Bazzite grub.cfg.
    let target_exists = !target.is_empty() && fs::symlink_metadata(target).is_ok();
    let generator_exists = program_path(family.generator()).is_some();
    core::profile_from_evidence(&info, family, efi, bootloader, target_exists, generator_exists)
}

pub fn scan_themes() -> Vec<Theme> {
    let roots = ["/boot/grub2/themes", "/boot/grub/themes", "/usr/share/grub/themes",
        "/usr/share/grub2/themes", "/etc/grub/themes", "/boot/themes"];
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for base in roots {
        let Ok(dir) = fs::read_dir(base) else { continue };
        for item in dir.flatten() {
            let name = item.file_name().to_string_lossy().to_string();
            if !core::valid_simple_name(&name) { continue }
            let path = item.path().join("theme.txt");
            if !item.path().is_dir() || !path.is_file() { continue }
            let canonical = path.to_string_lossy().into_owned();
            if seen.insert(canonical.clone()) { found.push(Theme { name, path: canonical }); }
        }
    }
    found.sort_by_key(|x| x.name.to_lowercase());
    found
}

pub fn scan_uefi(output: &str) -> Vec<OsRecord> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for line in output.lines().take(256) {
        if line.len() < 9 || !line.starts_with("Boot") || !line[4..8].bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let tail = line[8..].trim_start_matches('*').trim();
        if tail.is_empty() { continue }
        let title = tail.split("HD(").next().unwrap_or(tail).split("File(").next()
            .unwrap_or(tail).trim();
        let lower = title.to_lowercase();
        let relevant = ["windows", "ubuntu", "debian", "fedora", "opensuse", "arch",
            "mint", "manjaro", "cachyos", "linux", "grub", "refind", "limine", "bazzite"]
            .iter().any(|marker| lower.contains(marker));
        if !relevant { continue }
        let title = title.chars().take(90).collect::<String>();
        if seen.insert(title.to_lowercase()) {
            result.push(OsRecord { name: title, source: "UEFI".into(), detail: line.chars().take(230).collect() });
        }
    }
    result
}

pub fn scan_efi_files() -> Vec<OsRecord> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for root in ["/boot/efi/EFI", "/efi/EFI", "/boot/EFI"] {
        let Ok(items) = fs::read_dir(root) else { continue };
        for dir in items.flatten() {
            let name = dir.file_name().to_string_lossy().to_string();
            if !core::valid_simple_name(&name) || !dir.path().is_dir() { continue }
            for candidate in ["Boot/bootmgfw.efi", "shimx64.efi", "grubx64.efi", "systemd-bootx64.efi"] {
                let path = dir.path().join(candidate);
                if path.is_file() {
                    let desc = format!("{}/{}", name, candidate);
                    if seen.insert(desc.to_ascii_lowercase()) {
                        result.push(OsRecord { name: if name.eq_ignore_ascii_case("microsoft") {
                            "Windows Boot Manager".into() } else { name.clone() },
                            source: "EFI".into(), detail: path.to_string_lossy().to_string() });
                    }
                }
            }
        }
    }
    result
}

pub fn scan_systems() -> Vec<OsRecord> {
    let mut result = run_capture("efibootmgr", &["-v"])
        .map(|s| scan_uefi(&s)).unwrap_or_default();
    result.extend(scan_efi_files());
    result.sort_by_key(|s| (s.name.to_lowercase(), s.source.clone()));
    result
}

pub fn read_custom_menu() -> Vec<MenuEntry> {
    let text = fs::read_to_string(MANIFEST).unwrap_or_default();
    core::entries_from_tsv(&text).unwrap_or_default()
}
pub fn parse_generated_menu(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let rest = line.strip_prefix("menuentry ").or_else(|| line.strip_prefix("submenu "));
        let Some(rest) = rest else { continue };
        let Some(quote) = rest.chars().next() else { continue };
        if quote != '\'' && quote != '"' { continue }
        let title = rest[1..].split(quote).next().unwrap_or("");
        if !title.is_empty() && title.len() < 180 {
            out.push(title.to_string());
            if out.len() >= 150 { break; }
        }
    }
    out
}
pub fn generated_entries(p: &Profile) -> Vec<String> {
    let path = p.family.target(p.efi);
    if path.is_empty() { return vec![] }
    // Metadata failures, including EACCES, never prevent launching the GUI.
    let Ok(metadata) = fs::metadata(path) else { return vec![] };
    if !metadata.is_file() || metadata.len() > 3_000_000 { return vec![] }
    fs::read_to_string(path).map(|text| parse_generated_menu(&text)).unwrap_or_default()
}

pub fn scan_backups(family: Family) -> Vec<String> {
    let mut result = Vec::new();
    if let Ok(items) = fs::read_dir(family.backup_root()) {
        for entry in items.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if valid_backup_id(&name) && entry.path().is_dir() { result.push(name); }
        }
    }
    result.sort_by(|a, b| b.cmp(a));
    result
}
pub fn valid_backup_id(name: &str) -> bool {
    let sections: Vec<_> = name.split('-').collect();
    sections.len() == 3 && sections[0].len() == 8 && sections[1].len() == 6
        && !sections[2].is_empty() && sections.iter().all(|x| x.bytes().all(|b| b.is_ascii_digit()))
}

pub fn deployments() -> Vec<String> {
    // Deployment metadata only. No reads of protected, generated grub.cfg.
    let Some(raw) = run_capture("rpm-ostree", &["status", "--json"]) else { return Vec::new() };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else { return Vec::new() };
    let Some(deps) = json.get("deployments").and_then(|v| v.as_array()) else { return Vec::new() };
    deps.iter().take(25).enumerate().map(|(i, d)| {
        let os = d.get("osname").and_then(|v| v.as_str()).unwrap_or("Linux");
        let version = d.get("version").and_then(|v| v.as_str())
            .or_else(|| d.get("checksum").and_then(|v| v.as_str())).unwrap_or("unknown");
        let current = d.get("booted").and_then(|v| v.as_bool()).unwrap_or(false);
        format!("{} · {} · {}{}", i, os, version.chars().take(64).collect::<String>(),
            if current { " (booted)" } else { "" })
    }).collect()
}

pub fn inspect() -> Snapshot {
    let p = profile();
    Snapshot {
        config: core::parse_grub_config(&fs::read_to_string(CONFIG).unwrap_or_default()),
        themes: scan_themes(),
        systems: scan_systems(),
        backups: scan_backups(p.family),
        menu: read_custom_menu(),
        generated: generated_entries(&p),
        deployments: deployments(),
        profile: p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_windows_and_linux() {
        let records = scan_uefi("BootCurrent: 0001\nBoot0001* Fedora HD(1,GPT,abc)File(\\EFI\\fedora\\shimx64.efi)\nBoot0002* Windows Boot Manager HD(1,GPT,abc)File(\\EFI\\Microsoft\\Boot\\bootmgfw.efi)\n");
        assert_eq!(records.len(), 2);
        assert!(records.iter().any(|r| r.name.contains("Windows")));
    }
    #[test]
    fn accepts_only_owned_backup_folders() {
        assert!(valid_backup_id("20261113-153231-1234"));
        assert!(!valid_backup_id("../20261113-153231-1234"));
        assert!(!valid_backup_id("20261113-153231-1/foo"));
    }
    #[test]
    fn parses_generated_grub_titles_read_only() {
        let rows=parse_generated_menu("menuentry 'Bazzite Linux' --id=bazzite {\n submenu \"Advanced options\" {\n");
        assert_eq!(rows,vec!["Bazzite Linux","Advanced options"]);
    }
}
