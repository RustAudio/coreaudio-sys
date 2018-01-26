extern crate bindgen;

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn main() {
    use std::env;
    use std::path::PathBuf;

    // Generate one large set of bindings for all frameworks.
    //
    // We do this rather than generating a module per framework as some frameworks depend on other
    // frameworks and in turn share types. To ensure all types are compatible across each
    // framework, we feed all headers to bindgen at once.
    //
    // Only link to each framework and include their headers if their features are enabled and they
    // are available on the target os.

    let mut frameworks = vec![];
    let mut headers = vec![];

    #[cfg(feature = "audio_toolbox")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        frameworks.push("/System/Library/Frameworks/AudioToolbox");
        headers.push("/System/Library/Frameworks/AudioToolbox.framework/Headers/AudioToolbox.h");
    }

    #[cfg(feature = "audio_unit")]
    {
        println!("cargo:rustc-link-lib=framework=AudioUnit");
        frameworks.push("/System/Library/Frameworks/AudioUnit");
        headers.push("/System/Library/Frameworks/AudioUnit.framework/Headers/AudioUnit.h");
    }

    #[cfg(feature = "core_audio")]
    {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        frameworks.push("/System/Library/Frameworks/CoreAudio");
        headers.push("/System/Library/Frameworks/CoreAudio.framework/Headers/CoreAudio.h");
    }

    #[cfg(feature = "open_al")]
    {
        println!("cargo:rustc-link-lib=framework=OpenAL");
        frameworks.push("/System/Library/Frameworks/OpenAL");
        headers.push("/System/Library/Frameworks/OpenAL.framework/Headers/al.h");
        headers.push("/System/Library/Frameworks/OpenAL.framework/Headers/alc.h");
    }

    #[cfg(all(feature = "core_midi", target_os = "macos"))]
    {
        println!("cargo:rustc-link-lib=framework=CoreMIDI");
        frameworks.push("/System/Library/Frameworks/CoreMIDI");
        headers.push("/System/Library/Frameworks/CoreMIDI.framework/Headers/CoreMIDI.h");
    }

    // Get the cargo out directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

    // Begin building the bindgen params.
    let mut builder = bindgen::Builder::default();

    // Add all headers.
    for header_path in headers {
        builder = builder.header(header_path);
    }

    // Link to all frameworks.
    for framework in frameworks {
        builder = builder.link_framework(framework);
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

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn main() {
    eprintln!("coreaudio-sys requires macos or ios");
}
