use std::path::Path;

fn main() {
    let src_dir = Path::new("src");

    cc::Build::new()
        .file(src_dir.join("no-link-rt.c"))
        .compile("no-link-rt");
}
