//! Privileged, tightly allowlisted operations; GUI code never runs as root.
//! The system profile is verified again INSIDE the root helper before every write.
use crate::core::{self, Family, Profile};
use crate::inspect::{self, CONFIG, MANIFEST, MENU_SCRIPT};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub type AdminResult<T> = Result<T, String>;
fn error(ctx: &str, e: std::io::Error) -> String { format!("{ctx}: {e}") }
fn clean_field(text: &str, limit: usize) -> String {
    text.chars().map(|c| if matches!(c, '\n' | '\r' | '\t') { ' ' } else { c })
        .take(limit).collect()
}
fn ensure_root() -> AdminResult<()> {
    unsafe extern "C" { fn geteuid() -> u32; }
    if unsafe { geteuid() } != 0 { return Err("Root helper must be launched through Polkit".into()); }
    Ok(())
}
fn safe_file(path: &Path) -> AdminResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(info) if info.file_type().is_symlink() => Err(format!("Refusing symlink: {}", path.display())),
        Ok(info) if info.is_file() => Ok(true),
        Ok(_) => Err(format!("Not a normal file: {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(error("Cannot check file", e)),
    }
}
fn read_existing(path: &Path) -> AdminResult<String> {
    if safe_file(path)? {
        fs::read_to_string(path).map_err(|e| error("Cannot read existing configuration", e))
    } else { Ok(String::new()) }
}
fn safe_dir(path: &Path) -> AdminResult<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| error("Cannot access directory", e))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(format!("Unsafe directory: {}", path.display()));
    }
    Ok(())
}
fn temp_file_path(path: &Path) -> AdminResult<PathBuf> {
    let parent = path.parent().ok_or("Path without parent")?;
    safe_dir(parent)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    Ok(parent.join(format!(".grub-studio-{}-{stamp}", std::process::id())))
}
fn atomic_write(path: &Path, content: &[u8], mode: u32) -> AdminResult<()> {
    safe_file(path)?;
    let temp = temp_file_path(path)?;
    let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&temp).map_err(|e| error("Cannot create staging file", e))?;
    let result = (|| {
        f.write_all(content).map_err(|e| error("Cannot write staging file", e))?;
        f.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|e| error("Cannot set file permissions", e))?;
        f.sync_all().map_err(|e| error("Cannot sync staging file", e))?;
        fs::rename(&temp, path).map_err(|e| error("Cannot atomically install file", e))
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}
fn ensure_owned_dir(path: &Path) -> AdminResult<()> {
    if !path.exists() { fs::create_dir_all(path).map_err(|e| error("Cannot create directory", e))?; }
    safe_dir(path)?;
    let md = fs::metadata(path).map_err(|e| error("Cannot stat directory", e))?;
    if md.uid() != 0 { return Err("Directory is not root-owned".into()) }
    Ok(())
}
fn run_fixed(program: &str, args: &[&str]) -> AdminResult<Output> {
    let exec = if program == "bash" { PathBuf::from("/usr/bin/bash") } else {
        inspect::program_path(program).ok_or_else(|| format!("Missing program: {program}"))?
    };
    Command::new(exec).args(args).output().map_err(|e| error("Cannot execute system command", e))
}
fn run_success(program: &str, args: &[&str]) -> AdminResult<()> {
    let out = run_fixed(program, args)?;
    if out.status.success() { return Ok(()) }
    Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).chars().take(1100).collect::<String>()))
}
fn supported_profile() -> AdminResult<Profile> {
    let p = inspect::profile();
    if !p.writable { return Err(format!("GRUB is read-only: {}", p.reasons.join("; "))); }
    if p.family == Family::ReadOnly { return Err("Unknown boot profile".into()) }
    safe_dir(Path::new("/etc/default"))?;
    safe_file(Path::new(CONFIG))?;
    // Root-side check: never use a stale profile from the unprivileged UI.
    if fs::symlink_metadata(p.family.target(p.efi)).is_err() {
        return Err("Expected GRUB target is missing".into())
    }
    if !Path::new("/boot").is_dir() { return Err("Boot directory missing".into()) }
    Ok(p)
}

