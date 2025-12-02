fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set protoc to use bundled version if not found
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path().unwrap());
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["src/proto/distbuild.proto"], &["src/proto"])?;
    Ok(())
}

