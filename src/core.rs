use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Bazzite,
    Fedora,
    Debian,
    Arch,
    Suse,
    ReadOnly,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bazzite => "bazzite",
            Self::Fedora => "fedora",
            Self::Debian => "debian",
            Self::Arch => "arch",
            Self::Suse => "suse",
            Self::ReadOnly => "readonly",
        }
    }
    pub fn target(self, efi: bool) -> &'static str {
        match self {
            Self::Bazzite if efi => "/etc/grub2-efi.cfg",
            Self::Bazzite => "/etc/grub2.cfg",
            Self::Fedora | Self::Suse => "/boot/grub2/grub.cfg",
            Self::Debian | Self::Arch => "/boot/grub/grub.cfg",
            Self::ReadOnly => "",
        }
    }
    pub fn generator(self) -> &'static str {
        match self {
            Self::Bazzite | Self::Fedora | Self::Suse => "grub2-mkconfig",
            Self::Debian | Self::Arch => "grub-mkconfig",
            Self::ReadOnly => "",
        }
    }
    pub fn theme_root(self) -> &'static str {
        match self {
            Self::Bazzite | Self::Fedora | Self::Suse => "/boot/grub2/themes",
            Self::Debian | Self::Arch => "/boot/grub/themes",
            Self::ReadOnly => "",
        }
    }
    pub fn backup_root(self) -> &'static str {
        match self {
            Self::Bazzite => "/var/lib/bazzite-grub-studio/backups",
            _ => "/var/lib/grub-studio-universal/backups",
        }
    }
    pub fn supports_menu_edit(self) -> bool {
        matches!(self, Self::Fedora | Self::Suse | Self::Debian | Self::Arch)
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub family: Family,
    pub efi: bool,
    pub bootloader: String,
    pub writable: bool,
    pub reasons: Vec<String>,
}

pub fn parse_os_release(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.is_empty() || !key.chars().all(|x| x.is_ascii_uppercase() || x == '_' || x.is_ascii_digit()) {
            continue;
        }
        let value = value.trim();
        let stripped = if value.len() >= 2 &&
            ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\''))) {
            &value[1..value.len() - 1]
        } else { value };
        out.insert(key.to_string(), stripped.replace("\\\"", "\""));
    }
    out
}

pub fn select_family(info: &BTreeMap<String, String>, bazzite_helper: bool, ostree: bool) -> Family {
    let id = info.get("ID").map(String::as_str).unwrap_or("").to_ascii_lowercase();
    let variant = info.get("VARIANT_ID").map(String::as_str).unwrap_or("").to_ascii_lowercase();
    if ["silverblue", "kinoite", "sericea", "onyx", "sway-atomic", "aeon", "kalpa", "microos", "coreos"]
        .contains(&variant.as_str()) { return Family::ReadOnly; }
    if ostree && id != "bazzite" { return Family::ReadOnly; }
    if ["silverblue", "kinoite", "sericea", "onyx", "bluefin", "aurora", "steamos",
        "blendos", "vanilla", "nixos", "guix", "pop", "pop_os"].contains(&id.as_str())
        || id.starts_with("opensuse-microos") || id.starts_with("opensuse-aeon")
        || id.starts_with("opensuse-kalpa") { return Family::ReadOnly; }
    match id.as_str() {
        "bazzite" if bazzite_helper => Family::Bazzite,
        "fedora" | "nobara" | "ultramarine" | "rhel" | "centos" | "rocky" | "almalinux" => Family::Fedora,
        "ubuntu" | "debian" | "linuxmint" | "mint" | "elementary" | "zorin" | "kali" | "mx" | "neon" | "raspbian" => Family::Debian,
        "arch" | "cachyos" | "endeavouros" | "manjaro" | "garuda" | "artix" => Family::Arch,
        "suse" | "sles" => Family::Suse,
        name if name.starts_with("opensuse") => Family::Suse,
        _ => Family::ReadOnly,
    }
}