struct BootMount(bool);
impl BootMount {
    fn new(p: &Profile) -> AdminResult<Self> {
        if p.family == Family::Bazzite {
            let helper = "/usr/libexec/bazzite-boot-remount";
            if !Path::new(helper).is_file() { return Err("Bazzite boot remount helper unavailable".into()) }
            run_success("bash", &["-c", "source /usr/libexec/bazzite-boot-remount; remount_boot_rw"])?;
            Ok(Self(true))
        } else { Ok(Self(false)) }
    }
}
impl Drop for BootMount {
    fn drop(&mut self) {
        if self.0 {
            match run_success("bash", &["-c", "source /usr/libexec/bazzite-boot-remount; remount_boot_ro"]) {
                Ok(_) => self.0 = false,
                Err(e) => eprintln!("CRITICAL: could not remount /boot read-only: {e}"),
            }
        }
    }
}
fn regen(p: &Profile) -> AdminResult<()> {
    let _mount = BootMount::new(p)?;
    let target = p.family.target(p.efi);
    run_success(p.family.generator(), &["-o", target])?;
    let checker = if inspect::program_path("grub2-script-check").is_some() { "grub2-script-check" }
                  else if inspect::program_path("grub-script-check").is_some() { "grub-script-check" }
                  else { "" };
    if !checker.is_empty() { run_success(checker, &[target])?; }
    Ok(())
}
fn backup_base(p: &Profile) -> PathBuf { PathBuf::from(p.family.backup_root()) }
fn copy_regular_if_exists(from: &Path, to: &Path, missing: &Path) -> AdminResult<()> {
    if safe_file(from)? {
        fs::copy(from, to).map_err(|e| error("Cannot copy file into backup", e))?;
        fs::set_permissions(to, fs::Permissions::from_mode(0o600))
            .map_err(|e| error("Cannot protect backup", e))?;
    } else {
        File::create(missing).map_err(|e| error("Cannot note absent file", e))?;
    }
    Ok(())
}
fn backup(p: &Profile) -> AdminResult<PathBuf> {
    let base = backup_base(p);
    ensure_owned_dir(&base)?;
    let stamp = run_fixed("bash", &["-c", "date -u +%Y%m%d-%H%M%S"])?
        .stdout;
    let stamp = String::from_utf8_lossy(&stamp).trim().to_string();
    if stamp.len() != 15 || !stamp.chars().filter(|c| *c != '-').all(|c| c.is_ascii_digit()) {
        return Err("Cannot generate a safe backup timestamp".into())
    }
    let dir = base.join(format!("{stamp}-{}", std::process::id()));
    fs::create_dir(&dir).map_err(|e| error("Cannot create backup", e))?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| error("Cannot protect backup dir", e))?;
    copy_regular_if_exists(Path::new(CONFIG), &dir.join("grub.orig"), &dir.join("grub.missing"))?;
    copy_regular_if_exists(Path::new(MENU_SCRIPT), &dir.join("custom.orig"), &dir.join("custom.missing"))?;
    copy_regular_if_exists(Path::new(MANIFEST), &dir.join("entries.orig"), &dir.join("entries.missing"))?;
    // GRUB-generated configuration can be root-only; copy read-only to backup.
    if let Ok(src) = fs::metadata(p.family.target(p.efi)) {
        if src.is_file() && src.len() <= 10_000_000 {
            let dest = dir.join("grub.cfg.orig");
            let _ = fs::copy(p.family.target(p.efi), &dest);
            let _ = fs::set_permissions(dest, fs::Permissions::from_mode(0o600));
        }
    }
    if matches!(p.family, Family::Bazzite | Family::Fedora) {
        let visibility = inspect::run_capture("grub2-editenv", &["-", "list"]).unwrap_or_default();
        let val = visibility.lines().find_map(|s| s.strip_prefix("menu_auto_hide=")).unwrap_or("unset");
        atomic_write(&dir.join("visibility"), val.as_bytes(), 0o600)?;
    }
    Ok(dir)
}
fn restore_one(backup_dir: &Path, filename: &str, missing: &str, target: &Path, mode: u32) -> AdminResult<()> {
    if safe_file(&backup_dir.join(filename))? {
        let data = fs::read(backup_dir.join(filename)).map_err(|e| error("Cannot read backup", e))?;
        atomic_write(target, &data, mode)
    } else if backup_dir.join(missing).is_file() {
        if safe_file(target)? { fs::remove_file(target).map_err(|e| error("Cannot remove newly created config", e))? }
        Ok(())
    } else { Err(format!("Incomplete backup: {filename}")) }
}
fn rollback_config(backup_dir: &Path) -> AdminResult<()> {
    restore_one(backup_dir, "grub.orig", "grub.missing", Path::new(CONFIG), 0o644)
}
fn rollback_menu(backup_dir: &Path) -> AdminResult<()> {
    restore_one(backup_dir, "custom.orig", "custom.missing", Path::new(MENU_SCRIPT), 0o755)?;
    ensure_owned_dir(Path::new("/etc/grub-studio-universal"))?;
    restore_one(backup_dir, "entries.orig", "entries.missing", Path::new(MANIFEST), 0o600)
}
fn rollback_generated(p: &Profile, backup_dir: &Path) -> AdminResult<()> {
    if backup_dir.join("grub.cfg.orig").is_file() {
        let _mount = BootMount::new(p)?;
        // Bazzite uses a validated, distribution-owned grub2.cfg symlink here.
        fs::copy(backup_dir.join("grub.cfg.orig"), p.family.target(p.efi))
            .map_err(|e| error("Could not restore previous generated GRUB file", e))?;
    }
    Ok(())
}
fn transactional(p: &Profile, update: impl FnOnce() -> AdminResult<()>, menu: bool) -> AdminResult<String> {
    let saved = backup(p)?;
    if let Err(e) = update() {
        let revert = if menu { rollback_menu(&saved) } else { rollback_config(&saved) };
        return Err(format!("Could not save changes: {e}. Revert: {:?}. Backup: {}",
            revert.err(), saved.display()));
    }
    if let Err(e) = regen(p) {
        let revert = if menu { rollback_menu(&saved) } else { rollback_config(&saved) };
        let regen_again = regen(p);
        if regen_again.is_err() { let _ = rollback_generated(p, &saved); }
        return Err(format!("Regeneration failed: {e}. Revert: {:?}. Previous GRUB backup: {}",
            revert.err(), saved.display()));
    }
    Ok(format!("Changes saved. Backup: {}", saved.display()))
}

