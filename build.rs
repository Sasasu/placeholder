extern crate protobuf_codegen_pure;
use cc;

fn main() {
    cc::Build::new().file("c_src/tuntap.c").compile("libtuntap");

    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/generated",
        input: &["types/transport.proto"],
        includes: &["types"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");
}
