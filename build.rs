extern crate bindgen;

fn sdk_path(target: &str) -> Result<String, std::io::Error> {
    // Use environment variable if set
    println!("cargo:rerun-if-env-changed=COREAUDIO_SDK_PATH");
    if let Ok(path) = std::env::var("COREAUDIO_SDK_PATH") {
        return Ok(path);
    }

    use std::process::Command;

    let sdk = if target.contains("apple-darwin") {
        "macosx"
    } else if target == "x86_64-apple-ios"
        || target == "i386-apple-ios"
        || target == "aarch64-apple-ios-sim"
    {
        "iphonesimulator"
    } else if target == "aarch64-apple-ios"
        || target == "armv7-apple-ios"
        || target == "armv7s-apple-ios"
    {
        "iphoneos"
    } else if target == "aarch64-apple-visionos" {
        "xros"
    } else if target == "aarch64-apple-visionos-sim" {
        "xrsimulator"
    } else {
        unreachable!();
    };
    let output = Command::new("xcrun")
        .args(&["--sdk", sdk, "--show-sdk-path"])
        .output()?
        .stdout;
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcrun`");

    println!("COREAUDIO_SDK_PATH - {prefix_str}");

    Ok(prefix_str.trim_end().to_string())
}

fn build(sdk_path: Option<&str>, target: &str) {
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

    #[allow(unused)]
    let mut headers: Vec<&'static str> = vec![];

    #[cfg(feature = "audio_unit")]
    {
        // Since iOS 10.0, macOS 10.12, visionOS 1.0, all the functionality in AudioUnit
        // moved to AudioToolbox, and the AudioUnit headers have been simple
        // wrappers ever since.
        if target.contains("apple-ios"){
            // On iOS, the AudioUnit framework does not have (and never had) an
            // actual dylib to link to, it is just a few header files.
            // The AudioToolbox framework contains the symbols instead.
            println!("cargo:rustc-link-lib=framework=AudioToolbox");
            // headers.push("AudioToolbox/AudioUnit.h");
        } else {
            // On macOS, the symbols are present in the AudioToolbox framework,
            // but only on macOS 10.12 and above.
            //
            // However, unlike on iOS or visionOS, the AudioUnit framework on macOS
            // contains a dylib with the desired symbols, that we can link to
            // (in later versions just re-exports from AudioToolbox).
            println!("cargo:rustc-link-lib=framework=AudioUnit");
            // headers.push("AudioUnit/AudioUnit.h");
        }
        headers.push("AudioUnit/AudioUnit.h");
    }

    #[cfg(feature = "audio_toolbox")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        headers.push("AudioToolbox/AudioToolbox.h");
    }

    #[cfg(feature = "core_audio")]
    {
        println!("cargo:rustc-link-lib=framework=CoreAudio");

        if target.contains("apple-ios") || target.contains("apple-visionos") {
            headers.push("CoreAudio/CoreAudioTypes.h");
        } else {
            headers.push("CoreAudio/CoreAudio.h");

            #[cfg(feature = "audio_server_plugin")]
            {
                headers.push("CoreAudio/AudioServerPlugIn.h");
            }
        }
    }

    #[cfg(feature = "io_kit_audio")]
    {
        assert!(target.contains("apple-darwin"));
        println!("cargo:rustc-link-lib=framework=IOKit");
        headers.push("IOKit/audio/IOAudioTypes.h");
    }

    #[cfg(feature = "open_al")]
    {
        if target.contains("ios") {
            println!("cargo:rustc-link-lib=framework=OpenAL");
            headers.push("OpenAL/al.h");
            headers.push("OpenAL/alc.h");
        }
    }

    #[cfg(all(feature = "core_midi"))]
    {
        if target.contains("apple-darwin") && !target.contains("visionos") {
            println!("cargo:rustc-link-lib=framework=CoreMIDI");
            headers.push("CoreMIDI/CoreMIDI.h");
        }
    }

    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    // Get the cargo out directory.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

    // Begin building the bindgen params.
    let mut builder = bindgen::Builder::default();

    // See https://github.com/rust-lang/rust-bindgen/issues/1211
    // Technically according to the llvm mailing list, the argument to clang here should be
    // -arch arm64 but it looks cleaner to just change the target.
    let target = if target == "aarch64-apple-ios" {
        "arm64-apple-ios"
    } else if target == "aarch64-apple-visionos" {
        "arm64-apple-visionos"
    } else if target == "aarch64-apple-darwin" {
        "arm64-apple-darwin"
    } else {
        target
    };
    builder = builder.size_t_is_usize(true);

    builder = builder.clang_args(&[&format!("--target={}", target)]);
    // idea from https://github.com/RustAudio/coreaudio-sys/issues/23#issuecomment-507364379
    // builder = builder.clang_args(&["-I/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/System/Library/Frameworks/Kernel.framework", "-I/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk//usr/include"]);

    if let Some(sdk_path) = sdk_path {
        builder = builder.clang_args(&["-isysroot", sdk_path]);
    }
    if target.contains("apple-ios") || target.contains("apple-visionos") {
        // time.h as has a variable called timezone that conflicts with some of the objective-c
        // calls from NSCalendar.h in the Foundation framework. This removes that one variable.
        builder = builder.blocklist_item("timezone");
        builder = builder.blocklist_item("objc_object");
    }

    // bindgen produces alignment tests that cause undefined behavior in some cases.
    // This seems to happen across all apple target tripples :/.
    // https://github.com/rust-lang/rust-bindgen/issues/1651
    builder = builder.layout_tests(false);

    let meta_header: Vec<_> = headers
        .iter()
        .map(|h| format!("#include <{}>\n", h))
        .collect();

    
    let fixes = if target.contains("apple-visionos") {
        vec![
            // /Applications/Xcode.app/Contents/Developer/Platforms/XROS.platform/Developer/SDKs/XROS1.1.sdk/System/Library/Frameworks/AudioToolbox.framework/Headers/AudioToolbox.h:43:11: fatal error: 'AudioToolbox/AudioFileComponent.h' file not found
            "#define TARGET_OS_IPHONE true\n",
            // https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.13.sdk/usr/include/MacTypes.h#L289
            // /Applications/Xcode.app/Contents/Developer/Platforms/XROS.platform/Developer/SDKs/XROS1.1.sdk/System/Library/Frameworks/CoreMIDI.framework/Headers/MIDIServices.h:1633:8: error: unknown type name 'ItemCount'
            "typedef unsigned long ItemCount;\n",
            "typedef unsigned long ByteCount;\n"
        ]
    }else {
        vec![""]
    };

    let contents = format!("{}{}", fixes.concat(), meta_header.concat());

    println!("=============================");
    println!("{}",&contents);
    println!("=============================");

    builder = builder.header_contents("coreaudio.h", &contents);

    // Generate the bindings.
    builder = builder.trust_clang_mangling(false).derive_default(true);

    let bindings = builder.generate().expect("unable to generate bindings");

    // Write them to the crate root.
    bindings
        .write_to_file(out_dir.join("coreaudio.rs"))
        .expect("could not write bindings");
}

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if !(target.contains("apple-darwin") || target.contains("apple-ios") || target.contains("apple-visionos")) {
        panic!("coreaudio-sys requires macos or ios or visionos target");
    }

    let directory = sdk_path(&target).ok();
    build(directory.as_ref().map(String::as_ref), &target);
}
