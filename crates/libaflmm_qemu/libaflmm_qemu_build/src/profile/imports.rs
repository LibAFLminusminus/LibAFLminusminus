use std::{env, fs, io::Result, path::PathBuf};

use regex::Regex;

#[derive(Debug)]
#[expect(dead_code)]
pub struct ProfileFile {
    // dir in which the file is located.
    dir: PathBuf,
    // the file content
    content: String,
}

// get the user profile
// this is first file to start from
pub fn find_user_profile() -> Result<ProfileFile> {
    println!("cargo:rerun-if-env-changed=LIBAFL_QEMU_PROFILE");

    if let Ok(env_path) = env::var("LIBAFL_QEMU_PROFILE") {
        // env variable set, use this in priority
        let path = PathBuf::from(env_path);

        if !path.is_file() {
            panic!("Profile \"{}\" could not be found.", path.display());
        }

        Ok(ProfileFile {
            dir: path.parent().unwrap().to_path_buf(),
            content: fs::read_to_string(path)?,
        })
    } else {
        // fallback: use the default user file
        // if not there, this is an unexpected error.

        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let project_dir = out_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        let profile_path = project_dir.join("qemu.toml");

        if !profile_path.is_file() {
            panic!(
                "\"qemu.toml\" not found in the project directory at: \"{}\"",
                project_dir.display()
            );
        }

        Ok(ProfileFile {
            dir: project_dir.to_path_buf(),
            content: fs::read_to_string(profile_path)?,
        })
    }
}

impl ProfileFile {
    // get an import profile
    #[expect(dead_code)]
    pub fn find_import_profile(&self, import_str: &str) -> Result<ProfileFile> {
        // check first if it matches the regex
        let re = Regex::new(r"^libaflmm_qemu:(?<profile_name>.+)$").unwrap();

        let profile_file = if let Some(caps) = re.captures(import_str) {
            // match, try to import one of the pre-made profiles.

            let content = match &caps["profile_name"] {
                "base" => include_str!("../../../profiles/base.toml"),
                "default" => include_str!("../../../profiles/default.toml"),
                profile_name => panic!("The profile file: \"{profile_name}\" could not be found. Available profiles: \"base\", \"default\".")
            }.to_string();

            ProfileFile {
                dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../profiles"),
                content,
            }
        } else {
            // no match, this is a path relative to the file in which the import was done.

            let profile_path = self.dir.join(import_str);

            if !profile_path.is_file() {
                panic!(
                    "Profile file: \"{}\" could not be found.",
                    profile_path.display()
                );
            }

            let profile_path = profile_path.canonicalize()?;
            let profile_content = fs::read_to_string(&profile_path)?;

            ProfileFile {
                dir: profile_path.parent().unwrap().to_path_buf(),
                content: profile_content,
            }
        };

        Ok(profile_file)
    }
}
