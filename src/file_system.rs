use clean_path::Clean;
use ignore::{types::TypesBuilder, WalkBuilder, WalkState};
use parking_lot::Mutex;
use std::{path::PathBuf, str::FromStr};

pub fn get_files(paths: &Vec<PathBuf>) -> Vec<PathBuf> {
    let mut types_builder = TypesBuilder::new();
    types_builder
        .add("typescript", "*.{cts,mts,ts,tsx}")
        .expect("Invalid glob");
    types_builder.select("typescript");
    types_builder
        .add("javascript", "*.{cjs,mjs,js,jsx}")
        .expect("Invalid glob");
    types_builder.select("javascript");
    let types = types_builder.build().expect("Unable to build types");

    let mut walk_builder = WalkBuilder::new(paths[0].to_owned());
    if paths.len() > 1 {
        for path in paths.iter().skip(1) {
            walk_builder.add(path.to_owned());
        }
    }
    walk_builder.types(types);

    // no need for an Arc here because we know the closures will never outlive the function
    let files = Mutex::new(vec![]);

    /*
    NOTE: we could implement this using a custom `.visit` implementation that defers any shared memory operations until
    the end of each thread to avoid any time spent waiting for the mutex.
    However in practice the cost of the mutex on each write doesn't add any overhead worth mentioning - so instead we
    prefer the much simpler code.

    Tested with `hyperfine --warmup 2 './target/release/athena ../../work/canva/web/src`:

    Single threaded:
      Time (mean ± σ):      1.473 s ±  0.020 s    [User: 0.491 s, System: 0.975 s]
      Range (min … max):    1.446 s …  1.506 s    10 runs

    Parallel mutex-heavy:
      Time (mean ± σ):     835.0 ms ±  13.9 ms    [User: 539.7 ms, System: 1093.2 ms]
      Range (min … max):   815.3 ms … 859.4 ms    10 runs

    Parallel mutex-light:
      Time (mean ± σ):     839.6 ms ±  10.9 ms    [User: 540.0 ms, System: 1104.4 ms]
      Range (min … max):   819.9 ms … 851.4 ms    10 runs
    */
    walk_builder.build_parallel().run(|| {
        return Box::new(|result| {
            // Each item yielded by the iterator is either a directory entry or an
            // error, so either handle the path or the error.
            match result {
                Ok(entry) => match entry.file_type() {
                    Some(file_type) => {
                        if !file_type.is_dir() {
                            files.lock().push(entry.path().to_owned().clean());
                        }
                    }
                    None => {
                        // ignore non-file entries
                    }
                },
                Err(err) => println!("ERROR: {}", err),
            };
            return WalkState::Continue;
        });
    });

    return files.into_inner();
}

#[inline]
pub fn is_declaration_file(path: &PathBuf) -> bool {
    return path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts");
}

/// Ensures a path exists and converts it to an absolute representation
pub fn path_parser_absolute(path: &str) -> Result<PathBuf, std::io::Error> {
    return PathBuf::from_str(path)
        .expect(&format!("Expected a valid path, got {}", path))
        .canonicalize();
}

pub struct Extensions;
impl Extensions {
    // TS extensions
    pub const TS: &str = "ts";
    pub const D_TS: &str = "d.ts";
    pub const TSX: &str = "tsx";
    pub const CTS: &str = "cts";
    pub const D_CTS: &str = "d.cts";
    pub const MTS: &str = "mts";
    pub const D_MTS: &str = "d.mts";

    // JS extensions
    pub const JS: &str = "js";
    pub const JSX: &str = "jsx";
    pub const CJS: &str = "cjs";
    pub const MJS: &str = "mjs";

    // Special code-like extensions
    pub const CSS: &str = "css";
    pub const EJS: &str = "ejs";
    pub const JSON: &str = "json";

    // Misc loaded files
    pub const AVIF: &str = "avif";
    pub const FRAG: &str = "frag";
    pub const GIF: &str = "gif";
    pub const HTML: &str = "html";
    pub const JPG: &str = "jpg";
    pub const M4A: &str = "m4a";
    pub const MD: &str = "md";
    pub const MP3: &str = "mp3";
    pub const MP4: &str = "mp4";
    pub const OGV: &str = "ogv";
    pub const OTF: &str = "otf";
    pub const PNG: &str = "png";
    pub const SVG: &str = "svg";
    pub const TXT: &str = "txt";
    pub const TTF: &str = "ttf";
    pub const VERT: &str = "vert";
    pub const VTT: &str = "vtt";
    pub const WASM: &str = "wasm";
    pub const WEBM: &str = "webm";
    pub const WOFF: &str = "woff";
    pub const WOFF2: &str = "woff2";
}
