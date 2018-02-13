extern crate bindgen;

fn frameworks_path() -> Result<String, std::io::Error> {
    // While macOS has its system frameworks located at "/System/Library/Frameworks"
    // for actually linking against them (especially for cross-compilation) once
    // has to refer to the frameworks as found within "Xcode.app/Contents/Developer/…".

    use std::process::Command;

    let output = Command::new("xcode-select").arg("-p").output()?.stdout;
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcode-select`");
    let prefix = prefix_str.trim_right();

    let platform = if cfg!(target_os = "macos") {
        "MacOSX"
    } else if cfg!(target_os = "ios") {
        "iPhoneOS"
    } else {
        unreachable!();
    };

    let infix = format!("Platforms/{}.platform/Developer/SDKs/{}.sdk", platform, platform);
    let suffix = "System/Library/Frameworks";
    let directory = format!("{}/{}/{}", prefix, infix, suffix);

    Ok(directory)
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
        headers.push("CoreAudio.framework/Headers/CoreAudio.h");
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
        println!("cargo:rustc-link-lib=framework=CoreMIDI");
        frameworks.push("CoreMIDI");
        headers.push("CoreMIDI.framework/Headers/CoreMIDI.h");
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

    // Link to all frameworks.
    for relative_path in frameworks {
        let absolute_path = format!("{}/{}", frameworks_path, relative_path);
        builder = builder.link_framework(absolute_path);
    }

    // Generate the bindings.
    let bindings = builder
        .trust_clang_mangling(false)
        .derive_default(true)
        .generate()
        .expect("unable to generate bindings");

    // Write them to the crate root.
    bindings.write_to_file(out_dir.join("coreaudio.rs")).expect("could not write bindings");
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn main() {
    if let Ok(directory) = frameworks_path() {
        build(&directory);
    } else {
        eprintln!("coreaudio-sys could not find frameworks path");
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn main() {
    eprintln!("coreaudio-sys requires macos or ios target");
}