fn apply_settings(p: &Profile, timeout: &str, style: &str, gfx: &str, theme: &str) -> AdminResult<String> {
    if !core::valid_timeout(timeout) || !core::valid_style(style) || !core::valid_resolution(gfx)
        || !core::valid_theme_path(theme) { return Err("Invalid GRUB setting".into()) }
    if !theme.is_empty() {
        let file = Path::new(theme);
        if !safe_file(file)? { return Err("Selected theme was not found".into()) }
        if file.parent().is_some_and(|d| fs::symlink_metadata(d).map(|m| m.file_type().is_symlink()).unwrap_or(true)) {
            return Err("Theme directory is a symlink or inaccessible".into())
        }
    }
    let mut vars = BTreeMap::new();
    vars.insert("GRUB_TIMEOUT", timeout.to_string());
    vars.insert("GRUB_TIMEOUT_STYLE", style.to_string());
    vars.insert("GRUB_GFXMODE", gfx.to_string());
    vars.insert("GRUB_THEME", theme.to_string());
    transactional(p, || {
        let original = read_existing(Path::new(CONFIG))?;
        let updated = core::rewrite_grub_config(&original, &vars);
        atomic_write(Path::new(CONFIG), updated.as_bytes(), 0o644)
    }, false)
}
fn apply_default(p: &Profile, value: &str) -> AdminResult<String> {
    if !core::valid_default(value) { return Err("Invalid GRUB default entry ID".into()) }
    if p.family == Family::Bazzite && value != "0" { return Err("Bazzite deployment IDs must not be changed".into()) }
    if value.starts_with("gs-") && !inspect::read_custom_menu().iter().any(|e| e.id==value && e.enabled) {
        return Err("Save and enable this custom entry before setting it as default".into())
    }
    transactional(p, || {
        let mut updates = BTreeMap::new();
        updates.insert("GRUB_DEFAULT", value.to_string());
        let original = read_existing(Path::new(CONFIG))?;
        atomic_write(Path::new(CONFIG), core::rewrite_grub_config(&original, &updates).as_bytes(), 0o644)
    }, false)
}
fn apply_kernel_args(p: &Profile, value: &str) -> AdminResult<String> {
    if !matches!(p.family, Family::Debian | Family::Arch) {
        return Err("Kernel arguments on this distribution are managed elsewhere".into())
    }
    if !core::valid_kernel_args(value) { return Err("Invalid kernel arguments".into()) }
    transactional(p, || {
        let mut updates = BTreeMap::new();
        updates.insert("GRUB_CMDLINE_LINUX_DEFAULT", value.to_string());
        let original = read_existing(Path::new(CONFIG))?;
        atomic_write(Path::new(CONFIG), core::rewrite_grub_config(&original, &updates).as_bytes(), 0o644)
    }, false)
}
fn apply_menu(p: &Profile, encoded: &str) -> AdminResult<String> {
    if !p.family.supports_menu_edit() { return Err("Managed custom menu is not supported on this boot profile".into()) }
    safe_dir(Path::new("/etc/grub.d"))?;
    if safe_file(Path::new(MENU_SCRIPT))? {
        let existing = fs::read_to_string(MENU_SCRIPT).map_err(|e| error("Cannot check managed script", e))?;
        if !existing.lines().any(|l| l == "# GRUB STUDIO MANAGED v1") {
            return Err("Refusing to overwrite an unmanaged /etc/grub.d/42_grub-studio".into())
        }
    }
    if safe_file(Path::new(MANIFEST))? {
        let existing = fs::read_to_string(MANIFEST).map_err(|e| error("Cannot check menu manifest", e))?;
        if !existing.starts_with("# GRUB STUDIO ENTRIES v1\n") { return Err("Unmanaged menu manifest".into()) }
    }
    let data = core::hex_decode(encoded)?;
    let entries = core::entries_from_tsv(&data)?;
    let script = core::render_grub_entries(&entries)?;
    transactional(p, || {
        ensure_owned_dir(Path::new("/etc/grub-studio-universal"))?;
        atomic_write(Path::new(MENU_SCRIPT), script.as_bytes(), 0o755)?;
        atomic_write(Path::new(MANIFEST), format!("# GRUB STUDIO ENTRIES v1\n{data}").as_bytes(), 0o600)
    }, true)
}
fn visibility(p: &Profile, mode: &str) -> AdminResult<String> {
    if !matches!(p.family, Family::Bazzite | Family::Fedora) { return Err("Visibility is only supported on Fedora/Bazzite".into()) }
    if inspect::program_path("grub2-editenv").is_none() { return Err("Missing grub2-editenv".into()) }
    let _backup = backup(p)?;
    let _mount = BootMount::new(p)?;
    match mode {
        "show" => run_success("grub2-editenv", &["-", "unset", "menu_auto_hide"])?,
        "auto" => run_success("grub2-editenv", &["-", "set", "menu_auto_hide=1"])?,
        "hide" => run_success("grub2-editenv", &["-", "set", "menu_auto_hide=2"])?,
        _ => return Err("Invalid visibility".into()),
    }
    Ok("GRUB menu visibility updated".into())
}
fn restore_visibility(p: &Profile, saved: &Path) -> AdminResult<()> {
    if !matches!(p.family, Family::Bazzite | Family::Fedora) { return Ok(()) }
    let path=saved.join("visibility");
    if !safe_file(&path)? { return Ok(()) }
    if inspect::program_path("grub2-editenv").is_none() { return Ok(()) }
    let value=fs::read_to_string(path).map_err(|e| error("Cannot read menu visibility backup",e))?;
    let _mount=BootMount::new(p)?;
    match value.trim() {
        "1" => run_success("grub2-editenv",&["-","set","menu_auto_hide=1"]),
        "2" => run_success("grub2-editenv",&["-","set","menu_auto_hide=2"]),
        _ => run_success("grub2-editenv",&["-","unset","menu_auto_hide"]),
    }
}
fn restore(p: &Profile, name: &str) -> AdminResult<String> {
    if !inspect::valid_backup_id(name) { return Err("Invalid backup name".into()) }
    let bdir = backup_base(p).join(name);
    safe_dir(&bdir)?;
    let current = backup(p)?;
    let result = (|| {
        rollback_config(&bdir)?;
        if bdir.join("custom.orig").exists() || bdir.join("custom.missing").exists() {
            rollback_menu(&bdir)?;
        }
        regen(p)?;
        restore_visibility(p,&bdir)?;
        Ok::<_, String>(())
    })();
    if let Err(e) = result {
        let _ = rollback_config(&current);
        let _ = rollback_menu(&current);
        if regen(p).is_err() { let _ = rollback_generated(p, &current); }
        let _ = restore_visibility(p,&current);
        return Err(format!("Restore failed; previous state recovered when possible: {e}"));
    }
    Ok(format!("Restored {name}. Previous state backed up: {}", current.display()))
}