pub fn detect_bootloader(efi_output: &str) -> &'static str {
    let current = efi_output.lines().find_map(|s| s.strip_prefix("BootCurrent:"))
        .map(str::trim).unwrap_or("");
    if current.len() != 4 || !current.chars().all(|c| c.is_ascii_hexdigit()) {
        return "unknown";
    }
    for line in efi_output.lines() {
        if line.len() < 8 || !line[..4].eq_ignore_ascii_case("Boot") { continue }
        if !line[4..8].eq_ignore_ascii_case(current) { continue }
        let low = line.to_ascii_lowercase();
        if ["systemd-boot", "limine", "refind", "windows boot manager", "bootmgfw.efi",
            "linux boot manager", "sd-boot"].iter().any(|x| low.contains(x)) {
            return "other";
        }
        if ["grubx64.efi", "grub.efi", "grub2", "shimx64.efi", "shim.efi",
            "fedora", "ubuntu", "debian", "opensuse", "cachyos"].iter().any(|x| low.contains(x)) {
            return "grub";
        }
        break;
    }
    "unknown"
}

pub fn profile_from_evidence(info: &BTreeMap<String, String>, family: Family,
    efi: bool, bootloader: &str, target_exists: bool, generator_exists: bool) -> Profile {
    let mut reasons = Vec::new();
    if family == Family::ReadOnly { reasons.push("Unsupported or atomic distribution: review only".into()); }
    if family != Family::ReadOnly && !target_exists { reasons.push(format!("GRUB configuration not found: {}", family.target(efi))); }
    if family != Family::ReadOnly && !generator_exists { reasons.push(format!("GRUB generator not found: {}", family.generator())); }
    if bootloader == "other" { reasons.push("Current UEFI entry uses another bootloader".into()); }
    if efi && bootloader == "unknown" {
        reasons.push("Active UEFI bootloader could not be verified as GRUB".into());
    }
    Profile {
        name: info.get("PRETTY_NAME").or(info.get("NAME")).cloned().unwrap_or_else(|| "Unknown Linux".into()),
        family, efi, bootloader: bootloader.to_owned(), writable: reasons.is_empty(), reasons,
    }
}

pub fn parse_grub_config(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        if !key.starts_with("GRUB_") || !key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            continue;
        }
        let val = value.trim();
        let val = if val.len() >= 2 &&
            ((val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\''))) {
            &val[1..val.len() - 1]
        } else { val };
        out.insert(key.to_string(), val.to_string());
    }
    out
}

pub fn rewrite_grub_config(original: &str, updates: &BTreeMap<&str, String>) -> String {
    let mut text = String::new();
    for line in original.lines() {
        let norm = line.trim().strip_prefix("export ").unwrap_or(line.trim()).trim();
        let key = norm.split_once('=').map(|(k, _)| k.trim()).unwrap_or("");
        if !updates.contains_key(key) { text.push_str(line); text.push('\n'); }
    }
    text.push_str("\n# GRUB Studio Universal 2.0 - managed settings\n");
    for (key, value) in updates {
        if !value.is_empty() { text.push_str(&format!("{key}=\"{value}\"\n")); }
    }
    text
}

