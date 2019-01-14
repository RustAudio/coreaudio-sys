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

fn isysroot_path(platform: &str) -> Result<String, std::io::Error> {
    use std::process::Command;

    let output = Command::new("xcrun")
        .arg("--sdk")
        .arg(platform)
        .arg("--show-sdk-path")
        .output()?
        .stdout;

    let sdk_path =
        std::str::from_utf8(&output).expect("invalid output from `xcrun --show-sdk-path`");
    let directory = String::from(sdk_path.trim());

    println!("SDK Directory: {}", directory);

    Ok(directory)
}

fn parse_version(version: &str) -> Option<i32> {
    version
        .split(".")
        .skip(1)
        .next()
        .and_then(|m| m.parse::<i32>().ok())
}

fn frameworks_path(platform: &str) -> Result<String, std::io::Error> {
    // For 10.13 and higher:
    //
    // While macOS has its system frameworks located at "/System/Library/Frameworks"
    // for actually linking against them (especially for cross-compilation) once
    // has to refer to the frameworks as found within "Xcode.app/Contents/Developer/…".

    if osx_version()
        .and_then(|version| Ok(parse_version(&version).map(|v| v >= 13).unwrap_or(false)))
        .unwrap_or(false)
    {
        use std::process::Command;

        let output = Command::new("xcode-select").arg("-p").output()?.stdout;
        let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcode-select`");
        let prefix = prefix_str.trim_right();

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

fn build(frameworks_path: &str, sysroot: &str, platform: &str, target: &str) {
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

    let mut frameworks = vec![];
    let mut headers = vec![];

    #[cfg(feature = "audio_toolbox")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        frameworks.push("AudioToolbox");
        headers.push("AudioToolbox.framework/Headers/AudioToolbox.h");
    }

    #[cfg(feature = "audio_unit")]
    {
        println!("cargo:rustc-link-lib=framework=AudioUnit");
        frameworks.push("AudioUnit");
        headers.push("AudioUnit.framework/Headers/AudioUnit.h");
    }

    #[cfg(feature = "core_audio")]
    {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        frameworks.push("CoreAudio");
        match platform {
            "MacOSX" => headers.push("CoreAudio.framework/Headers/CoreAudio.h"),
            "iPhoneOS" | "iPhoneSimulator" => {
                headers.push("CoreAudio.framework/Headers/CoreAudioTypes.h")
            }
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "open_al")]
    {
        println!("cargo:rustc-link-lib=framework=OpenAL");
        frameworks.push("OpenAL");
        headers.push("OpenAL.framework/Headers/al.h");
        headers.push("OpenAL.framework/Headers/alc.h");
    }

    #[cfg(all(feature = "core_midi", target_os = "macos"))]
    {
        if (platform == "MacOSX") {
            println!("cargo:rustc-link-lib=framework=CoreMIDI");
            frameworks.push("CoreMIDI");
            headers.push("CoreMIDI.framework/Headers/CoreMIDI.h");
        } else {
            panic!("framework CoreMIDI is not supported by Apple on this platform");
        }
    }

    // Get the cargo out directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

    // Begin building the bindgen params.
    let mut builder = bindgen::Builder::default();

    builder = builder
        .clang_arg(format!("-F/{}", frameworks_path))
        .clang_arg("-isysroot")
        .clang_arg(sysroot);

    // Add all headers.
    for relative_path in headers {
        let absolute_path = format!("{}/{}", frameworks_path, relative_path);
        builder = builder.header(absolute_path);
    }

    // Link to all frameworks.
    for relative_path in frameworks {
        let link_instruction = format!("#[link = \"{}/{}\"]", frameworks_path, relative_path);
        builder = builder.raw_line(link_instruction);
    }

    // Generate the bindings.
    let bindings = builder
        .clang_arg("-target")
        .clang_arg(target)
        .trust_clang_mangling(false)
        .derive_default(true)
        .rustfmt_bindings(false)
        .generate()
        .expect("unable to generate bindings");

    // Write them to the crate root.
    bindings
        .write_to_file(out_dir.join("coreaudio.rs"))
        .expect("could not write bindings");
}

fn main() {
    let (target, platform) = match std::env::var("TARGET") {
        Ok(val) => match val.as_ref() {
            "x86_64-apple-darwin" | "i686-apple-darwin" => (val, "MacOSX"),
            "aarch64-apple-ios" => (String::from("arm64-apple-ios"), "iPhoneOS"),
            "armv7-apple-ios" | "armv7s-apple-ios" | "i386-apple-ios" | "x86_64-apple-ios" => {
                (val, "iPhoneOS")
            }
            _ => panic!("coreaudio-sys requires macos or ios target. Found: {}", val),
        },
        Err(_e) => {
            panic!("TARGET environment variable not found (are you running this outside of cargo?)")
        }
    };
    
    if let Ok(directory) = frameworks_path(platform) {
        if let Ok(sysroot) = isysroot_path(&platform.to_lowercase()) {
            build(&directory, &sysroot, platform, &target);
        } else {
            eprintln!("coreaudio-sys could not find sysroot path");
        }
    } else {
        eprintln!("coreaudio-sys could not find frameworks path");
    }
}