fn safe_theme_copy(source: &Path, dest: &Path, count: &mut usize, bytes: &mut u64, depth: usize) -> AdminResult<()> {
    if depth > 8 { return Err("Theme directory nesting is too deep".into()) }
    for item in fs::read_dir(source).map_err(|e| error("Cannot read theme folder", e))? {
        let item = item.map_err(|e| error("Cannot read theme item", e))?;
        let name = item.file_name().to_string_lossy().to_string();
        if !core::valid_simple_name(&name) { return Err("Invalid name in theme".into()) }
        let md = fs::symlink_metadata(item.path()).map_err(|e| error("Cannot check theme item", e))?;
        if md.file_type().is_symlink() { return Err("Theme contains symlinks".into()) }
        *count += 1;
        if *count > 500 { return Err("Theme has too many files".into()) }
        let target = dest.join(&name);
        if md.is_dir() {
            fs::create_dir(&target).map_err(|e| error("Cannot create theme subfolder", e))?;
            safe_theme_copy(&item.path(), &target, count, bytes, depth+1)?;
        } else if md.is_file() {
            *bytes += md.len();
            if *bytes > 40_000_000 { return Err("Theme exceeds 40 MB".into()) }
            if !["txt", "pf2", "png", "jpg", "jpeg", "tga", "fnt", "bmp"].iter()
                .any(|ext| name.to_lowercase().ends_with(&format!(".{ext}"))) {
                return Err(format!("Unexpected file type in theme: {name}"))
            }
            fs::copy(item.path(), &target).map_err(|e| error("Cannot copy theme file", e))?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
                .map_err(|e| error("Cannot set theme permissions", e))?;
        } else { return Err("Theme contains unsupported filesystem object".into()) }
    }
    Ok(())
}
fn install_theme(p: &Profile, source: &str, name: &str) -> AdminResult<String> {
    if !core::valid_simple_name(name) { return Err("Invalid theme folder name".into()) }
    let root = Path::new(p.family.theme_root());
    if root.as_os_str().is_empty() { return Err("Theme folder not available".into()) }
    let source = Path::new(source);
    safe_dir(source)?;
    let canonical = fs::canonicalize(source).map_err(|e| error("Cannot resolve theme folder", e))?;
    if !["/home", "/var/home", "/tmp", "/var/tmp"].iter().any(|root| canonical.starts_with(root)) {
        return Err("Theme import must originate from a user or temporary folder".into());
    }
    let uid = std::env::var("PKEXEC_UID").ok().and_then(|id| id.parse::<u32>().ok())
        .ok_or("Polkit did not provide the unprivileged user ID")?;
    if fs::metadata(source).map_err(|e| error("Cannot check folder owner", e))?.uid() != uid {
        return Err("The theme folder must be owned by the requesting user".into());
    }
    if !source.join("theme.txt").is_file() { return Err("Missing theme.txt".into()) }
    if source.to_string_lossy().len() > 300 { return Err("Source path too long".into()) }
    let _mount = BootMount::new(p)?;
    ensure_owned_dir(root)?;
    let dest = root.join(name);
    if fs::symlink_metadata(&dest).is_ok() { return Err("Theme already installed".into()) }
    let temp = root.join(format!(".grub-studio-theme-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()));
    fs::create_dir(&temp).map_err(|e| error("Cannot create temporary theme dir", e))?;
    let mut count = 0;
    let mut bytes = 0;
    let result = safe_theme_copy(source, &temp, &mut count, &mut bytes, 0)
        .and_then(|_| fs::rename(&temp, &dest).map_err(|e| error("Cannot install theme", e)));
    if result.is_err() { let _ = fs::remove_dir_all(&temp); }
    result?;
    Ok(format!("Installed theme {name} ({count} items). Theme was not activated."))
}

pub fn probe_text() -> String {
    let s = inspect::inspect();
    let mut out = String::from("GRUB_STUDIO_PROBE_V2\n");
    let clean = |s: &str| clean_field(s, 290);
    out.push_str(&format!("PROFILE\t{}\t{}\t{}\t{}\n", s.profile.family.label(), u8::from(s.profile.writable),
        clean(&s.profile.name), clean(&s.profile.reasons.join("; "))));
    for (k, v) in s.config {
        // Do not expose arbitrary root-readable GRUB values to the unprivileged UI.
        if ["GRUB_TIMEOUT", "GRUB_TIMEOUT_STYLE", "GRUB_GFXMODE", "GRUB_THEME",
            "GRUB_DEFAULT", "GRUB_CMDLINE_LINUX_DEFAULT"].contains(&k.as_str()) {
            out.push_str(&format!("CONFIG\t{}\t{}\n", clean(&k), clean(&v)));
        }
    }
    for t in s.themes { out.push_str(&format!("THEME\t{}\t{}\n", clean(&t.name), clean(&t.path))); }
    for os in s.systems { out.push_str(&format!("OS\t{}\t{}\t{}\n", clean(&os.name), clean(&os.source), clean(&os.detail))); }
    for b in s.backups { out.push_str(&format!("BACKUP\t{}\n", clean(&b))); }
    for e in s.menu {
        out.push_str(&format!("ENTRY\t{}\t{}\t{}\t{}\t{}\n", clean(&e.id), clean(&e.title),
            clean(&e.uuid), clean(&e.path), u8::from(e.enabled)));
    }
    for title in s.generated { out.push_str(&format!("GENERATED\t{}\n", clean(&title))); }
    for d in s.deployments { out.push_str(&format!("DEPLOY\t{}\n", clean(&d))); }
    out
}

pub fn scan_deep() -> AdminResult<String> {
    let Some(program) = inspect::program_path("os-prober") else { return Err("os-prober is not installed".into()) };
    let timeout = inspect::program_path("timeout").ok_or("Missing timeout command")?;
    let out = Command::new(timeout).arg("50s").arg(program).output()
        .map_err(|e| error("Cannot execute os-prober", e))?;
    if !out.status.success() { return Err("os-prober failed or timed out".into()) }
    let mut result = String::from("GRUB_STUDIO_DEEP_V2\n");
    for line in String::from_utf8_lossy(&out.stdout).lines().take(80) {
        let title = clean_field(line.split(':').nth(1).unwrap_or(line), 90);
        result.push_str(&format!("OS\t{}\tos-prober\t{}\n", title.chars().take(90).collect::<String>(),
            clean_field(line, 270)));
    }
    Ok(result)
}
pub fn main_admin(args: &[String]) -> AdminResult<String> {
    ensure_root()?;
    match args.first().map(String::as_str) {
        Some("--probe") if args.len() == 1 => Ok(probe_text()),
        Some("--scan-deep") if args.len() == 1 => scan_deep(),
        Some("--backup") if args.len() == 1 => {
            let p = supported_profile()?;
            Ok(format!("Created backup: {}", backup(&p)?.display()))
        }
        Some("--apply") if args.len() == 5 => {
            let p = supported_profile()?;
            apply_settings(&p, &args[1], &args[2], &args[3], &args[4])
        }
        Some("--default") if args.len() == 2 => apply_default(&supported_profile()?, &args[1]),
        Some("--kernel-args") if args.len() == 2 => apply_kernel_args(&supported_profile()?, &args[1]),
        Some("--menu-update") if args.len() == 2 => apply_menu(&supported_profile()?, &args[1]),
        Some("--visibility") if args.len() == 2 => visibility(&supported_profile()?, &args[1]),
        Some("--restore") if args.len() == 2 => restore(&supported_profile()?, &args[1]),
        Some("--install-theme") if args.len() == 3 => install_theme(&supported_profile()?, &args[1], &args[2]),
        _ => Err("Unknown privileged operation / wrong argument count".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_commands_without_root() {
        // The helper does not accept shell fragments, environment variables or paths as actions.
        assert!(core::valid_theme_path("/boot/grub2/themes/legion/theme.txt"));
        assert!(!core::valid_theme_path("/tmp/test/theme.txt"));
        assert_eq!(inspect::valid_backup_id("20261113-142202-100"), true);
    }
}
