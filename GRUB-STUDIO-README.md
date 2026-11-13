# GRUB Studio Universal 2.0.0 — Rust Edition

Natywna aplikacja GTK4/Wayland w języku Rust. Projekt jest nowym portem rozwiązania 1.9.0. **Wersja źródłowa / prototyp do testów**, nieprzetestowany jeszcze build produkcyjny.

## Funkcje

- Responsywny, kompaktowy interfejs GTK4: układ boczny z możliwością schowania na małym ekranie; przewijanie każdej strony.
- Ciemny i jasny wygląd, siedem języków (PL/EN/DE/ES/FR/UK/PT-BR), ustawienia użytkownika w katalogu domowym.
- Wykrywanie dystrybucji: Bazzite, Fedora, Debian/Ubuntu, Arch/CachyOS, openSUSE; ograniczanie operacji w systemach atomowych i przy niezgodnym bootloaderze.
- Wykrywanie już zainstalowanych motywów GRUB (katalogi zawierające `theme.txt`).
- Wykrywanie śladów Windows i innych systemów w UEFI i na zamontowanej partycji EFI; głębszy skan przez `os-prober` wyłącznie po zgodzie.
- Edycja czasu, stylu, rozdzielczości GRUB i wyboru motywu; `/etc/default/grub` jest tworzony dopiero podczas zatwierdzonego zapisu, jeżeli go nie ma.
- Osobny, kompilowany w Rust helper uruchamiany przez `pkexec`; GUI **nie działa jako root**, hasło administratora wpisuje się przez systemowy Polkit już podczas startu.
- Kopie zapasowe i próba cofnięcia zmian, gdy generowanie pliku GRUB się nie powiedzie.
- Edytor **własnych** wpisów EFI typu chainloader, obsługiwany na klasycznych GRUB Fedora/Arch/Debian/openSUSE. Automatycznie generowanych wpisów GRUB/OSTree nie edytujemy.
- Podgląd nazw wpisów wygenerowanych w GRUB (jeżeli system pozwala je odczytać); bez zapisu do `grub.cfg`.
- Zmiana widoczności menu poprzez `grub2-editenv` (Fedora/Bazzite), zmiana argumentów jądra tylko na Arch/Debian.
- Przegląd OSTree przez `rpm-ostree status --json` bez odczytu chronionego `/boot/grub2/grub.cfg`.

## Bezpieczeństwo

1. Nie uruchamiaj GUI przez `sudo`. Zgoda administracyjna odbywa się przez Polkit. Odmowa oznacza tryb podglądu.
2. Operacje roota mają zamkniętą listę akcji i walidują wartości oraz profil systemu ponownie.
3. Odmowa pisania na systemach z nieobsługiwanym lub innym aktywnym bootloaderem (Limine, systemd-boot, rEFInd).
4. Program nie zmienia kolejności UEFI, nie uruchamia `grub-install`, nie nadpisuje bezpośrednio `grub.cfg` z GUI.
5. Motywy GRUB na Bazzite nie są oficjalnie wspierane; zmiany mogą wymagać nośnika ratunkowego. Każdy zapis konfiguracji tworzy kopię.
6. Import obsługuje **katalog z motywem** (`theme.txt`), a nie jeszcze archiwa `.tar.gz` / `.zip`. Kontrolowane są typy plików, dowiązania, liczba i rozmiar.
7. W Bazzite podczas operacji root używany jest systemowy `/usr/libexec/bazzite-boot-remount`. Dla innych dystrybucji używany jest ich standardowy generator GRUB. Weryfikacja na konkretnym systemie jest obowiązkowa.

## Kompilacja na typowej Fedorze / Bazzite w Toolbox

Build wymaga Rust/Cargo, GTK4 development, kompilatora C i pkg-config **w środowisku kompilacji**. Nie należy dodawać tych pakietów do obrazu Bazzite.

```bash
# W środowisku Toolbox/Distrobox z Fedorą:
sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel
# Zainstaluj toolchain Rust przez rustup (jeżeli nie masz cargo).
cargo test --all-targets
cargo build --release --bins
GDK_BACKEND=wayland ./target/release/grub-studio-universal --demo
```

**Przenośność:** GTK4 wymaga bibliotek desktopowych. AppImage musi zawierać odpowiedni zestaw bibliotek/zasobów GTK4. Dostarczono `build-appimage.sh`, który używa `linuxdeploy`, jeżeli jest obecny. **Nie należy traktować AppImage jako przetestowanego, dopóki nie przejdzie prób na różnych dystrybucjach.** Bazzite zwykle ma GTK4, ale poprawny pakiet AppImage nie powinien zakładać dodatkowej instalacji bibliotek użytkownika.

Uruchamianie testowego GUI bez dotykania rzeczywistego GRUB:

```bash
cargo run --bin grub-studio-universal -- --demo
```

Przy normalnym uruchomieniu oba pliki wykonywalne muszą znajdować się obok siebie:

```bash
cargo build --release --bins
./target/release/grub-studio-universal
```

## Struktura

- `src/core.rs`: walidacja ustawień, profili, menu EFI, testy.
- `src/inspect.rs`: wykrywanie systemu, motywów, backupów i UEFI/EFI.
- `src/admin.rs`: zamknięty zestaw operacji z uprawnieniami roota, backup/rollback.
- `src/bin/helper.rs`: osobny helper root uruchamiany przez `pkexec`.
- `src/main.rs`: natywny interfejs GTK4 Wayland/X11.
- `assets/translations.json`: 238 tekstów w siedmiu językach (przeniesione i zaktualizowane z 1.9.0).
- `build-appimage.sh`, `packaging/`: pliki do budowy AppImage.

## Granice wersji 2.0.0

- To pierwszy port w Rust, a nie binarny odpowiednik funkcji 1.9.0. Wciąż potrzeba testów z rzeczywistym GRUB i GTK4 oraz dopracowania instalatora archiwów motywów i podglądu graficznego motywów.
- UEFI to dowód obecności wpisu w firmware, nie potwierdzenie istnienia systemu ani wpisu w wygenerowanym menu GRUB.
- Operacja odtworzenia przy niepowodzeniu może sama wymagać ręcznego odzyskania z kopii; **testuj najpierw na maszynie wirtualnej**.
- Zrzut demonstracyjny można wykonać na komputerze z GTK4 przez `--demo`. Nie przedstawiamy nieuruchomionego mockupu jako rzeczywistego zrzutu aplikacji.

Licencja: GPL-3.0-or-later. Niezależny projekt, niezwiązany z Bazzite.