pub fn valid_timeout(value: &str) -> bool {
    value == "-1" || value.parse::<u8>().map(|n| n <= 60 && value == n.to_string()).unwrap_or(false)
}
pub fn valid_style(value: &str) -> bool {
    matches!(value, "menu" | "hidden" | "countdown")
}
pub fn valid_resolution(value: &str) -> bool {
    if value == "auto" { return true }
    let Some((w, h)) = value.split_once('x') else { return false };
    ["640", "800", "1024", "1280", "1366", "1600", "1920", "2560", "3840"].contains(&w)
        && ["480", "600", "720", "768", "800", "900", "1080", "1200", "1440", "1600", "2160"].contains(&h)
}
pub fn valid_theme_path(value: &str) -> bool {
    if value.is_empty() { return true }
    let p = Path::new(value);
    if p.file_name().and_then(|n| n.to_str()) != Some("theme.txt") { return false }
    let Some(name) = p.parent().and_then(|a| a.file_name()).and_then(|s| s.to_str()) else { return false };
    let Some(root) = p.parent().and_then(Path::parent).and_then(Path::to_str) else { return false };
    valid_simple_name(name) &&
        ["/boot/grub2/themes", "/boot/grub/themes", "/usr/share/grub/themes",
            "/usr/share/grub2/themes", "/etc/grub/themes", "/boot/themes"].contains(&root)
}
pub fn valid_simple_name(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64 && v.as_bytes()[0].is_ascii_alphanumeric()
    && v.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
    && v != "." && v != ".."
}
pub fn valid_default(v: &str) -> bool {
    v == "0" || (v.starts_with("gs-") && v.len() == 15 && v[3..].bytes().all(|b| b.is_ascii_hexdigit()))
    || (!v.is_empty() && v.len() <= 130 && (v.as_bytes()[0].is_ascii_alphabetic() || v.starts_with('_'))
        && v.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'_'|b'.'|b'+'|b'-')))
}
pub fn valid_kernel_args(v: &str) -> bool {
    v.len() <= 512 && (v.is_empty() || v.split(' ').all(|t| !t.is_empty() && t.len() <= 90
        && t.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'_'|b'.'|b'/'|b'='|b':'|b','|b'+'|b'-'))))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub id: String,
    pub title: String,
    pub uuid: String,
    pub path: String,
    pub enabled: bool,
}
pub fn valid_entry(e: &MenuEntry) -> bool {
    if !e.id.starts_with("gs-") || e.id.len() != 15 || !e.id[3..].bytes().all(|b| b.is_ascii_hexdigit()) { return false }
    if e.title.is_empty() || e.title.len() > 140 || e.title.trim() != e.title
        || e.title.chars().any(|c| c.is_control() || "'\"$\\`{};".contains(c)) { return false }
    if e.uuid.len() < 4 || e.uuid.len() > 64 || !e.uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') { return false }
    if !e.path.starts_with("/EFI/") || !e.path.to_ascii_lowercase().ends_with(".efi") { return false }
    let parts: Vec<&str> = e.path[5..].split('/').collect();
    if !(2..=6).contains(&parts.len()) || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.'|b'_'|b'-'))) { return false }
    true
}
pub fn validate_entries(entries: &[MenuEntry]) -> Result<(), String> {
    if entries.len() > 24 { return Err("Maximum 24 custom EFI entries".into()) }
    let mut ids = HashSet::new();
    for e in entries {
        if !valid_entry(e) { return Err(format!("Invalid EFI entry: {}", e.title)); }
        if !ids.insert(&e.id) { return Err("Duplicate EFI entry identifier".into()); }
    }
    Ok(())
}
pub fn entries_to_tsv(entries: &[MenuEntry]) -> Result<String, String> {
    validate_entries(entries)?;
    let mut rows = String::new();
    for e in entries {
        rows.push_str(&format!("{}\t{}\t{}\t{}\t{}\n",
            e.id, e.title, e.uuid.to_uppercase(), e.path, u8::from(e.enabled)));
    }
    if rows.len() > 12000 { return Err("EFI entry manifest too large".into()) }
    Ok(rows)
}
pub fn entries_from_tsv(text: &str) -> Result<Vec<MenuEntry>, String> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with("# GRUB STUDIO ENTRIES") { continue }
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() != 5 || !matches!(parts[4], "0" | "1") { return Err("Invalid stored EFI entry".into()) }
        rows.push(MenuEntry { id: parts[0].into(), title: parts[1].into(), uuid: parts[2].into(),
            path: parts[3].into(), enabled: parts[4] == "1" });
    }
    validate_entries(&rows)?;
    Ok(rows)
}
pub fn render_grub_entries(entries: &[MenuEntry]) -> Result<String, String> {
    validate_entries(entries)?;
    let mut s = "#!/bin/sh\nexec tail -n +4 \"$0\"\n# GRUB STUDIO MANAGED v1\n".to_string();
    for e in entries.iter().filter(|e| e.enabled) {
        s.push_str(&format!("\nmenuentry '{}' --id={} {{\n  insmod part_gpt\n  insmod fat\n  search --no-floppy --fs-uuid --set=root {}\n  chainloader {}\n}}\n",
            e.title, e.id, e.uuid, e.path));
    }
    Ok(s)
}
pub fn hex_encode(text: &str) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(text.len() * 2);
    for b in text.bytes() { out.push(DIGITS[(b >> 4) as usize] as char); out.push(DIGITS[(b & 15) as usize] as char); }
    out
}
pub fn hex_decode(text: &str) -> Result<String, String> {
    if text.len() > 24000 || text.len() % 2 != 0 { return Err("Invalid encoded manifest length".into()) }
    fn digit(b: u8) -> Option<u8> {
        match b { b'0'..=b'9' => Some(b-b'0'), b'a'..=b'f' => Some(b-b'a'+10), _ => None }
    }
    let mut bytes = Vec::with_capacity(text.len()/2);
    for part in text.as_bytes().chunks_exact(2) {
        bytes.push((digit(part[0]).ok_or("Invalid hex")? << 4) | digit(part[1]).ok_or("Invalid hex")?);
    }
    String::from_utf8(bytes).map_err(|_| "Invalid UTF-8 manifest".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distro_profiles_are_conservative() {
        let release = parse_os_release("ID=bazzite\nPRETTY_NAME=\"Bazzite Linux\"\n");
        assert_eq!(select_family(&release, true, true), Family::Bazzite);
        assert_eq!(select_family(&release, false, true), Family::ReadOnly);
        assert_eq!(select_family(&parse_os_release("ID=cachyos"), false, false), Family::Arch);
        assert_eq!(select_family(&parse_os_release("ID=ubuntu"), false, false), Family::Debian);
        assert_eq!(select_family(&parse_os_release("ID=fedora\nVARIANT_ID=kinoite"), false, true), Family::ReadOnly);
        assert!(!profile_from_evidence(&release, Family::Bazzite, true, "unknown", true, true).writable);
    }
    #[test]
    fn bootloader_detection() {
        assert_eq!(detect_bootloader("BootCurrent: 0003\nBoot0003* Limine EFI"), "other");
        assert_eq!(detect_bootloader("BootCurrent: 0004\nBoot0004* Fedora /EFI/fedora/shimx64.efi"), "grub");
        assert_eq!(detect_bootloader("Boot0004* Windows Boot Manager"), "unknown");
    }
    #[test]
    fn config_never_executes_shell() {
        let map = parse_grub_config("GRUB_TIMEOUT=\"5\"\nGRUB_THEME=\"/boot/grub/themes/legion/theme.txt\"\nGRUB_BAD=$(touch /tmp/no)\n");
        assert_eq!(map["GRUB_TIMEOUT"], "5");
        let mut updates = BTreeMap::new();
        updates.insert("GRUB_TIMEOUT", "8".to_string());
        let rewritten = rewrite_grub_config("GRUB_TIMEOUT=3\n# keep\nGRUB_CMDLINE_LINUX_DEFAULT=\"quiet\"\n", &updates);
        assert!(rewritten.contains("# keep"));
        assert!(rewritten.contains("GRUB_CMDLINE_LINUX_DEFAULT"));
        assert_eq!(parse_grub_config(&rewritten)["GRUB_TIMEOUT"], "8");
    }
    #[test]
    fn validates_settings() {
        assert!(valid_timeout("-1") && valid_timeout("0") && valid_timeout("60"));
        assert!(!valid_timeout("61") && !valid_timeout("05"));
        assert!(valid_theme_path("/boot/grub2/themes/legion/theme.txt"));
        assert!(!valid_theme_path("/tmp/a/theme.txt"));
        assert!(!valid_theme_path("/boot/grub/themes/../theme.txt"));
        assert!(!valid_kernel_args("quiet splash iommu=pt"));
        assert!(!valid_kernel_args("quiet;reboot"));
        assert!(valid_default("gs-0123456789ab"));
    }
    #[test]
    fn roundtrip_efi_menu() {
        let e = MenuEntry {id:"gs-0123456789ab".into(), title:"Windows 11".into(),
            uuid:"ABCD-1234".into(), path:"/EFI/Microsoft/Boot/bootmgfw.efi".into(), enabled:true};
        let rows = vec![e.clone()];
        let encoded = hex_encode(&entries_to_tsv(&rows).unwrap());
        assert_eq!(entries_from_tsv(&hex_decode(&encoded).unwrap()).unwrap(), rows);
        assert!(render_grub_entries(&rows).unwrap().contains("chainloader /EFI/Microsoft/Boot/bootmgfw.efi"));
        let mut bad = e;
        bad.title = "danger' ; reboot".into();
        assert!(!valid_entry(&bad));
    }
}
