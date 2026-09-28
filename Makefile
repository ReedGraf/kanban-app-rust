PREFIX ?= /usr/local
DESTDIR ?=

.PHONY: build install uninstall deb rpm arch clean

build:
	cargo build --release

install: build
	install -Dm755 target/release/kanban $(DESTDIR)$(PREFIX)/bin/kanban
	install -Dm644 assets/io.github.reedgraf.kanban.desktop $(DESTDIR)$(PREFIX)/share/applications/io.github.reedgraf.kanban.desktop
	install -Dm644 assets/io.github.reedgraf.kanban.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/io.github.reedgraf.kanban.svg

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/kanban \
	      $(DESTDIR)$(PREFIX)/share/applications/io.github.reedgraf.kanban.desktop \
	      $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/io.github.reedgraf.kanban.svg

# Debian / Ubuntu / Mint / Pop!_OS  -> target/debian/*.deb
deb:
	command -v cargo-deb >/dev/null || cargo install cargo-deb
	cargo deb

# Fedora / RHEL / openSUSE  -> target/generate-rpm/*.rpm
rpm: build
	command -v cargo-generate-rpm >/dev/null || cargo install cargo-generate-rpm
	cargo generate-rpm

# Arch / Manjaro / EndeavourOS / CachyOS  -> packaging/arch/*.pkg.tar.zst
arch:
	cd packaging/arch && makepkg -f

clean:
	cargo clean
	rm -rf packaging/arch/src packaging/arch/pkg packaging/arch/*.pkg.tar.*
