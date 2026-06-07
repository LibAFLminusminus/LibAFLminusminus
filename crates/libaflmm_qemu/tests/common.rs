use libaflmm_qemu::{GuestAddr, elf::EasyElf, qemu::Qemu};
use std::{env, fs, panic::Location, path::PathBuf, process::Command};

const STUB: &str = r#"
int target();

int main() {
    return target();
}
"#;

fn cross_cc() -> String {
    if let Ok(cross_cc) = env::var("CROSS_CC") {
        cross_cc
    } else {
        "gcc".to_string()
    }
}

fn snippet_path(caller: &str) -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let main_rs = base.join(caller);
        if main_rs.is_file() {
            return main_rs.parent().unwrap().join("snippet.s");
        }
        assert!(base.pop(), "could not locate {caller}");
    }
}

pub fn find_symbol(qemu: Qemu, symbol: &str) -> GuestAddr {
    let mut elf_buf = Vec::new();
    let elf = EasyElf::from_file(qemu.binary_path(), &mut elf_buf).unwrap();
    elf.resolve_symbol(symbol, qemu.load_addr()).unwrap()
}

#[track_caller]
pub fn boot_qemu() -> Qemu {
    let snippet = snippet_path(Location::caller().file());
    assert!(
        snippet.is_file(),
        "missing snippet next to main.rs: {}",
        snippet.display()
    );

    let dir = env::temp_dir().join(format!("libaflmm_qemu_test_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let (stub_c, bin) = (dir.join("stub.c"), dir.join("prog"));
    fs::write(&stub_c, STUB).unwrap();

    let ok = Command::new(cross_cc())
        .args(["-static", "-no-pie"])
        .args(
            env::var("CROSS_CFLAGS")
                .unwrap_or_default()
                .split_whitespace(),
        )
        .arg("-o")
        .arg(&bin)
        .arg(&stub_c)
        .arg(&snippet)
        .status()
        .expect("failed to invoke guest C compiler (CROSS_CC)")
        .success();

    assert!(ok, "snippet compilation failed");

    Qemu::init(&["qemu", bin.to_str().unwrap()]).expect("qemu init")
}
