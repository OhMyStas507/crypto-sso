fn main() {
    std::fs::write(
        "wallet/src/bridge/generated.ts",
        wallet_lib::dto::declarations(),
    )
    .unwrap();
}
