build:
    anchor build --no-idl
idl:
    RUSTUP_TOOLCHAIN=nightly-2025-04-01 anchor idl build -o target/idl/weedminer.json -t target/types/weedminer.ts