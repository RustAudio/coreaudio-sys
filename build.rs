extern crate bindgen;

const FRAMEWORKS_PATH: &'static str = "System/Library/Frameworks";

fn sdk_path(target: &str) -> Result<String, std::io::Error> {
    // Use environment variable if set
    println!("cargo:rerun-if-env-changed=COREAUDIO_SDK_PATH");
    if let Ok(path) = std::env::var("COREAUDIO_SDK_PATH") {
        return Ok(path);
    }

    use std::process::Command;

    let sdk = if target.contains("apple-darwin") {
        "macosx"
    } else if target.contains("apple-ios") {
        "iphoneos"
    } else {
        unreachable!();
    };
    let output = Command::new("xcrun")
        .args(&["--sdk", sdk, "--show-sdk-path"])
        .output()?
        .stdout;
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcrun`");
    Ok(prefix_str.trim_end().to_string())
}

fn build(sdk_path: &str, target: &str) {
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
        if target.contains("apple-darwin") {
            println!("cargo:rustc-link-lib=framework=CoreMIDI");
            headers.push("CoreMIDI.framework/Headers/CoreMIDI.h");
        }
    }

    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    // Get the cargo out directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

    // Begin building the bindgen params.
    let mut builder = bindgen::Builder::default();

    builder = builder.clang_args(&[&format!("--target={}", target), "-isysroot", sdk_path]);

    // Add all headers.
    for relative_path in headers {
        let absolute_path = format!("{}/{}/{}", sdk_path, FRAMEWORKS_PATH, relative_path);
        builder = builder.header(absolute_path);
    }

    // Generate the bindings.
    builder = builder
        .trust_clang_mangling(false)
        .derive_default(true);

    let bindings = builder.generate().expect("unable to generate bindings");

    // Write them to the crate root.
    bindings
        .write_to_file(out_dir.join("coreaudio.rs"))
        .expect("could not write bindings");
}

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if !(target.contains("apple-darwin") || target.contains("apple-ios")) {
        panic!("coreaudio-sys requires macos or ios target");
    }

    if let Ok(directory) = sdk_path(&target) {
        build(&directory, &target);
    } else {
        panic!("coreaudio-sys could not find an appropriate SDK");
    }
}
