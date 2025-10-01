fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(false)
        .build_server(true)
        .compile_protos(&["proto/user.proto"], &["proto/"])?;

    Ok(())
}
