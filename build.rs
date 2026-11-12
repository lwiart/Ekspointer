fn main() {
    println!("cargo:rerun-if-changed=assets/ekspointer.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    embed_resource::compile("assets/ekspointer.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
