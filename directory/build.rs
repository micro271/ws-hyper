fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(&["../auth/proto/info.proto"], &["../auth/proto"])?;

    tonic_prost_build::configure()
        .build_client(false)
        .build_server(true)
        .compile_protos(&["proto/directory.proto"], &["proto"])?;

    Ok(())
}
