# Maintainer: Givemegitpls <givemegitpls@users.noreply.github.com>

pkgname=hyprlock-accent-git
_pkgbase=hyprlock-accent
pkgver=r3.b0e8dc8
pkgrel=1
pkgdesc='Compute accent/foreground colors and clock horizontal offset for hyprlock from current awww wallpaper'
url='https://github.com/Givemegitpls/hyprlock-accent-tool'
license=('MIT')
makedepends=('cargo' 'git')
depends=('glibc' 'libgcc')
arch=('x86_64')
source=("${_pkgbase}::git+https://github.com/Givemegitpls/hyprlock-accent-tool.git")
sha256sums=('SKIP')
provides=("${_pkgbase}=${pkgver}")
conflicts=("${_pkgbase}")

pkgver() {
  cd "$_pkgbase"
  (
    set -o pipefail
    git describe --long --tags --abbrev=7 2>/dev/null |
      sed 's/\([^-]*-g\)/r\1/;s/-/./g' ||
      printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
  )
}

build() {
  cd "$_pkgbase"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  # refresh the local registry index (locked: Cargo.lock is never rewritten),
  # so the --frozen build below works on machines with a stale cache
  cargo fetch --locked
  cargo build --frozen --release
}

package() {
  cd "$_pkgbase"
  install -Dm0755 -t "$pkgdir/usr/bin/" "target/release/$_pkgbase"
  install -Dm644 src/LICENSE -t "$pkgdir/usr/share/licenses/$_pkgbase/"
}
