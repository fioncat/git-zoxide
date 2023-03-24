all:
	@cargo build --release --locked --color=always --verbose

.PHONY: dev
dev:
	@cargo build
	@mv ./target/debug/git-zoxide ~/.cargo/bin
