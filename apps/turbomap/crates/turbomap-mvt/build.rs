fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["proto/vector_tile.proto"], &["proto/"])
}
