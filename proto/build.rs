fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    // SAFETY: single-threaded build script, runs before anything reads PROTOC.
    unsafe { std::env::set_var("PROTOC", protoc) };

    tonic_prost_build::configure()
        .compile_protos(&["sync/account/v1/account.proto"], &["proto"])
        .expect("proto compilation");
}
