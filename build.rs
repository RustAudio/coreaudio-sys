extern crate bindgen;

fn osx_version() -> Result<String, std::io::Error> {
    use std::process::Command;

    let output = Command::new("defaults")
        .arg("read")
        .arg("loginwindow")
        .arg("SystemVersionStampAsString")
        .output()?
        .stdout;
    let version_str = std::str::from_utf8(&output).expect("invalid output from `defaults`");
    let version = version_str.trim_right();

    Ok(version.to_owned())
}

fn parse_version(version: &str) -> Option<i32> {
    version
        .split(".")
        .skip(1)
        .next()
        .and_then(|m| m.parse::<i32>().ok())
}

fn frameworks_path() -> Result<String, std::io::Error> {
    // For 10.13 and higher:
    //
    // While macOS has its system frameworks located at "/System/Library/Frameworks"
    // for actually linking against them (especially for cross-compilation) once
    // has to refer to the frameworks as found within "Xcode.app/Contents/Developer/…".

    // Use environment variable if set
    if let Ok(path) = std::env::var("COREAUDIO_FRAMEWORKS_PATH") {
        return Ok(path)
    }

    if osx_version()
        .and_then(|version| Ok(parse_version(&version).map(|v| v >= 13).unwrap_or(false)))
        .unwrap_or(false)
    {
        use std::process::Command;

        let output = Command::new("xcode-select").arg("-p").output()?.stdout;
        let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcode-select`");
        let prefix = prefix_str.trim_right();

        let target = std::env::var("TARGET").unwrap();
        let platform = if target.contains("apple-darwin") {
            "MacOSX"
        } else if target.contains("apple-ios") {
            "iPhoneOS"
        } else {
            unreachable!();
        };

        let infix = if prefix == "/Library/Developer/CommandLineTools" {
            format!("SDKs/{}.sdk", platform)
        } else {
            format!(
                "Platforms/{}.platform/Developer/SDKs/{}.sdk",
                platform, platform
            )
        };

        let suffix = "System/Library/Frameworks";
        let directory = format!("{}/{}/{}", prefix, infix, suffix);

        Ok(directory)
    } else {
        Ok("/System/Library/Frameworks".to_string())
    }
}

fn build(frameworks_path: &str) {
    // Generate one large set of bindings for all frameworks.
    //
    // We do this rather than generating a module per framework as some frameworks depend on other
    // frameworks and in turn share types. To ensure all types are compatible across each
    // framework, we feed all headers to bindgen at once.
    //
    // Only link to each framework and include their headers if their features are enabled and they
    // are available on the target os.

    use std::env;
    use std::path::PathBuf;

    let mut headers = vec![];

    #[cfg(feature = "audio_toolbox")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        headers.push("AudioToolbox.framework/Headers/AudioToolbox.h");
    }

    #[cfg(feature = "audio_unit")]
    {
        println!("cargo:rustc-link-lib=framework=AudioUnit");
        headers.push("AudioUnit.framework/Headers/AudioUnit.h");
    }

    #[cfg(feature = "core_audio")]
    {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        headers.push("CoreAudio.framework/Headers/CoreAudio.h");
    }

    #[cfg(feature = "open_al")]
    {
        println!("cargo:rustc-link-lib=framework=OpenAL");
        headers.push("OpenAL.framework/Headers/al.h");
        headers.push("OpenAL.framework/Headers/alc.h");
    }

    #[cfg(all(feature = "core_midi"))]
    {
        if std::env::var("TARGET").unwrap().contains("apple-darwin") {
            println!("cargo:rustc-link-lib=framework=CoreMIDI");
            headers.push("CoreMIDI.framework/Headers/CoreMIDI.h");
        }
    }

    // Get the cargo out directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

    // Begin building the bindgen params.
    let mut builder = bindgen::Builder::default();

    builder = builder.clang_arg(format!("-F/{}", frameworks_path));

    // Add all headers.
    for relative_path in headers {
        let absolute_path = format!("{}/{}", frameworks_path, relative_path);
        builder = builder.header(absolute_path);
    }

    // Generate the bindings.
    builder = builder
        .trust_clang_mangling(false)
        .derive_default(true)
        .rustfmt_bindings(false);

    if let Ok(cflags) = std::env::var("COREAUDIO_CFLAGS") {
        builder = builder.clang_args(cflags.split(" "));
    }

    let bindings = builder
        .generate()
        .expect("unable to generate bindings");

    // Write them to the crate root.
    bindings
        .write_to_file(out_dir.join("coreaudio.rs"))
        .expect("could not write bindings");
}

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if ! (target.contains("apple-darwin") || target.contains("apple-ios")) {
        eprintln!("coreaudio-sys requires macos or ios target");
    }

    if let Ok(directory) = frameworks_path() {
        build(&directory);
    } else {
        eprintln!("coreaudio-sys could not find frameworks path");
    }
}

