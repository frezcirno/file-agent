TARGETS = x86_64-unknown-linux-gnu x86_64-pc-windows-msvc

agent:
	for target in $(TARGETS); do \
		cargo build -r --bin agent --target $$target; \
	done

server:
	cargo build -r --bin server
