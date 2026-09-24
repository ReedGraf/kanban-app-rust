# Kanban App in Rust

A small, native kanban board for Linux desktops, written in Rust with [egui](https://github.com/emilk/egui).
It doesn't need a server, an account or a database: your board is one readable JSON file.

I was tired of seeing all these kanban apps that either don't satisfty my needs out of a kanban organizer or they run some bloated web based app like Electron. So I decided I needed one made my way.

## Features

- **Columns (categories)**: add, rename (double-click the title), reorder and delete them.
  Give a column a **color theme** from its ☰ menu; it shows as the column's border.
- **Cards** have a title, a description, an optional **due date**, a **color label** and any number of **tags**.
- **Check off cards**: tick a card's checkbox and it fades out over 2.5 seconds, then moves to the **Archive**.
  Untick it during those seconds to keep it. From the archive you can restore cards or delete them for good.
- **Sort each column** by manual order, title, or due date. Sorting only changes the view, so your manual order is kept.
- **Drag and drop** cards between columns. In a manually sorted column you can drop a card onto another card to put it just above that card.
- **Due date hints**: overdue dates show in red, and dates due today or tomorrow show in orange.
- **Filter** cards by any text in their title, description or tags.
- **Settings menu** (⚙): pick Dark, Light, or Follow system. Your choice is saved. The menu also shows where the data file is.

Useful shortcuts: right-click a card for Edit/Done/Delete, double-click a card to edit it,
press `Ctrl+Enter` to save in the editor, and press `Esc` to close it.

## Where your data lives

Everything is stored in a single pretty-printed JSON file:

```
~/.local/share/kanban/board.json     ($XDG_DATA_HOME/kanban/board.json)
```

The file is written atomically (to a temp file, then renamed) every time something changes.
It is easy to back up, sync or put under version control, and you can edit it by hand.
If the file can't be parsed, a copy is saved as `board.json.bak` and a fresh board is started, so nothing is overwritten.

## Installing

Each distro has its own package format, so you build the package on your own machine.
All builds need a Rust toolchain (1.85 or newer, for edition 2024). Use [rustup](https://rustup.rs) or your distro's `rust`/`cargo` package.

### Arch Linux, Manjaro, EndeavourOS, CachyOS

```sh
sudo pacman -S --needed base-devel rust
git clone https://github.com/ReedGraf/kanban-app-rust.git kanban && cd kanban
make arch                      # runs makepkg in packaging/arch
sudo pacman -U packaging/arch/kanban-*.pkg.tar.zst
```

### Debian, Ubuntu, Linux Mint, Pop!_OS

```sh
sudo apt install build-essential curl pkg-config libxkbcommon-dev libwayland-dev libgl-dev
curl https://sh.rustup.rs -sSf | sh     # Debian/Ubuntu's own cargo is often too old
cd kanban
make deb                       # installs cargo-deb if needed
sudo apt install ./target/debian/kanban_*.deb
```

### Fedora, RHEL, Rocky, AlmaLinux, openSUSE

```sh
# Fedora/RHEL:
sudo dnf install gcc rust cargo libxkbcommon-devel wayland-devel mesa-libGL-devel
# openSUSE:
sudo zypper install gcc rust cargo libxkbcommon-devel wayland-devel Mesa-libGL-devel

cd kanban
make rpm                       # installs cargo-generate-rpm if needed
sudo dnf install ./target/generate-rpm/kanban-*.rpm      # or: sudo zypper install ./target/generate-rpm/kanban-*.rpm
```

### Any other distro (Void, Gentoo, NixOS shell, Alpine…)

```sh
cd kanban
make install                   # installs into /usr/local (use sudo if needed)
# or install somewhere else:
make install PREFIX=$HOME/.local
```

To remove it, run `make uninstall` with the same `PREFIX`. You can also just run `cargo run --release` without installing.

### Runtime requirements

The app draws with OpenGL and works on both Wayland and X11. It needs `libxkbcommon`, `libwayland-client` and/or `libX11`,
and an OpenGL driver (Mesa or a vendor driver). A normal desktop install already has all of these.
The `.deb` and `.rpm` packages list their dependencies automatically.

## Project layout

```
src/main.rs            UI: the board, columns, cards, the editor window and the archive panel
src/model.rs           data model plus loading/saving the JSON file
assets/                .desktop launcher and SVG icon
packaging/arch/        PKGBUILD for Arch-based distros
Cargo.toml             includes the cargo-deb and cargo-generate-rpm packaging metadata
Makefile               build, install, deb, rpm and arch targets
```

## License

MIT. See [LICENSE](LICENSE).

## AI Usage

Real Good AI REAL Rating

[![alt text](4rr.png)](https://www.realgoodai.org/real-rating)