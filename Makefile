VERSION=0.0.1

all: debug

debug:
	cargo build

tag:
	git tag -a v${VERSION} -m v${VERSION}
	git push origin --tags

ver:
	sed -i 's/^version = ".*/version = "${VERSION}"/g' Cargo.toml
	sed -i 's/^pub const VERSION.*/pub const VERSION: \&str = "${VERSION}";/g' src/lib.rs

release:
	cargo build --target x86_64-unknown-linux-musl --release
	strip ./target/x86_64-unknown-linux-musl/release/rgbmon

pkg:
	rm -rf _build
	mkdir -p _build
	cd target/x86_64-unknown-linux-musl/release && tar czvf ../../../_build/rgbmon-${VERSION}-x86_64-musl.tar.gz rgbmon
	cd _build && echo "" | gh release create v$(VERSION) -t "v$(VERSION)" \
		  rgbmon-${VERSION}-x86_64-musleabihf.tar.gz
