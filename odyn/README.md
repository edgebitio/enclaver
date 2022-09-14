# Odyn: PID 1 for the inside of the enclave

## Responsibilities
1. Initialize the enclave
	- bring up loopback network device
	- seed entropy
2. Load the manifest
3. Start the entrypoint
4. Serve the entrypoint status to the host
5. Reap the orphans
6. Serve up kernel and application logs to the host (console)
7. Run the network proxy
8. Enforce the network security policy
9. Run the KMS proxy
10. Handle graceful shutdown

## Build
1. Ensure you have the latest [Rust](https://www.rust-lang.org/tools/install)
2. Add musl toolchain: `rustup target add x86_64-unknown-linux-musl`
2. Build the static binary: `cargo build`
